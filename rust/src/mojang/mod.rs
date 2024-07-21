//! Extension to the standard installer with verification and installation of missing
//! Mojang versions.

pub mod serde;
mod manifest;

use crate::standard::{Installer, Handler, Event, Result};
use crate::http;

pub use manifest::MojangManifest;


/// An installer for Mojang-provided versions.
#[derive(Debug)]
pub struct MojangInstaller {
    /// The underlying standard installer logic.
    pub installer: Installer,
    /// Underlying version manifest.
    pub manifest: MojangManifest,
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

/// Internal handler wrapper.
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
