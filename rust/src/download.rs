//! Parallel batch download implementation.
//! 
//! Partially inspired by: https://patshaughnessy.net/2020/1/20/downloading-100000-files-using-async-rust

use std::io::{self, Write};
use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;

use sha1::{Digest, Sha1};

use reqwest::Client;

use tokio::io::AsyncWriteExt;
use tokio::runtime::Builder;
use tokio::task::JoinSet;
use tokio::sync::mpsc;
use tokio::fs::File;


/// A list of pending download that can be all downloaded at once.
#[derive(Debug)]
pub struct Batch {
    entries: Vec<Entry>,
}

impl Batch {

    /// Create a new empty download list.
    #[inline]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a single download entry to the batch.
    #[inline]
    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    /// Block while downloading all entries in the batch.
    pub fn download(self, handler: impl Handler) -> Result<()> {
        
        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(download_impl(handler, 40, self.entries))
        
    }

}

/// A handle for watching a batch download progress.
pub trait Handler {
    
    /// Notification of a download progress. This function should return true to continue
    /// the downloading. This is called anyway at the beginning and at the end of the
    /// download.
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32);

    fn as_download_dyn(&mut self) -> &mut dyn Handler
    where Self: Sized {
        self
    }
    
}

/// Blanket implementation it no handler is needed.
impl Handler for () {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        let _ = (count, total_count, size, total_size);
    }
}

impl<H: Handler + ?Sized> Handler for &'_ mut H {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        (*self).handle_download_progress(count, total_count, size, total_size)
    }
}

/// A download entry to be added to a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Source of the download.
    pub source: EntrySource,
    /// Path to the file to ultimately download.
    pub file: Box<Path>,
    /// True if the file should be made executable on systems where its relevant to 
    /// later execute a binary.
    pub executable: bool,
}

/// A download source, with the URL, expected size (optional) and hash (optional),
/// it doesn't contain any information about the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySource {
    /// Url of the file to download.
    pub url: Box<str>,
    /// Expected size of the file, checked after downloading.
    pub size: Option<u32>,
    /// Expected SHA-1 of the file, checked after downloading.
    pub sha1: Option<[u8; 20]>,
}

/// Convert this entry into a batch with this single entry in it.
impl From<Entry> for Batch {
    #[inline]
    fn from(value: Entry) -> Self {
        Batch { entries: vec![value] }
    }
}

/// The error type containing one error for each failed entry.
#[derive(thiserror::Error, Debug)]
#[error("errors: {errors:?}")]
pub struct Error {
    pub errors: Vec<(Entry, EntryError)>,
}

/// An error for a single entry.
#[derive(thiserror::Error, Debug)]
pub enum EntryError {
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("invalid size")]
    InvalidSize,
    #[error("invalid sha1")]
    InvalidSha1,
}

/// Type alias for a result of batch download.
pub type Result<T> = std::result::Result<T, Error>;


/// Bulk download async entrypoint.
async fn download_impl(
    mut handler: impl Handler,
    concurrent_count: usize,
    mut entries: Vec<Entry>
) -> Result<()> {

    // Sort our entries in order to download big files first, this is allows better
    // parallelization at start and avoid too much blocking at the end.
    // Not sorting entries without size.
    entries.sort_by(|a, b| {
        match (a.source.size, b.source.size) {
            (Some(a), Some(b)) => Ord::cmp(&b, &a),
            _ => Ordering::Equal,
        }
    });

    // Current downloaded size and total size.
    let mut size = 0;
    let mut total_size = entries.iter()
        .map(|dl| dl.source.size.unwrap_or(0))
        .sum::<u32>();

    handler.handle_download_progress(0, entries.len() as u32, size, total_size);

    // Initialize the HTTP(S) client.
    let client = crate::http::builder()
        .build()
        .unwrap(); // FIXME:

    // Downloads are now immutable un order to be efficiently shared.
    // Note that we are intentionally not creating a arc of slice, because it is then
    // impossible to transform back to a vector, to we keep this double indirection.
    let entries = Arc::new(entries);
    let client = Arc::new(client);

    let mut index = 0;
    let mut completed = 0;

    let mut futures = JoinSet::new();
    let mut trackers = vec![EntryTracker::default(); entries.len()];

    let (tx, mut rx) = mpsc::channel(concurrent_count * 2);

    // Send a progress update for each 1000 parts of the download.
    let progress_size_interval = if total_size == 0 { 0 } else { total_size / 1000 };
    let mut last_size = 0u32;

    // The error list returned as error if at least one entry.
    let mut errors = Vec::new();

    // If we have theoretically completed all downloads, we still wait for joining all
    // remaining futures in the join set.
    while completed < entries.len() || !futures.is_empty() {
        
        while futures.len() < concurrent_count && index < entries.len() {
            futures.spawn(download_wrapper(
                Arc::clone(&client), 
                Arc::clone(&entries),
                index, 
                tx.clone()));
            index += 1;
        }

        let event = tokio::select! {
            _ = futures.join_next() => continue,
            event = rx.recv() => event.expect("channel should never close"),
        };

        let download = &entries[event.index];
        let tracker = &mut trackers[event.index];
        let mut force_progress = false;
        
        match event.kind {
            EntryEventKind::Progress(current_size) => {

                let diff = current_size - tracker.size;
                tracker.size = current_size;

                size += diff as u32;

                // If the source size was not initially counted in total size,
                // also add diff to total size.
                if download.source.size.is_none() {
                    total_size += diff as u32;
                }

            }
            EntryEventKind::Failed(e) => {
                errors.push((download.clone(), e));
                completed += 1;
                force_progress = true;
            }
            EntryEventKind::Success => {
                completed += 1;
                force_progress = true;
            }
        }
        
        if force_progress || size - last_size >= progress_size_interval {
            handler.handle_download_progress(completed as u32, entries.len() as u32, size, total_size);
            last_size = size;
        }

    }

    // Ensure that all tasks are aborted, this allows us to take back ownership of the 
    // underlying vector of entries.
    assert!(futures.is_empty());

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error { errors })
    }

}

/// Download entrypoint for a download, this is a wrapper around core download
/// function in order to easily catch any error and send it as event.
async fn download_wrapper(
    client: Arc<Client>, 
    entries: Arc<Vec<Entry>>,
    index: usize,
    mut tx: mpsc::Sender<EntryEvent>,
) {
    if let Err(e) = download_core(&client, &entries, index, &mut tx).await {
        tx.send(EntryEvent {
            index,
            kind: EntryEventKind::Failed(e),
        }).await.unwrap();
    } else {
        tx.send(EntryEvent {
            index,
            kind: EntryEventKind::Success,
        }).await.unwrap();
    }
}

/// Internal function to download a single download entry.
async fn download_core(
    client: &Client, 
    entries: &[Entry],
    index: usize,
    tx: &mut mpsc::Sender<EntryEvent>,
) -> std::result::Result<(), EntryError> {

    let download = &entries[index];
    let mut res = client.get(&*download.source.url).send().await?;
    
    if let Some(parent) = download.file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut dst = File::create(&*download.file).await?;

    let mut sha1 = download.source.sha1.map(|_| Sha1::new());
    let mut size = 0usize;
    
    while let Some(chunk) = res.chunk().await? {

        size += chunk.len();
        AsyncWriteExt::write_all(&mut dst, &chunk).await?;

        // // Taking ownership of digest to temporarily pass it to the blocking closure.
        // if let Some(mut digest) = sha1.take() {
        //     sha1 = Some(spawn_blocking(move || Write::write_all(&mut digest, &chunk).map(|()| digest)).await.unwrap()?);
        // }

        if let Some(digest) = &mut sha1 {
            Write::write_all(digest, &chunk)?;
        }

        tx.send(EntryEvent {
            index,
            kind: EntryEventKind::Progress(size),
        }).await.unwrap();

    }

    if let Some(expected_size) = download.source.size {
        if expected_size as usize != size {
            return Err(EntryError::InvalidSize);
        }
    }

    if let Some(expected_sha1) = &download.source.sha1 {
        if sha1.unwrap().finalize().as_slice() != expected_sha1 {
            return Err(EntryError::InvalidSha1);
        }
    }

    Ok(())

}

#[derive(Debug)]
struct EntryEvent {
    index: usize,
    kind: EntryEventKind,
}

#[derive(Debug)]
enum EntryEventKind {
    /// Progress of the download, total downloaded size is given.
    Progress(usize),
    /// The download has been completed with an error.
    Failed(EntryError),
    /// The download has been completed successfully.
    Success,
}

#[derive(Debug, Default, Clone)]
struct EntryTracker {
    /// Current downloaded size for this download.
    size: usize,
}