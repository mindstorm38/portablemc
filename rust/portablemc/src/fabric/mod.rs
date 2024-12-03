//! Extension to the Mojang installer to support fetching and installation of 
//! Fabric-related mod loader versions.

pub mod serde;

use std::path::PathBuf;

use reqwest::Client;

use crate::download;
use crate::standard;
use crate::mojang;

pub use mojang::{Root, Game};


/// This is the original and official Fabric API.
pub static FABRIC_API: Api = Api {
    base_url: "https://meta.fabricmc.net/v2",
};

/// This is the API for the Quilt mod loader, which is a fork of Fabric.
pub static QUILT_API: Api = Api {
    base_url: "https://meta.quiltmc.org/v3",
};

/// This is the API for the LegacyFabric project which aims to backport the Fabric loader
/// to older versions, up to 1.14 snapshots.
pub static LEGACY_FABRIC_API: Api = Api {
    base_url: "https://meta.legacyfabric.net/v2",
};

/// This is the API for the LegacyFabric project which aims to backport the Fabric loader
/// to older versions, up to 1.14 snapshots.
pub static BABRIC_API: Api = Api {
    base_url: "https://meta.babric.glass-launcher.net/v2",
};

/// An installer for supporting mod loaders that are Fabric or like it (Quilt, 
/// LegacyFabric, Babric). The generic parameter is used to specify the API to use.
#[derive(Debug, Clone)]
pub struct Installer {
    /// The underlying Mojang installer logic.
    mojang: mojang::Installer,
    /// Inner installer data, put in a sub struct to fix borrow issue.
    inner: InstallerInner,
}

/// Internal installer data.
#[derive(Debug, Clone)]
struct InstallerInner {
    api: &'static Api,
    root: Root,
    loader: Loader,
}

impl Installer {

    /// Create a new installer with default configuration.
    pub fn new(main_dir: impl Into<PathBuf>, api: &'static Api) -> Self {
        Self {
            mojang: mojang::Installer::new(main_dir),
            inner: InstallerInner {
                api,
                root: Root::Release,
                loader: Loader::Latest,
            }
        }
    }

    /// Same as [`Self::new`] but using the default main directory in your system,
    /// returning none if there is no default main directory on your system.
    pub fn new_with_default(api: &'static Api) -> Option<Self> {
        Some(Self::new(standard::default_main_dir()?, api))
    }

    /// Execute some callback to alter the mojang installer.
    /// 
    /// *Note that the `root` and `fetch` property will be overwritten when installing.*
    #[inline]
    pub fn with_mojang<F>(&mut self, func: F) -> &mut Self
    where
        F: FnOnce(&mut mojang::Installer) -> &mut mojang::Installer,
    {
        func(&mut self.mojang);
        self
    }

    /// By default, this Fabric installer targets the latest release version. To also
    /// change the fabric loader's version to use, see [`Self::loader`]. 
    /// 
    /// If this root version is an alias (`Release` the default, or `Snapshot`), it will 
    /// require the online version manifest, if the alias is not found in the manifest 
    /// (which is an issue on Mojang's side) then a 
    /// [`mojang::Error::RootAliasNotFound`] is returned.
    pub fn root(&mut self, root: impl Into<Root>) {
        self.inner.root = root.into();
    }

    /// By default, this Fabric installer targets the latest loader version compatible
    /// with the root version, use this function to override the loader version to use.
    pub fn loader(&mut self, loader: impl Into<Loader>) {
        self.inner.loader = loader.into();
    }

    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {

        let Self {
            ref mut mojang,
            ref inner,
        } = self;

        // We need to resolve the root version ahead of the Mojang version.
        let alias = match self.inner.root {
            Root::Release => Some(standard::serde::VersionType::Release),
            Root::Snapshot => Some(standard::serde::VersionType::Snapshot),
            _ => None,
        };

        // // If we need an alias then we need to load the manifest.
        // let id = if let Some(alias) = alias {
        //     mojang::request_manifest(handler.as_download_dyn())?
        //         .latest.get(&alias)
        //         .cloned()
        //         .ok_or_else(|| mojang::Error::RootAliasNotFound { root: self.inner.root.clone() })?
        // } else {
        //     match self.inner.root {
        //         Root::Id(ref new_id) => new_id.clone(),
        //         _ => unreachable!(),
        //     }
        // };

        // let versions = inner.api.request_game_versions(handler.as_download_dyn())?;

        todo!()

    }

}

/// A fabric-compatible API.
#[derive(Debug)]
pub struct Api {
    /// Base URL for that API, not ending with a '/'. This API must support the following
    /// endpoints supporting the same API as official Fabric API: 
    /// `/versions/game`, `/versions/loader`, `/versions/loader/<game_version>` and 
    /// `/versions/loader/<game_version>/<loader_loader>/profile/json`.
    pub base_url: &'static str,
}

impl Api {

    /// Request supported game versions, using the local cache to support offline 
    /// starting with .
    fn request_game_versions(&self, client: &Client) -> Result<Vec<serde::Game>> {
        crate::tokio::sync(async move {
            let res = client.get(format!("{}/versions/game", self.base_url))
                .send().await?
                .error_for_status()?
                .json::<Vec<serde::Game>>().await?;
            Ok(res)
        })
    }

    /// Request supported game versions, using the local cache to support offline 
    /// starting with .
    fn request_loader_versions(&self, client: &Client) -> Result<Vec<serde::Loader>> {
        crate::tokio::sync(async move {
            let res = client.get(format!("{}/versions/loader", self.base_url))
                .send().await?
                .error_for_status()?
                .json::<Vec<serde::Loader>>().await?;
            Ok(res)
        })
    }

}

/// Handler for events happening when installing.
pub trait Handler: mojang::Handler {

    /// Handle an even from the mojang installer.
    fn handle_fabric_event(&mut self, event: Event) {
        let _ = event;
    }

    fn as_fabric_dyn(&mut self) -> &mut dyn Handler 
    where Self: Sized {
        self
    }

}

/// Blanket implementation that does nothing.
impl Handler for () { }

impl<H: Handler + ?Sized> Handler for  &'_ mut H {
    fn handle_fabric_event(&mut self, event: Event) {
        (*self).handle_fabric_event(event)
    }
}

/// An event produced by the installer that can be handled by the install handler.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Mojang(#[source] mojang::Error),
    /// A loader latest or specific version has not been found for the root version.
    #[error("loader not found: {loader:?}")]
    LoaderNotFound {
        root: String,
        loader: Loader,
    },
}

impl<T: Into<mojang::Error>> From<T> for Error {
    fn from(value: T) -> Self {
        Self::Mojang(value.into())
    }
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Specify the root version to start with Mojang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loader {
    /// Use the latest loader version for the root version.
    Latest,
    /// Use the specific
    Version(String),
}

/// An impl so that we can give string-like objects to the builder.
impl<T: Into<String>> From<T> for Loader {
    fn from(value: T) -> Self {
        Self::Version(value.into())
    }
}

// ========================== //
// Following code is internal //
// ========================== //

/// Internal handler given to the standard installer.
struct InternalHandler<'a, H: Handler> {
    /// Inner handler.
    inner: &'a mut H,
    /// Back-reference to the installer to know its configuration.
    installer: &'a InstallerInner,
    /// If there is an error in the handler.
    error: Result<()>,
}

impl<H: Handler> download::Handler for InternalHandler<'_, H> {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        self.inner.handle_download_progress(count, total_count, size, total_size)
    }
}

impl<H: Handler> standard::Handler for InternalHandler<'_, H> {
    fn handle_standard_event(&mut self, event: standard::Event) { 
        self.error = self.handle_standard_event_inner(event);
    }
}

impl<H: Handler> InternalHandler<'_, H> {

    fn handle_standard_event_inner(&mut self, event: standard::Event) -> Result<()> { 
        
        match event {
            standard::Event::VersionNotFound { id, file, error, retry } => {
                todo!()
            }
            _ => self.inner.handle_standard_event(event),
        }

        Ok(())

    }
    
}
