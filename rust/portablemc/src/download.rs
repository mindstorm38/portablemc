//! Parallel batch HTTP(S) download implementation.
//! 
//! Partially inspired by: 
//! <https://patshaughnessy.net/2020/1/20/downloading-100000-files-using-async-rust>

use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::iter::FusedIterator;
use std::cmp::Ordering;
use std::path::Path;
use std::{env, mem};
use std::sync::Arc;

use sha1::{Digest, Sha1};

use reqwest::{header, Client, StatusCode};

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::fs::{self, File};
use tokio::task::JoinSet;
use tokio::sync::mpsc;

use crate::path::PathBufExt;


/// Download a single entry from the given URL to the given file.
pub fn single(url: impl Into<Box<str>>, file: impl Into<Box<Path>>) -> Single {
    Single(Entry::new(url.into(), file.into()))
}

/// Download a single cached entry.
pub fn single_cached(url: impl Into<Box<str>>) -> Single {
    Single(Entry::new_cached(url.into()))
}

#[derive(Debug)]
pub struct Single(Entry);

impl Single {

    #[inline]
    pub fn url(&self) -> &str {
        self.0.url()
    }

    #[inline]
    pub fn file(&self) -> &Path {
        self.0.file()
    }

    #[inline]
    pub fn set_expected_size(&mut self, size: Option<u32>) -> &mut Self {
        self.0.set_expected_size(size);
        self
    }

    #[inline]
    pub fn set_expected_sha1(&mut self, sha1: Option<[u8; 20]>) -> &mut Self {
        self.0.set_expected_sha1(sha1);
        self
    }

    #[inline]
    pub fn set_keep_open(&mut self) -> &mut Self {
        self.0.set_keep_open();
        self
    }

    #[inline]
    pub fn set_use_cache(&mut self) -> &mut Self {
        self.0.set_use_cache();
        self
    }

    /// Download this singe entry, returning success or error entry depending on the
    /// result.
    /// 
    /// This is internally starting an asynchronous Tokio runtime and block on it, so
    /// this function will just panic if launched inside another runtime!
    #[must_use]
    pub fn download(&mut self, mut handler: impl Handler) -> Result<EntrySuccess, EntryError> {

        let client = crate::http::client()
            .map_err(|e| EntryError { 
                core: self.0.core.clone(), 
                kind: EntryErrorKind::new_reqwest(e),
            })?;

        crate::tokio::sync(download_single(client, &mut handler, &self.0))

    }

}

/// A list of pending download that can be all downloaded at once.
#[derive(Debug)]
pub struct Batch {
    /// All entries to be downloaded.
    entries: Vec<Entry>,
}

impl Batch {

    /// Create a new empty download list.
    #[inline]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Return the total number of entries pushed into this download batch.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if this batch has no entry.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert a new entry to be downloaded in this download batch.
    pub fn push(&mut self, url: impl Into<Box<str>>, file: impl Into<Box<Path>>) -> &mut Entry {
        self.entries.push(Entry::new(url.into(), file.into()));
        self.entries.last_mut().unwrap()
    }

    /// Insert a new entry to be downloaded in this download batch, this entry don't
    /// need a file because it is purely cached and so the file is derived from the URL.
    /// It is constructed from a standard cache directory called `portablemc-cache` 
    /// located in a standard user cache directory (or system tmp as a fallback), 
    /// the file name in that directory is the hash of the URL.
    pub fn push_cached(&mut self, url: impl Into<Box<str>>) -> &mut Entry {
        self.entries.push(Entry::new_cached(url.into()));
        self.entries.last_mut().unwrap()
    }

    pub fn entry(&self, index: usize) -> &Entry {
        &self.entries[index]
    }

    pub fn entry_mut(&mut self, index: usize) -> &mut Entry {
        &mut self.entries[index]
    }

    /// Download this whole batch, the batch is cleared if returning ok. It's left 
    /// untouched if it returns an error and no file is downloaded.
    /// 
    /// This is internally starting an asynchronous Tokio runtime and block on it, so
    /// this function will just panic if launched inside another runtime!
    pub fn download(&mut self, mut handler: impl Handler) -> reqwest::Result<BatchResult> {
        let client = crate::http::client()?;
        let entries = mem::take(&mut self.entries);
        Ok(crate::tokio::sync(download_many(client, &mut handler, 40, entries)))
    }

}

/// Represent the core information of an entry, its URL and the path where it's 
/// downloaded. We put this in its own structure to ensure that these values are always 
/// contiguous and this improves the copy of this structure when actually copied (when
/// moved at assembly level).
#[derive(Debug, Clone)]
struct EntryCore {
    /// The URL to download the file from.
    url: Box<str>,
    /// The file where the downloaded content is written.
    file: Box<Path>,
}

#[derive(Debug)]
pub struct Entry {
    /// Core information.
    core: EntryCore,
    /// Optional expected size of the file.
    expected_size: Option<u32>,
    /// Optional expected SHA-1 of the file.
    expected_sha1: Option<[u8; 20]>,
    /// Use a file next to the entry file to keep track of the last-modified and entity
    /// tag HTTP informations, that will be used in next downloads to actually download
    /// the data only if needed. This means that the entry will not always be downloaded,
    /// and its optional size and SHA-1 will be only checked when actually downloaded.
    /// Also, this implies that if the program has no internet access then it will use
    /// the cached version if existing.
    use_cache: bool,
    /// True to keep the file open after it has been downloaded, and store the handle
    /// in the completed entry.
    keep_open: bool,
}

impl Entry {

    fn new(url: Box<str>, file: Box<Path>) -> Self {
        Self {
            core: EntryCore {
                url,
                file,
            },
            expected_size: None,
            expected_sha1: None,
            use_cache: false,
            keep_open: false,
        }
    }

    fn new_cached(url: Box<str>) -> Self {
        
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

        let mut ret = Self::new(url, file.into_boxed_path());
        ret.set_use_cache();
        ret

    }

    #[inline]
    pub fn url(&self) -> &str {
        &self.core.url
    }

    #[inline]
    pub fn file(&self) -> &Path {
        &self.core.file
    }

    #[inline]
    pub fn expected_size(&self) -> Option<u32> {
        self.expected_size
    }

    #[inline]
    pub fn set_expected_size(&mut self, size: Option<u32>) -> &mut Self {
        self.expected_size = size;
        self
    }

    #[inline]
    pub fn expected_sha1(&self) -> Option<&[u8; 20]> {
        self.expected_sha1.as_ref()
    }

    #[inline]
    pub fn set_expected_sha1(&mut self, sha1: Option<[u8; 20]>) -> &mut Self {
        self.expected_sha1 = sha1;
        self
    }

    /// After the file has been successfully downloaded, keep the handle opened so it
    /// can be retrieved via [`EntrySuccess::handle`] related methods. The file's 
    /// cursor is rewind to the start.
    #[inline]
    pub fn set_keep_open(&mut self) -> &mut Self {
        self.keep_open = true;
        self
    }

    /// Use a file next to the entry file to keep track of the last-modified and entity
    /// tag HTTP informations, that will be used in next downloads to actually download
    /// the data only if needed. This means that the entry will not always be downloaded,
    /// and its optional size and SHA-1 will be only checked when actually downloaded.
    /// Also, this implies that if the program has no internet access then it will use
    /// the cached version if existing.
    /// 
    /// This is usually not needed to call this function, prefer [`Batch::push_cached`].
    #[inline]
    pub fn set_use_cache(&mut self) -> &mut Self {
        self.use_cache = true;
        self
    }

}

/// When a download batch has been downloaded, this returned completed batch contains, 
/// for each entry, it's success or not.
#[derive(Debug)]
pub struct BatchResult {
    /// Each entry's result.
    entries: Box<[Result<EntrySuccess, EntryError>]>,
    /// The index of each entry that has an error.
    errors: Box<[usize]>,
}

impl BatchResult {

    /// Return the total number of entries pushed into this download batch.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if this batch has no entry.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn entry(&self, index: usize) -> Result<&EntrySuccess, &EntryError> {
        self.entries[index].as_ref()
    }

    #[inline]
    pub fn entry_mut(&mut self, index: usize) -> Result<&mut EntrySuccess, &mut EntryError> {
        self.entries[index].as_mut()
    }

    #[inline]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    #[inline]
    pub fn successes_count(&self) -> usize {
        self.entries.len() - self.errors.len()
    }

    #[inline]
    pub fn errors_count(&self) -> usize {
        self.errors.len()
    }

    pub fn iter_successes(&self) -> BatchResultSuccessesIter<'_> {
        BatchResultSuccessesIter {
            entries: self.entries.iter(),
            count: self.successes_count(),
        }
    }

    pub fn iter_errors(&self) -> BatchResultErrorsIter<'_> {
        BatchResultErrorsIter {
            errors: self.errors.iter(),
            entries: &self.entries,
        }
    }

    /// Make this batch result into a result which will be an error if at least one entry
    /// has an error.
    pub fn into_result(self) -> Result<Self, Self> {
        if self.has_errors() {
            Err(self)
        } else {
            Ok(self)
        }
    }

}

/// To allow creation of a batch result from a single download.
impl From<Result<EntrySuccess, EntryError>> for BatchResult {
    fn from(value: Result<EntrySuccess, EntryError>) -> Self {
        Self {
            errors: if value.is_err() { Box::new([0]) } else { Box::new([]) },
            entries: Box::new([value]),
        }
    }
}

impl From<EntrySuccess> for BatchResult {
    fn from(value: EntrySuccess) -> Self {
        Self {
            entries: Box::new([Ok(value)]),
            errors: Box::new([]),
        }
    }
}

impl From<EntryError> for BatchResult {
    fn from(value: EntryError) -> Self {
        Self {
            entries: Box::new([Err(value)]),
            errors: Box::new([0]),
        }
    }
}

/// Iterator for successful 
#[derive(Debug)]
pub struct BatchResultSuccessesIter<'a> {
    entries: std::slice::Iter<'a, Result<EntrySuccess, EntryError>>,
    count: usize,
}

impl FusedIterator for BatchResultSuccessesIter<'_> { }
impl ExactSizeIterator for BatchResultSuccessesIter<'_> { }
impl<'a> Iterator for BatchResultSuccessesIter<'a> {

    type Item = &'a EntrySuccess;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Ok(success) = self.entries.next()? {
                self.count -= 1;
                return Some(success);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }

}

/// Iterator for successful 
#[derive(Debug)]
pub struct BatchResultErrorsIter<'a> {
    errors: std::slice::Iter<'a, usize>,
    entries: &'a [Result<EntrySuccess, EntryError>],
}

impl FusedIterator for BatchResultErrorsIter<'_> { }
impl ExactSizeIterator for BatchResultErrorsIter<'_> { }
impl<'a> Iterator for BatchResultErrorsIter<'a> {

    type Item = &'a EntryError;

    fn next(&mut self) -> Option<Self::Item> {
        let index = *self.errors.next()?;
        Some(self.entries[index].as_ref().unwrap_err())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.errors.size_hint()
    }

}

/// State of a successfully downloaded entry.
#[derive(Debug)]
pub struct EntrySuccess {
    core: EntryCore,
    inner: EntrySuccessInner,
}

#[derive(Debug)]
struct EntrySuccessInner {
    /// The final size of the downloaded entry.
    size: u32,
    /// The final SHA-1 of the downloaded entry.
    sha1: [u8; 20],
    /// Optional handle to the opened file, in case `keep_open` option was enabled.
    handle: Option<std::fs::File>,
}

impl EntrySuccess {

    #[inline]
    pub fn url(&self) -> &str {
        &self.core.url
    }

    #[inline]
    pub fn file(&self) -> &Path {
        &self.core.file
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.inner.size
    }

    #[inline]
    pub fn sha1(&self) -> &[u8; 20] {
        &self.inner.sha1
    }

    /// If the entry was configured with `keep_open` option then it should return some
    /// file handle.
    #[inline]
    pub fn handle(&self) -> Option<&std::fs::File> {
        self.inner.handle.as_ref()
    }

    /// If the entry was configured with `keep_open` option then it should return some
    /// file handle through mutable ref.
    #[inline]
    pub fn handle_mut(&mut self) -> Option<&mut std::fs::File> {
        self.inner.handle.as_mut()
    }

    /// If the entry was configured with `keep_open` option then it should return some
    /// file handle, once, and after this any `handle_` method will return none.
    #[inline]
    pub fn take_handle(&mut self) -> Option<std::fs::File> {
        self.inner.handle.take()
    }

    /// Take the internal handle if the entry was configured with `keep_open` option, and
    /// read the entire file to a string.
    /// 
    /// For now internal because it's being tested...
    pub(crate) fn read_handle_to_string(&mut self) -> Option<io::Result<String>> {
        let mut buf = String::new();
        match self.take_handle()?.read_to_string(&mut buf) {
            Ok(_) => Some(Ok(buf)),
            Err(e) => Some(Err(e)),
        }
    }

}

/// State of an entry that failed to download, it also acts as a standard error type.
#[derive(thiserror::Error, Debug)]
#[error("{core:?}: {kind}")]
pub struct EntryError {
    core: EntryCore,
    kind: EntryErrorKind,
}

/// An error for a single entry.
#[derive(thiserror::Error, Debug)]
pub enum EntryErrorKind {
    /// Invalid size of the fully downloaded entry compared to the expected size.
    /// Implies that [`Entry::set_expected_size`] is not none.
    #[error("invalid size")]
    InvalidSize,
    /// Invalid SHA-1 of the fully downloaded entry compared to the expected SHA-1.
    /// Implies that [`Entry::set_expected_sha1`] is not none.
    #[error("invalid sha1")]
    InvalidSha1,
    /// Invalid HTTP status code while requesting the entry.
    #[error("invalid status: {0}")]
    InvalidStatus(u16),
    /// A generic error type for internal and third-party errors that may change depending
    /// on the actual implementation.
    /// 
    /// The current implementation yields the following error types:
    /// 
    /// - [`std::io::Error`] for any I/O error related to opening and writing local files.
    /// 
    /// - [`reqwest::Error`] for any error related to HTTP requests.
    #[error("internal: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl EntryErrorKind {

    #[inline]
    fn new_io(e: io::Error) -> Self {
        Self::Internal(Box::new(e))
    }

    #[inline]
    fn new_reqwest(e: reqwest::Error) -> Self {
        Self::Internal(Box::new(e))
    }

}

impl EntryError {

    #[inline]
    pub fn url(&self) -> &str {
        &self.core.url
    }

    #[inline]
    pub fn file(&self) -> &Path {
        &self.core.file
    }

    #[inline]
    pub fn kind(&self) -> &EntryErrorKind {
        &self.kind
    }

}

crate::trait_event_handler! {
    /// A handle for watching a batch download progress.
    pub trait Handler {
        /// Notification of a download progress, the download should be considered done when
        /// 'count' is equal to 'total_count'. This is called anyway at the beginning and at 
        /// the end of the download. Note that the final given 'size' may be greater than
        /// 'total_size' in case of unknown expected size, which 'total_size' is the sum.
        fn progress(count: u32, total_count: u32, size: u32, total_size: u32);
    }
}

/// Internal split of the download_impl function without reqwest initialization error.
#[inline]
async fn download_many(
    client: Client,
    handler: &mut dyn Handler,
    concurrent_count: usize,
    entries: Vec<Entry>,
) -> BatchResult {

    // Make it constant and sharable between all tasks.
    let entries = Arc::new(entries);

    // Collect the index of each pending entry, we also keep the expected size for 
    // sorting and total size. We do this to avoid loosing the original entries order.
    let mut indices = (0..entries.len()).collect::<Vec<_>>();

    // Sort our entries in order to download big files first, this is allowing better
    // parallelization at start and avoid too much blocking at the end. Because our
    // indices vector will pop the first index from the end, we put big files at the
    // end, and so sort by ascending size.
    indices.sort_by(|&a_index, &b_index| {
        match (entries[a_index].expected_size, entries[b_index].expected_size) {
            (Some(a), Some(b)) => Ord::cmp(&a, &b),
            _ => Ordering::Equal,
        }
    });

    // Current downloaded size and total size.
    let mut size = 0;
    let total_size = indices.iter()
        .map(|&index| entries[index].expected_size.unwrap_or(0))
        .sum::<u32>();

    // Send a progress update for each 1000 parts of the download.
    let progress_size_interval = total_size / 1000;
    let mut last_size = 0u32;

    handler.progress(0, entries.len() as u32, size, total_size);

    let mut completed = 0;
    let mut futures = JoinSet::new();

    let (
        progress_tx, 
        mut progress_rx,
    ) = mpsc::channel(concurrent_count * 2);

    let mut results = (0..entries.len()).map(|_| None).collect::<Vec<_>>();

    // If we have theoretically completed all downloads, we still wait for joining all
    // remaining futures in the join set.
    while completed < entries.len() || !futures.is_empty() {
        
        while futures.len() < concurrent_count && !indices.is_empty() {
            futures.spawn(download_many_entry(
                client.clone(), 
                Arc::clone(&entries),
                indices.pop().unwrap(),  // Safe because not empty.
                progress_tx.clone()));
        }

        let mut force_progress = false;

        tokio::select! {
            Some(res) = futures.join_next() => {
                let (index, res) = res.expect("task should not be cancelled nor panicking");
                completed += 1;
                force_progress = true;
                let prev_res = results[index].replace(res);
                debug_assert!(prev_res.is_none());
            }
            Some(progress) = progress_rx.recv() => {
                size += progress as u32;
            }
            else => {
                // Just ignore, because it's invalid state, in case of join_next we 
                // ignore if JoinSet is empty because we rely mostly 'completed'.
                // For the queue receive, we know that the other end will never be fully
                // closed because we locally own both 'tx' and 'rx'.
                continue;
            }
        };
        
        if force_progress || size - last_size >= progress_size_interval {
            handler.progress(completed as u32, entries.len() as u32, size, total_size);
            last_size = size;
        }

    }

    // Ensure that all tasks are aborted, this allows us to take back ownership of the 
    // underlying vector of entries.
    assert!(futures.is_empty());

    // Now that every task has terminated we should be able to take back the entries.
    let entries = Arc::into_inner(entries).unwrap();
    let mut ret_entries = Vec::with_capacity(entries.len());
    let mut ret_errors = Vec::new();

    for (entry, res) in entries.into_iter().zip(results) {
        let res = res.expect("all entries should have a result");
        if res.is_err() {
            ret_errors.push(ret_entries.len());
        }
        ret_entries.push(match res {
            Ok(inner) => Ok(EntrySuccess { core: entry.core, inner }),
            Err(kind) => Err(EntryError { core: entry.core, kind }),
        });
    }

    BatchResult {
        entries: ret_entries.into_boxed_slice(),
        errors: ret_errors.into_boxed_slice(),
    }

}

/// Download entrypoint for a download, this is a wrapper around core download
/// function in order to easily catch the result and send it as an event.
async fn download_many_entry(
    client: Client, 
    entries: Arc<Vec<Entry>>,
    index: usize,
    progress_sender: mpsc::Sender<u32>,
) -> (usize, Result<EntrySuccessInner, EntryErrorKind>) {

    let progress_sender = ManyEntryProgressSender {
        sender: progress_sender,
    };

    (index, download_entry(client, &entries[index], progress_sender).await)

}

async fn download_single(
    client: Client,
    handler: &mut dyn Handler,
    entry: &Entry,
) -> Result<EntrySuccess, EntryError> {

    let mut size = 0u32;
    let total_size = entry.expected_size.unwrap_or(0);

    handler.progress(0, 1, 0, total_size);

    let progress_sender = SingleEntryProgressSender {
        handler: &mut *handler,
        size: &mut size,
        total_size,
    };

    let res = download_entry(client, entry, progress_sender).await;

    handler.progress(1, 1, size, total_size);

    match res {
        Ok(inner) => Ok(EntrySuccess { core: entry.core.clone(), inner }),
        Err(kind) => Err(EntryError { core: entry.core.clone(), kind }),
    }

}

/// Internal function to download a single download entry, returning a result with an
/// optional handle to the std file, if keep open parameter is enabled on the entry.
async fn download_entry(
    client: Client, 
    entry: &Entry,
    progress_sender: impl EntryProgressSender,
) -> Result<EntrySuccessInner, EntryErrorKind> {

    let mut progress_sender = progress_sender;

    let mut req = client.get(&*entry.core.url);
    
    // If we are in cache mode, then we derive the file name.
    let cache_file = entry.use_cache.then(|| {
        entry.core.file.to_path_buf().appended(".cache")
    });

    // If we are in cache mode, try checking the file, if the file is locally valid.
    let mut cache = None;
    if let Some(cache_file) = cache_file.as_deref() {
        cache = check_download_cache(&entry.core.file, cache_file).await
            .map_err(EntryErrorKind::new_io)?;
    }

    // Then we add corresponding request headers for cache control.
    if let Some((_, cache_meta)) = &cache {
        if let Some(etag) = cache_meta.etag.as_deref() {
            req = req.header(header::IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = cache_meta.last_modified.as_deref() {
            req = req.header(header::IF_MODIFIED_SINCE, last_modified);
        }
    }

    // If it's a connection error just use the cached copy.
    let mut res = match req.send().await {
        Ok(res) => res,
        Err(e) if cache.is_some() && (e.is_timeout() || e.is_request() || e.is_connect()) => {
            // Using cache in case of network error.
            let (handle, cache_meta) = cache.unwrap();
            return Ok(EntrySuccessInner { 
                size: cache_meta.size, 
                sha1: cache_meta.sha1.0,
                handle: entry.keep_open.then_some(handle),
            });
        }
        Err(e) => {
            // Other unhandled errors are returned and will be present in errored entries.
            return Err(EntryErrorKind::new_reqwest(e));
        }
    };

    // Checking if the status is not OK, if this is a NOT_MODIFIED then we returned the
    // file as-is, with the handle if keep open is requested.
    if res.status() == StatusCode::NOT_MODIFIED && cache.is_some() {
        let (handle, cache_meta) = cache.unwrap();
        return Ok(EntrySuccessInner { 
            size: cache_meta.size, 
            sha1: cache_meta.sha1.0,
            handle: entry.keep_open.then_some(handle),
        });
    } else if res.status() != StatusCode::OK {
        return Err(EntryErrorKind::InvalidStatus(res.status().as_u16()));
    }

    // Close the possible cached file because we'll need to create it just below. 
    drop(cache);

    // Create any parent directory so that we can create the file.
    if let Some(parent) = entry.core.file.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(EntryErrorKind::new_io)?;
    }

    // Only add read capability if the handle needs to be kept.
    let mut dst = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .read(entry.keep_open)
        .open(&*entry.core.file).await
        .map_err(EntryErrorKind::new_io)?;

    let mut size = 0usize;
    let mut sha1 = Sha1::new();
    
    while let Some(chunk) = res.chunk().await.map_err(EntryErrorKind::new_reqwest)? {

        let delta = chunk.len();
        size += delta;

        AsyncWriteExt::write_all(&mut dst, &chunk).await.map_err(EntryErrorKind::new_io)?;
        Write::write_all(&mut sha1, &chunk).map_err(EntryErrorKind::new_io)?;

        progress_sender.send(delta as u32).await;

    }

    // Ensure the file is fully written.
    dst.flush().await.map_err(EntryErrorKind::new_io)?;

    // Now check required size and SHA-1.
    let size = u32::try_from(size).map_err(|_| EntryErrorKind::InvalidSize)?;
    let sha1 = sha1.finalize();

    if let Some(expected_size) = entry.expected_size {
        if expected_size != size {
            return Err(EntryErrorKind::InvalidSize);
        }
    }

    if let Some(expected_sha1) = &entry.expected_sha1 {
        if expected_sha1 != sha1.as_slice() {
            return Err(EntryErrorKind::InvalidSha1);
        }
    }

    // If we have a cache file, write it.
    if let Some(cache_file) = cache_file.as_deref() {

        let etag = res.headers().get(header::ETAG)
            .and_then(|h| h.to_str().ok().map(str::to_string));

        let last_modified = res.headers().get(header::LAST_MODIFIED)
            .and_then(|h| h.to_str().ok().map(str::to_string));

        // Only write the cache file if relevant!
        if etag.is_some() || last_modified.is_some() {

            let cache_meta_writer = File::create(cache_file).await.map_err(EntryErrorKind::new_io)?;
            let cache_meta_writer = BufWriter::new(cache_meta_writer.into_std().await);

            let res = serde_json::to_writer(cache_meta_writer, &serde::CacheMeta {
                url: entry.core.url.to_string(),
                size,
                sha1: crate::serde::HexString(sha1.into()),
                etag,
                last_modified,
            });

            // Silently ignore errors by we remove the file if it happens.
            if res.is_err() {
                let _ = fs::remove_file(cache_file).await;
            }

        }

    }

    let handle;
    if entry.keep_open {
        let mut file = dst.into_std().await;
        file.rewind().map_err(EntryErrorKind::new_io)?;
        handle = Some(file);
    } else {
        handle = None;
    }

    Ok(EntrySuccessInner {
        size,
        sha1: sha1.into(),
        handle,
    })

}

/// Given a file and its cache file, return the cache metadata only if the file is 
/// existing and the file has not been modified (size and SHA-1). 
/// 
/// The opened file handle is also returned with the metadata, this avoids running into 
/// race conditions by closing and reopening the file. The returned file handle is
/// writeable and its position is set to 0.
async fn check_download_cache(file: &Path, cache_file: &Path) -> io::Result<Option<(std::fs::File, serde::CacheMeta)>> {

    // Start by reading the cache metadata associated to this file.
    let cache = match File::open(cache_file).await {
        Ok(file) => serde_json::from_reader::<_, serde::CacheMeta>(file.into_std().await).ok(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };

    let Some(cache) = cache else {
        return Ok(None);
    };

    // NOTE: We open the file with write permission so that it can be used when returned.
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

    reader.rewind().await?;

    // Then we check SHA-1...
    let mut reader = reader.into_std().await;
    let mut digest = Sha1::new();
    io::copy(&mut reader, &mut digest)?;
    if cache.sha1.0 != digest.finalize().as_slice() {
        return Ok(None);
    }

    reader.rewind()?;

    Ok(Some((reader, cache)))

}

/// Internal abstract progress sender that support sending the progress into a 
trait EntryProgressSender {
    async fn send(&mut self, delta: u32);
}

/// Implementation of the progress sender for the `download_many` function with channel.
struct ManyEntryProgressSender {
    sender: mpsc::Sender<u32>,
}

impl EntryProgressSender for ManyEntryProgressSender {
    async fn send(&mut self, delta: u32) {
        self.sender.send(delta).await.unwrap();
    }
}

/// A progress sender specialized when downloading a single progress, we can therefore
/// directly send any progress directly to the handler!
struct SingleEntryProgressSender<'a> {
    handler: &'a mut dyn Handler,
    size: &'a mut u32,
    total_size: u32,
}

impl EntryProgressSender for SingleEntryProgressSender<'_> {
    async fn send(&mut self, delta: u32) {
        *self.size += delta;
        self.handler.progress(0, 1, *self.size, self.total_size);
    }
}

/// Internal module for serde of cache metadata file.
mod serde {

    use crate::serde::HexString;

    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    pub struct CacheMeta {
        /// The full URL of the cached resource, just for information.
        pub url: String,
        /// Size of the cached file, used to verify its validity.
        pub size: u32,
        /// SHA-1 hash of the cached file, used to verify its validity. 
        pub sha1: HexString<20>,
        /// The ETag if present.
        pub etag: Option<String>,
        /// Last modified data if present.
        pub last_modified: Option<String>,
    }

}
