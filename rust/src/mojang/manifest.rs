//! Optionally cached Mojang manifest.

use std::path::PathBuf;
use std::fs::File;

use crate::standard::{Result, Error};
use super::serde;


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
    data: Option<serde::MojangManifest>,
}

impl MojangManifest {

    pub fn new() -> Self {
        Self {
            cache_file: None,
            data: None,
        }
    }

    pub fn get(&mut self) -> &serde::MojangManifest {

        if let Some(data) = &self.data {
            return data;
        }

        if let Some(cache_file) = self.cache_file.as_deref() {
            
            // let file = File::open(cache_file)
            //     .map_err(|e| )

        }

        todo!()

    }

}
