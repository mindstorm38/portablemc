//! Parallel batch HTTP(S) download implementation.
//! 
//! Partially inspired by: https://patshaughnessy.net/2020/1/20/downloading-100000-files-using-async-rust

use std::io::{self, BufWriter, SeekFrom, Write};
use std::cmp::Ordering;
use std::path::Path;
use std::sync::Arc;
use std::env;

use sha1::{Digest, Sha1};

use reqwest::{header, Client, StatusCode};

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::runtime::Builder;
use tokio::fs::{self, File};
use tokio::task::JoinSet;
use tokio::sync::mpsc;


/// A list of pending download that can be all downloaded at once.
#[derive(Debug)]
pub struct Batch {
    /// Internal batch entries to download.
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

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        let _ = (count, total_count, size, total_size);
    }

    fn as_download_dyn(&mut self) -> &mut dyn Handler
    where Self: Sized {
        self
    }
    
}

/// Blanket implementation it no handler is needed.
impl Handler for () { }

impl<H: Handler + ?Sized> Handler for &'_ mut H {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        (*self).handle_download_progress(count, total_count, size, total_size)
    }
}

/// A download source, with the URL, expected size (optional) and hash (optional),
/// it doesn't contain any information about the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySource {
    /// Url of the file to download, supporting only HTTP/HTTPS protocols.
    pub url: Box<str>,
    /// Expected size of the file, checked after downloading.
    pub size: Option<u32>,
    /// Expected SHA-1 of the file, checked after downloading.
    pub sha1: Option<[u8; 20]>,
}

/// Download mode for an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryMode {
    /// The entry is downloaded anyway.
    #[default]
    Force,
    /// Use a file next to the entry file to keep track of the last-modified and entity
    /// tag HTTP informations, that will be used in next downloads to actually download
    /// the data only if needed. This means that the entry will not always be downloaded,
    /// and its optional size and SHA-1 will be only checked when actually downloaded.
    Cache,
}

/// A download entry to be downloaded later. How this entry will be downloaded depends
/// on its mode, see [`EntryMode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Source of the download.
    pub source: EntrySource,
    /// Path to the file to ultimately download.
    pub file: Box<Path>,
    /// Download mode for this entry.
    pub mode: EntryMode,
}

impl EntrySource {

    /// Create a new entry source from the given URL, without known size of SHA-1. For
    /// a more complex construction, use the struct constructor directly.
    #[inline]
    pub fn new(url: impl Into<Box<str>>) -> Self {
        Self {
            url: url.into(),
            size: None,
            sha1: None,
        }
    }

    /// Convert this entry source into an entry with known destination and mode.
    #[inline]
    pub fn with_file_and_mode(self, file: impl Into<Box<Path>>, mode: EntryMode) -> Entry {
        Entry {
            source: self,
            file: file.into(),
            mode,
        }
    }

    /// Convert this entry source into an entry with known destination, the default
    /// [`Force`](EntryMode::Force) is used, the entry will be downloaded anyway.
    #[inline]
    pub fn with_file(self, file: impl Into<Box<Path>>) -> Entry {
        self.with_file_and_mode(file, EntryMode::Force)
    }

}

impl Entry {

    /// Create a new purely cached download entry, only the URL is given because the
    /// destination directory is constructed from a standard cache directory called
    /// `portablemc-cache` located in a standard user cache directory (or system tmp
    /// as a fallback), the file name in that directory is the hash of the URL.
    /// The entry mode is also set to [`Cache`](EntryMode::Cache).
    pub fn new_cached(url: impl Into<Box<str>>) -> Self {

        let url = url.into();
        let url_digest = {
            let mut sha1 = Sha1::new();
            sha1.update(&*url);
            format!("{:x}", sha1.finalize())
        };

        // Fallback to the tmp directory.
        let mut file = dirs::cache_dir()
            .unwrap_or(env::temp_dir());

        file.push("portablemc-cache");
        file.push(url_digest);

        EntrySource::new(url).with_file_and_mode(file, EntryMode::Cache)

    }

    /// Block while downloading this single entry.
    pub fn download(self, handler: impl Handler) -> Result<()> {
        Batch { entries: vec![self] }.download(handler)
    }

}

/// The error type containing one error for each failed entry in a download batch.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// This error happens when the initialization of the HTTP client fails, before 
    /// downloading any entry.
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// This error happens once the HTTP client has been fully initialize, and contains
    /// possibly many failed entries.
    #[error("entries: {0:?}")]
    Entries(Vec<(Entry, EntryError)>),
}

/// An error for a single entry.
#[derive(thiserror::Error, Debug)]
pub enum EntryError {
    /// HTTP error while downloading the entry.
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// System I/O error while writing the downloaded entry.
    #[error("io: {0}")]
    Io(#[from] io::Error),
    /// Invalid HTTP status code while requesting the entry.
    #[error("invalid status: {0}")]
    InvalidStatus(u16),
    /// Invalid size of the fully downloaded entry compared to the expected size.
    /// Implies that [`EntrySource::size`] is not none.
    #[error("invalid size")]
    InvalidSize,
    /// Invalid SHA-1 of the fully downloaded entry compared to the expected SHA-1.
    /// Implies that [`EntrySource::sha1`] is not none.
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
    let client = crate::http::builder().build()?;

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
    let progress_size_interval = total_size / 1000;
    let mut last_size = 0u32;

    // The error list returned as error if at least one entry.
    let mut errors = Vec::new();

    // If we have theoretically completed all downloads, we still wait for joining all
    // remaining futures in the join set.
    while completed < entries.len() || !futures.is_empty() {
        
        while futures.len() < concurrent_count && index < entries.len() {
            futures.spawn(download_entry_wrapper(
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
            EntryEventKind::Error(e) => {
                errors.push((download.clone(), e));
                completed += 1;
                force_progress = true;
            }
            EntryEventKind::Success(_) => {
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
        Err(Error::Entries(errors))
    }

}

/// Download entrypoint for a download, this is a wrapper around core download
/// function in order to easily catch the result and send it as an event.
async fn download_entry_wrapper(
    client: Arc<Client>, 
    entries: Arc<Vec<Entry>>,
    index: usize,
    mut tx: mpsc::Sender<EntryEvent>,
) {
    
    let event_kind = match download_entry(&client, &entries, index, &mut tx).await {
        Ok(()) => EntryEventKind::Success(EntrySuccess {  }),
        Err(e) => EntryEventKind::Error(e),
    };

    tx.send(EntryEvent {
        index,
        kind: event_kind,
    }).await.unwrap();

}

/// Internal function to download a single download entry, returning a result.
async fn download_entry(
    client: &Client, 
    entries: &[Entry],
    index: usize,
    tx: &mut mpsc::Sender<EntryEvent>,
) -> std::result::Result<(), EntryError> {

    let entry = &entries[index];

    let mut req = client.get(&*entry.source.url);
    
    // If we are in cache mode, then we derive the file name.
    let cache_file = (entry.mode == EntryMode::Cache).then(|| {
        let mut buf = entry.file.to_path_buf();
        buf.as_mut_os_string().push(".cache");
        buf
    });

    // If we are in cache mode, try open it.
    let mut cache_header = false;
    if let Some(cache_file) = cache_file.as_deref() {
        if let Some(cache) = check_download_cache(&entry.file, cache_file).await? {
            if let Some(etag) = cache.etag.as_deref() {
                req = req.header(header::IF_NONE_MATCH, etag);
                cache_header = true;
            }
            if let Some(last_modified) = cache.last_modified.as_deref() {
                req = req.header(header::IF_MODIFIED_SINCE, last_modified);
                cache_header = true;
            }
        }
    }

    // If it's a connection error just use the cached copy.
    let mut res = match req.send().await {
        Err(e) if cache_header && (e.is_timeout() || e.is_request() || e.is_connect()) => return Ok(()),
        Err(e) => return Err(e.into()),
        Ok(res) => res,
    };
    
    if res.status() == StatusCode::NOT_MODIFIED && cache_header {
        // The server answer that the file has not been modified, do nothing.
        return Ok(());
    } else if res.status() != StatusCode::OK {
        return Err(EntryError::InvalidStatus(res.status().as_u16()));
    }

    if let Some(parent) = entry.file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut dst = File::create(&*entry.file).await?;

    let mut size = 0usize;
    let mut sha1 = Sha1::new();
    
    while let Some(chunk) = res.chunk().await? {

        size += chunk.len();
        AsyncWriteExt::write_all(&mut dst, &chunk).await?;
        Write::write_all(&mut sha1, &chunk)?;

        tx.send(EntryEvent {
            index,
            kind: EntryEventKind::Progress(size),
        }).await.unwrap();

    }

    let size = u32::try_from(size).map_err(|_| EntryError::InvalidSize)?;
    let sha1 = sha1.finalize();

    if let Some(expected_size) = entry.source.size {
        if expected_size != size {
            return Err(EntryError::InvalidSize);
        }
    }

    if let Some(expected_sha1) = &entry.source.sha1 {
        if expected_sha1 != sha1.as_slice() {
            return Err(EntryError::InvalidSha1);
        }
    }

    // If we have a cache file, write it.
    if let Some(cache_file) = cache_file.as_deref() {

        let etag = res.headers().get(header::ETAG).and_then(|h| h.to_str().ok().map(str::to_string));
        let last_modified = res.headers().get(header::LAST_MODIFIED).and_then(|h| h.to_str().ok().map(str::to_string));

        // Only write the cache file if relevant!
        if etag.is_some() || last_modified.is_some() {

            let cache_meta_writer = File::create(cache_file).await?;
            let cache_meta_writer = BufWriter::new(cache_meta_writer.into_std().await);

            let res = serde_json::to_writer(cache_meta_writer, &serde::CacheMeta {
                url: entry.source.url.to_string(),
                size,
                sha1: crate::serde::Sha1HashString(sha1.into()),
                etag: res.headers().get(header::ETAG).and_then(|h| h.to_str().ok().map(str::to_string)),
                last_modified: res.headers().get(header::LAST_MODIFIED).and_then(|h| h.to_str().ok().map(str::to_string)),
            });

            // Silently ignore errors by we remove the file if it happens.
            if res.is_err() {
                let _ = fs::remove_file(cache_file).await;
            }

        }

    }

    Ok(())

}

/// Given a file and its cache file, return the cache metadata only if the file is valid
/// (existing) and the file has not been modified (size and SHA-1).
async fn check_download_cache(file: &Path, cache_file: &Path) -> io::Result<Option<serde::CacheMeta>> {

    // Start by reading the cache metadata associated to this file.
    let cache = match File::open(cache_file).await {
        Ok(file) => serde_json::from_reader::<_, serde::CacheMeta>(file.into_std().await).ok(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };

    let Some(cache) = cache else {
        return Ok(None);
    };

    let mut reader = match File::open(file).await {
        Ok(reader) => reader,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    // Start by checking size...
    let actual_size = reader.seek(SeekFrom::End(0)).await?;
    if cache.size as u64 != actual_size {
        return Ok(None);
    }
    reader.seek(SeekFrom::Start(0)).await?;

    // Then we check SHA-1...
    let mut digest = Sha1::new();
    io::copy(&mut reader.into_std().await, &mut digest)?;
    if cache.sha1.0 != digest.finalize().as_slice() {
        return Ok(None);
    }

    Ok(Some(cache))

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
    /// The download has been (partially) completed with an error.
    Error(EntryError),
    /// The download has been completed successfully.
    Success(EntrySuccess),
}

#[derive(Debug)]
struct EntrySuccess { }

#[derive(Debug, Default, Clone)]
struct EntryTracker {
    /// Current downloaded size for this download.
    size: usize,
}

/// Internal module for serde of cache metadata file.
mod serde {

    use crate::serde::Sha1HashString;

    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    pub struct CacheMeta {
        /// The full URL of the cached resource, just for information.
        pub url: String,
        /// Size of the cached file, used to verify its validity.
        pub size: u32,
        /// SHA-1 hash of the cached file, used to verify its validity. 
        pub sha1: Sha1HashString,
        /// The ETag if present.
        pub etag: Option<String>,
        /// Last modified data if present.
        pub last_modified: Option<String>,
    }

}
