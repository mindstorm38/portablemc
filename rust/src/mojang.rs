//! Mojang manifest backed installer.

use crate::standard::{ Environment, Handler, Installer, Result, Version};



pub struct MojangInstaller {
    inner: Installer,
}

impl MojangInstaller {

    pub fn new() -> Option<Self> {
        Some(Self {
            inner: Installer::new()?,
        })
    }

    pub fn install(&self, version: &str, handler: &mut dyn Handler) -> Result<Environment> {
        self.inner.install(version, &mut MojangHandler { inner: handler })
    }

}


/// This handler automatically downloads and install missing version metadata.
struct MojangHandler<'a> {
    inner: &'a mut dyn Handler,
}

impl Handler for MojangHandler<'_> {
    
    fn filter_version(&mut self, installer: &Installer, version: &mut Version) -> Result<bool> {
        // FIXME: Verify checksum and size
        let _ = (installer, version);
        Ok(true)
    }

    fn fetch_version(&mut self, installer: &Installer, version: &str) -> Result<Version> {
        // FIXME: Fetch from manifest
        let _ = (installer, version);
        todo!()
    }

}
