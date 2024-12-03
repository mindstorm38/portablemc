//! Extension to the standard installer with verification and installation of missing
//! Mojang versions.

use std::path::PathBuf;

use crate::standard::{Installer, Handler, Event, Result};


const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";


/// An installer for Mojang-provided versions.
#[derive(Debug)]
pub struct MojangInstaller {
    /// The underlying standard installer logic.
    pub installer: Installer,
    /// The Mojang version manifest can be cached in the filesystem. This can be useful
    /// because the only API to request a version JSON file is to query this enormous
    /// manifest file.
    pub manifest_cache_file: Option<PathBuf>,
}

impl MojangInstaller {

    /// Install the given Mojang version from its identifier.
    pub fn install(&self, handler: &mut dyn Handler, id: &str) -> Result<()> {
        
        let mut handler = InternalHandler {
            handler,
        };

        self.installer.install(&mut handler, id)

    }

}

/// Internal handler wrapper for properly 
struct InternalHandler<'a> {
    handler: &'a mut dyn Handler,
}

impl Handler for InternalHandler<'_> {

    fn handle(&mut self, installer: &Installer, event: Event) -> Result<()> {
        
        match event {
            // When loading a version, if 
            Event::VersionLoading { id, file } => {

            }
            _ => {}
        }

        self.handler.handle(installer, event)
        
    }

}
