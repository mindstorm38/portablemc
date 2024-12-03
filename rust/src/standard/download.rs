//! Parallel download implementation for standard installer.
//! 
//! Partially inspired by: https://patshaughnessy.net/2020/1/20/downloading-100000-files-using-async-rust

use std::io::{self, Write};
use std::cmp::Ordering;
use std::sync::Arc;

use sha1::{Digest, Sha1};

use reqwest::Client;

use tokio::io::AsyncWriteExt;
use tokio::runtime::Builder;
use tokio::task::JoinSet;
use tokio::sync::mpsc;
use tokio::fs::File;

use super::{Download, Event, Handler, Installer, Result};


/// The user agent to be used on each HTTP request.
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Bulk download blocking entrypoint.
pub fn download_many_blocking(
    installer: &Installer, 
    handler: &mut dyn Handler, 
    downloads: Vec<Download>
) -> Result<()> {

    // let cpu = std::thread::available_parallelism()
    //     .unwrap()
    //     .get();

    // let worker_threads = cpu * 2;

    // let rt = Builder::new_multi_thread()
    //     .worker_threads(worker_threads)
    //     .enable_time()
    //     .enable_io()
    //     .build()
    //     .unwrap();

    let rt = Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    rt.block_on(download_many(installer, handler, 40, downloads))

}

/// Bulk download async entrypoint.
pub async fn download_many(
    installer: &Installer, 
    handler: &mut dyn Handler,
    concurrent_count: usize,
    mut downloads: Vec<Download>
) -> Result<()> {

    // Sort our entries in order to download big files first, this is allows better
    // parallelization at start and avoid too much blocking at the end.
    // Not sorting entries without size.
    downloads.sort_by(|a, b| {
        match (a.source.size, b.source.size) {
            (Some(a), Some(b)) => Ord::cmp(&b, &a),
            _ => Ordering::Equal,
        }
    });

    // Current downloaded size and total size.
    let mut size = 0;
    let mut total_size = downloads.iter()
        .map(|dl| dl.source.size.unwrap_or(0))
        .sum::<u32>();

    handler.handle(installer, Event::DownloadProgress { 
        count: 0, 
        total_count: downloads.len() as u32, 
        size, 
        total_size,
    })?;

    // Initialize the HTTP(S) client.
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap(); // FIXME:

    // Downloads are now immutable un order to be efficiently shared.
    let downloads = Arc::<[Download]>::from(downloads.into_boxed_slice());
    let client = Arc::new(client);

    let mut index = 0;
    let mut completed = 0;

    let mut futures = JoinSet::new();
    let mut trackers = vec![DownloadTracker::default(); downloads.len()];

    let (tx, mut rx) = mpsc::channel(concurrent_count * 2);

    // Send a progress update for each 1000 parts of the download.
    let progress_size_interval = if total_size == 0 { 0 } else { total_size / 1000 };
    let mut last_size = 0u32;

    // If we have theoretically completed all downloads, we still wait for joining all
    // remaining futures in the join set.
    while completed < downloads.len() || !futures.is_empty() {
        
        while futures.len() < concurrent_count && index < downloads.len() {
            futures.spawn(download_wrapper(
                Arc::clone(&client), 
                Arc::clone(&downloads),
                index, 
                tx.clone()));
            index += 1;
        }

        let event = tokio::select! {
            _ = futures.join_next() => continue,
            event = rx.recv() => event.expect("channel should never close"),
        };

        let download = &downloads[event.index];
        let tracker = &mut trackers[event.index];
        let mut force_progress = false;
        
        match event.kind {
            DownloadEventKind::Progress(current_size) => {

                let diff = current_size - tracker.size;
                tracker.size = current_size;

                size += diff as u32;

                // If the source size was not initially counted in total size,
                // also add diff to total size.
                if download.source.size.is_none() {
                    total_size += diff as u32;
                }

            }
            DownloadEventKind::Failed(e) => {
                println!("error: {}: {e}", download.source.url);
                completed += 1;
                force_progress = true;
            }
            DownloadEventKind::Success => {
                completed += 1;
                force_progress = true;
            }
        }
        
        if force_progress || size - last_size >= progress_size_interval {
            handler.handle(installer, Event::DownloadProgress { 
                count: completed as u32, 
                total_count: downloads.len() as u32, 
                size, 
                total_size,
            })?;
            last_size = size;
        }

    }

    Ok(())

}

/// Download entrypoint for a download, this is a wrapper around core download
/// function in order to easily catch any error and send it as event.
async fn download_wrapper(
    client: Arc<Client>, 
    downloads: Arc<[Download]>,
    index: usize,
    mut tx: mpsc::Sender<DownloadEvent>,
) {
    if let Err(e) = download_core(&client, &downloads, index, &mut tx).await {
        tx.send(DownloadEvent {
            index,
            kind: DownloadEventKind::Failed(e),
        }).await.unwrap();
    } else {
        tx.send(DownloadEvent {
            index,
            kind: DownloadEventKind::Success,
        }).await.unwrap();
    }
}

/// Internal function to download a single download entry.
async fn download_core(
    client: &Client, 
    downloads: &[Download],
    index: usize,
    tx: &mut mpsc::Sender<DownloadEvent>,
) -> std::result::Result<(), DownloadError> {

    let download = &downloads[index];
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

        tx.send(DownloadEvent {
            index,
            kind: DownloadEventKind::Progress(size),
        }).await.unwrap();

    }

    if let Some(expected_size) = download.source.size {
        if expected_size as usize != size {
            return Err(DownloadError::InvalidSize);
        }
    }

    if let Some(expected_sha1) = &download.source.sha1 {
        if sha1.unwrap().finalize().as_slice() != expected_sha1 {
            return Err(DownloadError::InvalidSha1);
        }
    }

    Ok(())

}

#[derive(thiserror::Error, Debug)]
enum DownloadError {
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("invalid size")]
    InvalidSize,
    #[error("invalid sha1")]
    InvalidSha1,
}

#[derive(Debug)]
struct DownloadEvent {
    index: usize,
    kind: DownloadEventKind,
}

#[derive(Debug)]
enum DownloadEventKind {
    /// Progress of the download, total downloaded size is given.
    Progress(usize),
    /// The download has been completed with an error.
    Failed(DownloadError),
    /// The download has been completed successfully.
    Success,
}

#[derive(Debug, Default, Clone)]
struct DownloadTracker {
    /// Current downloaded size for this download.
    size: usize,
}
