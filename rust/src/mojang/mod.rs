//! Extension to the standard installer with verification and installation of missing
//! Mojang versions.

pub mod serde;

use std::io::BufReader;
use std::fs::{self, File};

use crate::standard::{self, check_file, Handler as _};
use crate::download::{self, Entry, EntrySource};


/// Static URL to the version manifest provided by Mojang.
const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";


/// An installer for Mojang-provided versions.
#[derive(Debug)]
pub struct Installer {
    /// The underlying standard installer logic.
    pub installer: standard::Installer,
}

impl Installer {

    /// Install the given Mojang version from its identifier. This also supports alias
    /// identifiers such as "release" and "snapshot" that will be resolved, note that
    /// these identifiers are just those presents in the "latest" mapping of the
    /// Mojang versions manifest. 
    /// 
    /// If the given version is not found in the manifest then it's silently ignored and
    /// the version metadata must already exists.
    pub fn install(&self, mut handler: impl Handler, id: &str) -> standard::Result<standard::Installed> {
        
        // We quickly lock and ensure that the manifest is present here because it will
        // always be used, first for resolving potential alias id, and then to check an
        // existing version's metadata file's hash or to download missing version.
        let manifest = self.request_manifest(&mut handler)?;
        
        let id = match manifest.latest.get(id) {
            Some(alias_id) => alias_id.as_str(),
            None => id,
        };

        // If the version is not found in manifest we just fallback to basic installer.
        let Some(manifest_version) = manifest.versions.iter()
            .find(|v| v.id == id) else {
                return self.installer.install(handler.as_standard_dyn(), id);
            };

        let mut handler = InternalHandler {
            inner: handler,
            id,
            manifest_version,
            error: None,
        };

        // Same as above, we are giving a &mut dyn ref to avoid huge monomorphization.
        let res = self.installer.install(handler.as_standard_dyn(), id);
        
        match handler.error {
            Some(e) => Err(e),
            None => res,
        }

    }

    /// Request the Mojang versions' manifest with the currently configured cache file.
    pub fn request_manifest(&self, mut handler: impl Handler) -> standard::Result<serde::MojangManifest> {
        
        let entry = Entry::new_cached(VERSION_MANIFEST_URL);
        let file = entry.file.to_path_buf();
        entry.download(handler.as_download_dyn())?;

        let reader = match File::open(&file) {
            Ok(reader) => BufReader::new(reader),
            Err(e) => return Err(standard::Error::new_io_file(e, file)),
        };

        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let manifest: serde::MojangManifest = match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(obj) => obj,
            Err(e) => return Err(standard::Error::new_json_file(e, file))
        };

        Ok(manifest)

    }

}

impl From<standard::Installer> for Installer {
    fn from(value: standard::Installer) -> Self {
        Self {
            installer: value,
        }
    }
}

/// Handler for events happening when installing.
pub trait Handler: standard::Handler {

    /// Handle an even from the mojang installer.
    fn handle_mojang_event(&mut self, event: Event);

    fn as_mojang_dyn(&mut self) -> &mut dyn Handler 
    where Self: Sized {
        self
    }

}

/// Blanket implementation that does nothing.
impl Handler for () {
    fn handle_mojang_event(&mut self, event: Event) {
        let _ = event;
    }
}

impl<H: Handler + ?Sized> Handler for  &'_ mut H {
    fn handle_mojang_event(&mut self, event: Event) {
        (*self).handle_mojang_event(event)
    }
}

/// An event produced by the installer that can be handled by the install handler.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// When the required Mojang version is being loaded (VersionLoading) but the file
    /// has an invalid size or SHA-1 and has been removed in order to download an 
    /// up-to-date version from the manifest.
    MojangVersionInvalidated {
        id: &'a str,
    },
    /// The required Mojang version metadata is missing and so will be fetched.
    MojangVersionFetching {
        id: &'a str,
    },
    /// The mojang version has been fetched.
    MojangVersionFetched {
        id: &'a str,
    },
}


/// Internal handler given to the standard installer.
struct InternalHandler<'a, H: Handler> {
    /// Inner standard handler.
    inner: H,
    /// The identifier of the Mojang version to launch.
    id: &'a str,
    /// The manifest version of the Mojang version to launch.
    manifest_version: &'a serde::MojangManifestVersion,
    /// If there is an error in the handler.
    error: Option<standard::Error>,
}

impl<H: Handler> download::Handler for InternalHandler<'_, H> {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        self.inner.handle_download_progress(count, total_count, size, total_size)
    }
}

impl<H: Handler> standard::Handler for InternalHandler<'_, H> {
    fn handle_standard_event(&mut self, event: standard::Event) {
        self.error = self.handle_standard_event_inner(event).err();
    }
}

impl<H: Handler> InternalHandler<'_, H> {

    fn handle_standard_event_inner(&mut self, event: standard::Event) -> standard::Result<()> {

        match event {
            // In this case we check the version hash just before loading it, if the hash
            // is wrong we delete the version and so the next event will be that version
            // is not found as handled below.
            standard::Event::VersionLoading { 
                id, 
                file
            } if id == self.id => {

                self.inner.handle_standard_event(event);
                
                let dl = &self.manifest_version.download;
                if !check_file(file, dl.size, dl.sha1.as_deref())? {
                    fs::remove_file(file).map_err(|e| standard::Error::new_io_file(e, file.to_path_buf()))?;
                    self.inner.handle_mojang_event(Event::MojangVersionInvalidated { id });
                }

            }
            // In this case we handle a missing version, by finding it in the manifest.
            standard::Event::VersionNotFound { 
                id, 
                file, 
                error: _, 
                retry 
            } if id == self.id => {

                self.inner.handle_mojang_event(Event::MojangVersionFetching { id });
                
                EntrySource::from(&self.manifest_version.download)
                    .with_file(file.to_path_buf())
                    .download(&mut self.inner)?;

                self.inner.handle_mojang_event(Event::MojangVersionFetched { id });

                // Retry only if no preceding error.
                *retry = true;

            }
            _ => self.inner.handle_standard_event(event),
        }

        Ok(())

    }
    
}
