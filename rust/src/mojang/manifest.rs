//! Optionally cached Mojang manifest.

use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;

use tokio::fs::File;
use tokio::runtime::Builder;

use super::serde;
use crate::http;
use crate::mojang::serde::PmcMojangManifest;


/// Static URL to the version manifest provided by Mojang.
const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";


/// The version manifest is an API to query official Mojang versions and where to 
/// download their .
#[derive(Debug)]
pub struct MojangManifest {
    /// The Mojang version manifest can be cached in the filesystem. This can be useful
    /// because the only API to request a version JSON file is to query this enormous
    /// manifest file.
    cache_file: Option<PathBuf>,
    /// Cached deserialized data of the manifest.
    data: Option<serde::PmcMojangManifest>,
}

impl MojangManifest {

    pub fn new() -> Self {
        Self {
            cache_file: None,
            data: None,
        }
    }

    /// Get the manifest data, if already memory cached.
    pub fn get(&self) -> Option<&serde::MojangManifest> {
        self.data.as_ref().map(|data| &data.inner)
    }

    /// Ensure that the manifest data has been memory cached and returns it.
    pub fn ensure(&mut self) -> io::Result<&serde::MojangManifest> {

        let rt = Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(self.ensure_impl())

    }

    async fn ensure_impl(&mut self) -> io::Result<&serde::MojangManifest> {

        // Doing so to avoid borrowing issues.
        if self.data.is_some() {
            return Ok(&self.data.as_ref().unwrap().inner);
        }

        if let Some(cache_file) = self.cache_file.as_deref() {

            // Using a loop for using early breaks.
            loop {

                let cache_reader = match File::open(cache_file).await {
                    Ok(reader) => BufReader::new(reader.into_std().await),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                    Err(e) => return Err(e),
                };
                
                // Silently ignoring any parsing error.
                self.data = serde_json::from_reader(cache_reader).ok();
                break;

            }

        }

        let client = http::builder()
            .build()
            .unwrap(); // FIXME:

        let mut req = client.get(VERSION_MANIFEST_URL);

        // If the last modified date is missing, we don't add this header so we request
        // the data anyway.
        if let Some(last_modified) = self.data.as_ref().and_then(|m| m.last_modified.as_deref()) {
            req = req.header(reqwest::header::IF_MODIFIED_SINCE, last_modified);
        }

        let res = req.send()
            .await
            .unwrap();

        // This status code implies that we previously set "last modified" header and so
        // that the data is existing.
        if res.status() == reqwest::StatusCode::NOT_MODIFIED {
            if self.data.is_none() {
                return Err(io::ErrorKind::InvalidData.into());
            }
            return Ok(&self.data.as_ref().unwrap().inner);
        }

        let last_modified = res.headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|val| val.to_str().ok())
            .map(|val| val.to_string());

        let manifest = res.json::<serde::MojangManifest>()
            .await
            .unwrap();

        let data = self.data.insert(PmcMojangManifest {
            inner: manifest,
            last_modified,
        });

        // If there is a last modified, write the data to the cache file. If not there
        // is no point in writing it because this will always request again if the last
        // modified date.
        if data.last_modified.is_some() {
            if let Some(cache_file) = self.cache_file.as_deref() {

                let cache_file = File::create(cache_file)
                    .await?
                    .into_std()
                    .await;

                let cache_writer = BufWriter::new(cache_file);
                serde_json::to_writer(cache_writer, data).unwrap(); // FIXME:

            }
        }

        Ok(&data.inner)

    }

}
