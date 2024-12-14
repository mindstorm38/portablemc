//! Extension to the Mojang installer to support fetching and installation of 
//! Fabric-related mod loader versions.

pub(crate) mod serde;

use std::path::PathBuf;
use core::fmt;

use reqwest::StatusCode;

use crate::mojang::{self, Handler as _, RootVersion};
use crate::download;
use crate::standard;

pub use mojang::Game;


/// This is the original and official Fabric API.
pub static FABRIC_API: Api = Api {
    base_url: "https://meta.fabricmc.net/v2",
    default_prefix: "fabric",
};

/// This is the API for the Quilt mod loader, which is a fork of Fabric.
pub static QUILT_API: Api = Api {
    base_url: "https://meta.quiltmc.org/v3",
    default_prefix: "quilt",
};

/// This is the API for the LegacyFabric project which aims to backport the Fabric loader
/// to older versions, up to 1.14 snapshots.
pub static LEGACY_FABRIC_API: Api = Api {
    base_url: "https://meta.legacyfabric.net/v2",
    default_prefix: "legacyfabric",
};

/// This is the API for the LegacyFabric project which aims to backport the Fabric loader
/// to older versions, up to 1.14 snapshots.
pub static BABRIC_API: Api = Api {
    base_url: "https://meta.babric.glass-launcher.net/v2",
    default_prefix: "babric",
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
    game_version: GameVersion,
    loader_version: LoaderVersion,
}

impl Installer {

    /// Create a new installer with default configuration.
    pub fn new(main_dir: impl Into<PathBuf>, api: &'static Api) -> Self {
        Self {
            mojang: mojang::Installer::new(main_dir),
            inner: InstallerInner {
                api,
                game_version: GameVersion::Stable,
                loader_version: LoaderVersion::Stable,
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

    /// Get the underlying standard installer.
    #[inline]
    pub fn standard(&self) -> &standard::Installer {
        self.mojang.standard()
    }

    /// Get the underlying mojang installer.
    #[inline]
    pub fn mojang(&self) -> &mojang::Installer {
        &self.mojang
    }

    /// By default, this Fabric installer targets the latest stable version. To also
    /// change the fabric loader's version to use, see [`Self::loader`]. 
    /// 
    /// If this root version is an alias (`Release` the default, or `Snapshot`), it will 
    /// require the online version manifest, if the alias is not found in the manifest 
    /// (which is an issue on Mojang's side) then a 
    /// [`mojang::Error::RootAliasNotFound`] is returned.
    pub fn set_game_version(&mut self, version: impl Into<GameVersion>) {
        self.inner.game_version = version.into();
    }

    /// See [`Self::set_game_version`].   
    #[inline]
    pub fn game_version(&self) -> &GameVersion {
        &self.inner.game_version
    }

    /// By default, this Fabric installer targets the latest loader version compatible
    /// with the root version, use this function to override the loader version to use.
    pub fn set_loader_version(&mut self, version: impl Into<LoaderVersion>) {
        self.inner.loader_version = version.into();
    }

    /// See [`Self::set_loader_version`].   
    #[inline]
    pub fn loader_version(&self) -> &LoaderVersion {
        &self.inner.loader_version
    }

    /// Install the currently configured Fabric loader with the given handler.
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {

        let Self {
            ref mut mojang,
            ref inner,
        } = self;

        let game_stable = match self.inner.game_version {
            GameVersion::Stable => Some(true),
            GameVersion::Unstable => Some(false),
            _ => None,
        };

        let game_version = if let Some(game_stable) = game_stable {
            inner.api.request_game_versions()?
                .into_iter()
                .find(|game| game.stable || !game_stable)
                .map(|game| game.version)
                .ok_or_else(|| Error::AliasGameVersionNotFound { 
                    game_version: self.inner.game_version.clone()
                })?
        } else {
            match self.inner.game_version {
                GameVersion::Name(ref name) => name.clone(),
                _ => unreachable!()
            }
        };

        let loader_stable = match self.inner.loader_version {
            LoaderVersion::Stable => Some(true),
            LoaderVersion::Unstable => Some(false),
            _ => None,
        };

        let loader_version = if let Some(loader_stable) = loader_stable {
            inner.api.request_game_loader_versions(&game_version)?
                .into_iter()
                .find(|loader| {
                    // Some APIs don't provide the 'stable' on loader/intermediary
                    // versions, so we rely on the version containing a '-beta'.
                    let stable = loader.loader.stable.unwrap_or_else(|| {
                        !loader.loader.version.contains("-beta")
                    });
                    // Latest stable is always valid, or we want unstable.
                    stable || !loader_stable
                })
                .map(|loader| loader.loader.version)
                .ok_or_else(|| Error::AliasLoaderVersionNotFound {
                    game_version: game_version.clone(),
                    loader_version: self.inner.loader_version.clone(),
                })?
        } else {
            match self.inner.game_version {
                GameVersion::Name(ref name) => name.clone(),
                _ => unreachable!()
            }
        };

        // Set the root version for underlying Mojang installer, equal to the name that
        // we'll give to the version.
        let prefix = inner.api.default_prefix;
        let root_version = format!("{prefix}-{game_version}-{loader_version}");
        mojang.set_root_version(RootVersion::Name(root_version.clone()));

        // Scoping the temporary internal handler.
        let game = {

            let mut handler = InternalHandler {
                inner: &mut handler,
                installer: &inner,
                error: Ok(()),
                root_version: &root_version,
                game_version: &game_version,
                loader_version: &loader_version,
            };
    
            // Same as above, we are giving a &mut dyn ref to avoid huge monomorphization.
            let res = mojang.install(handler.as_mojang_dyn());
            handler.error?;
            res?

        };
        
        Ok(game)

    }

}

/// Handler for events happening when installing.
pub trait Handler: mojang::Handler {

    /// Handle an even from the fabric installer.
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
pub enum Event<'a> {
    VersionFetching {
        game_version: &'a str,
        loader_version: &'a str,
    },
    VersionFetched {
        game_version: &'a str,
        loader_version: &'a str,
    },
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Mojang(#[source] mojang::Error),
    /// An alias game version, `Stable` or `Unstable` has not been found because the 
    /// no version is matching this criteria.
    #[error("alias game version not found: {game_version:?}")]
    AliasGameVersionNotFound {
        game_version: GameVersion,
    },
    /// An alias loader version, `Stable` has not been found because the alias is missing
    /// from the fabric API's versions.
    #[error("alias loader version not found: {game_version}/{loader_version:?}")]
    AliasLoaderVersionNotFound {
        game_version: String,
        loader_version: LoaderVersion,
    },
    /// The given game version as requested to launch Fabric with is not supported by the
    /// selected API.
    #[error("game version not found: {game_version}")]
    GameVersionNotFound {
        game_version: String,
    },
    /// The given loader version as requested to launch Fabric with is not supported by 
    /// the selected API for the requested game version (which is supported).
    #[error("loader version not found: {game_version}/{loader_version}")]
    LoaderVersionNotFound {
        game_version: String,
        loader_version: String,
    },
}

impl<T: Into<mojang::Error>> From<T> for Error {
    fn from(value: T) -> Self {
        Self::Mojang(value.into())
    }
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

/// A fabric-compatible API.
pub struct Api {
    /// Base URL for that API, not ending with a '/'. This API must support the following
    /// endpoints supporting the same API as official Fabric API: 
    /// - `/versions/game`
    /// - `/versions/loader`
    /// - `/versions/loader/<game_version>`
    /// - `/versions/loader/<game_version>/<loader_loader>` (returning status 400)
    /// - `/versions/loader/<game_version>/<loader_loader>/profile/json`
    base_url: &'static str,
    /// Default prefix for the full root version id of the format 
    /// '<default prefix>-<game version>-<loader version>.
    default_prefix: &'static str,
}

impl Api {

    /// Request supported game versions.
    pub fn request_game_versions(&self) -> reqwest::Result<Vec<serde::Game>> {
        crate::tokio::sync(async move {
            crate::http::client()?
                .get(format!("{}/versions/game", self.base_url))
                .header(reqwest::header::ACCEPT, "application/json")
                .send().await?
                .error_for_status()?
                .json().await
        })
    }

    /// Request supported loader versions.
    pub fn request_loader_versions(&self) -> reqwest::Result<Vec<serde::Loader>> {
        crate::tokio::sync(async move {
            crate::http::client()?
                .get(format!("{}/versions/loader", self.base_url))
                .header(reqwest::header::ACCEPT, "application/json")
                .send().await?
                .error_for_status()?
                .json().await
        })
    }

    /// Request supported loader versions for the given game version.
    pub fn request_game_loader_versions(&self, game_version: &str) -> reqwest::Result<Vec<serde::GameLoader>> {
        crate::tokio::sync(async move {
            crate::http::client()?
                .get(format!("{}/versions/loader/{game_version}", self.base_url))
                .header(reqwest::header::ACCEPT, "application/json")
                .send().await?
                .error_for_status()?
                .json().await
        })
    }

    /// Return true if the given game version has any loader versions supported.
    pub fn request_has_game_loader_versions(&self, game_version: &str) -> reqwest::Result<bool> {
        crate::tokio::sync(async move {
            crate::http::client()?
                .get(format!("{}/versions/loader/{game_version}", self.base_url))
                .header(reqwest::header::ACCEPT, "application/json")
                .send().await?
                .error_for_status()?
                .bytes().await
                .map(|bytes| &*bytes == b"[]") // This avoids parsing JSON
        })
    }

    /// Request the prebuilt version metadata for the given game and loader versions.
    pub fn request_game_loader_version_metadata(&self, game_version: &str, loader_version: &str) -> reqwest::Result<standard::serde::VersionMetadata> {
        crate::tokio::sync(async move {
            crate::http::client()?
                .get(format!("{}/versions/loader/{game_version}/{loader_version}/profile/json", self.base_url))
                .header(reqwest::header::ACCEPT, "application/json")
                .send().await?
                .error_for_status()?
                .json().await
        })
    }

}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Api").finish()
    }
}

/// Specify the fabric game version to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameVersion {
    /// Use the latest stable game version, this is usually equivalent to the 'Release'
    /// version with Mojang, but is up to each fabric-like API to decide.
    Stable,
    /// Use the latest unstable game version, this is usually equivalent to the 'Snapshot'
    /// version with Mojang, but is up to each fabric-like API to decide.
    /// 
    /// Note that if the most recent version is stable, it will also be selected as the
    /// most recent unstable one, much like Mojang, when a stable release is just
    /// published, it is also the latest snapshot (usually not for a long time).
    Unstable,
    /// Use the specific version.
    Name(String),
}

impl<T: Into<String>> From<T> for GameVersion {
    fn from(value: T) -> Self {
        Self::Name(value.into())
    }
}

/// Specify the fabric loader version to start, see [`GameVersion`] for more explanation,
/// both are almost the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderVersion {
    /// Use the latest stable loader version for the root version.
    Stable,
    /// Use the latest unstable loader version for the root version, see 
    /// [`GameVersion::Unstable`] for more explanation, the two are the same.
    Unstable,
    /// Use the specific version.
    Name(String),
}

/// An impl so that we can give string-like objects to the builder.
impl<T: Into<String>> From<T> for LoaderVersion {
    fn from(value: T) -> Self {
        Self::Name(value.into())
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
    /// The real version is, as defined 
    root_version: &'a str,
    game_version: &'a str,
    loader_version: &'a str,
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

impl<H: Handler> mojang::Handler for InternalHandler<'_, H> {
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        self.inner.handle_mojang_event(event)
    }
}

impl<H: Handler> InternalHandler<'_, H> {

    fn handle_standard_event_inner(&mut self, event: standard::Event) -> Result<()> { 
        
        match event {
            standard::Event::VersionNotFound { 
                version: id, 
                file, 
                retry,
            } if id == self.root_version => {

                self.inner.handle_fabric_event(Event::VersionFetching {
                    game_version: self.game_version,
                    loader_version: self.loader_version,
                });

                // At this point we've not yet checked if either game or loader versions
                // are known by the API, we just wanted to allow the user to input any
                // version if he will. But now that we need to request the prebuilt
                // version metadata, in case of error we'll try to understand what's the
                // issue: unknown game version or unknown loader version?
                let mut metadata = match self.installer.api.request_game_loader_version_metadata(self.game_version, self.loader_version) {
                    Ok(metadata) => metadata,
                    Err(e) if e.status() == Some(StatusCode::NOT_FOUND) => {
                        if self.installer.api.request_has_game_loader_versions(self.game_version)? {
                            return Err(Error::LoaderVersionNotFound { 
                                game_version: self.game_version.to_string(),
                                loader_version: self.loader_version.to_string(),
                            });
                        } else {
                            return Err(Error::GameVersionNotFound { 
                                game_version: self.game_version.to_string(),
                            });
                        }
                    }
                    Err(e) => return Err(e.into()),
                };

                // Force the version id, the prebuilt one might not be exact.
                metadata.id = id.to_string();
                standard::write_version_metadata(file, &metadata)?;

                *retry = true;

                self.inner.handle_fabric_event(Event::VersionFetched {
                    game_version: self.game_version,
                    loader_version: self.loader_version,
                });

                // Note that we never forward the event in any case...

            }
            _ => self.inner.handle_standard_event(event),
        }

        Ok(())

    }
    
}
