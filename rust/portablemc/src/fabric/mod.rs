//! Extension to the Mojang installer to support fetching and installation of 
//! Fabric-related mod loader versions.

mod serde;

use std::path::Path;

use reqwest::StatusCode;

use crate::download;
use crate::standard;
use crate::mojang;

pub use mojang::Game;


/// An installer for supporting mod loaders that are Fabric or like it (Quilt, 
/// LegacyFabric, Babric). The generic parameter is used to specify the API to use.
#[derive(Debug, Clone)]
pub struct Installer {
    /// The underlying Mojang installer logic.
    mojang: mojang::Installer,
    loader: Loader,
    game_version: GameVersion,
    loader_version: LoaderVersion,
}

impl Installer {

    /// Create a new installer with default configuration.
    pub fn new(loader: Loader, game_version: impl Into<GameVersion>, loader_version: impl Into<LoaderVersion>) -> Self {
        Self {
            mojang: mojang::Installer::new(String::new()),
            loader,
            game_version: game_version.into(),
            loader_version: loader_version.into(),
        }
    }

    /// Same as [`Self::new`] but use the latest stable game and loader versions.
    pub fn new_with_stable(loader: Loader) -> Self {
        Self::new(loader, GameVersion::Stable, LoaderVersion::Stable)
    }

    /// Get the underlying mojang installer.
    #[inline]
    pub fn mojang(&self) -> &mojang::Installer {
        &self.mojang
    }

    /// Get the underlying mojang installer through mutable reference.
    /// 
    /// *Note that the `version` and `fetch` properties will be overwritten when 
    /// installing.*
    #[inline]
    pub fn mojang_mut(&mut self) -> &mut mojang::Installer {
        &mut self.mojang
    }

    /// Get the kind of loader that will be installed.
    #[inline]
    pub fn loader(&self) -> Loader {
        self.loader
    }

    /// Set the kind of loader that will be installed.
    #[inline]
    pub fn set_loader(&mut self, loader: Loader) -> &mut Self {
        self.loader = loader;
        self
    }

    /// Get the game version the loader will be installed for.
    #[inline]
    pub fn game_version(&self) -> &GameVersion {
        &self.game_version
    }

    /// Set the game version the loader will be installed for.
    #[inline]
    pub fn set_game_version(&mut self, version: impl Into<GameVersion>) {
        self.game_version = version.into();
    }

    /// Get the loader version to install.
    #[inline]
    pub fn loader_version(&self) -> &LoaderVersion {
        &self.loader_version
    }

    /// Set the loader version to install.
    #[inline]
    pub fn set_loader_version(&mut self, version: impl Into<LoaderVersion>) {
        self.loader_version = version.into();
    }

    /// Install the currently configured Fabric loader with the given handler.
    #[inline]
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {
        self.install_dyn(&mut handler)
    }

    #[inline(never)]
    pub fn install_dyn(&mut self, handler: &mut dyn Handler) -> Result<Game> {

        let Self {
            ref mut mojang,
            loader,
            ref game_version,
            ref loader_version,
        } = *self;

        let api = Api::new(loader);

        let game_version = match game_version {
            GameVersion::Stable |
            GameVersion::Unstable => {

                let stable = matches!(game_version, GameVersion::Stable);
                let versions = api.request_game_versions()
                    .map_err(|e| standard::Error::new_reqwest(e, "request fabric game versions"))?;

                match versions.find_latest(stable) {
                    Some(v) => v.name().to_string(),
                    None => return Err(Error::LatestVersionNotFound { 
                        game_version: None, 
                        stable,
                    }),
                }

            }
            GameVersion::Name(name) => name.clone(),
        };

        let loader_version = match loader_version {
            LoaderVersion::Stable |
            LoaderVersion::Unstable => {
                
                let stable = matches!(loader_version, LoaderVersion::Stable);
                let versions = api.request_loader_versions(Some(&game_version))
                    .map_err(|e| standard::Error::new_reqwest(e, "request fabric loader versions"))?;
                
                match versions.find_latest(stable) {
                    Some(v) => v.name().to_string(),
                    None => return Err(Error::LatestVersionNotFound { 
                        game_version: Some(game_version), 
                        stable,
                    }),
                }

            }
            LoaderVersion::Name(name) => name.clone(),
        };

        // Set the root version for underlying Mojang installer, equal to the name that
        // we'll give to the version.
        let prefix = loader.default_prefix();
        let root_version = format!("{prefix}-{game_version}-{loader_version}");
        mojang.set_version(root_version.clone());

        // Scoping the temporary internal handler.
        let game = {

            let mut handler = InternalHandler {
                inner: &mut *handler,
                error: Ok(()),
                api,
                root_version: &root_version,
                game_version: &game_version,
                loader_version: &loader_version,
            };
    
            // Same as above, we are giving a &mut dyn ref to avoid huge monomorphization.
            let res = mojang.install(&mut handler);
            handler.error?;
            res?

        };
        
        Ok(game)

    }

}

crate::trait_event_handler! {
    pub trait Handler: mojang::Handler {
        fn fetch_loader_version(game_version: &str, loader_version: &str);
        fn fetched_loader_version(game_version: &str, loader_version: &str);
    }
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Mojang(#[source] mojang::Error),
    /// An alias version, `Stable` or `Unstable` has not been found because the no version
    /// is matching this criteria. This is used for both game version and loader version,
    /// when game version is specified it means that the given .
    #[error("latest version not found (stable: {stable})")]
    LatestVersionNotFound {
        game_version: Option<String>,
        stable: bool,
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

/// Represent the different kind of loaders to install or fetch for versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    /// This is the original and official Fabric API.
    Fabric,
    /// This is the API for the Quilt mod loader, which is a fork of Fabric.
    Quilt,
    /// This is the API for the LegacyFabric project which aims to backport the Fabric loader
    /// to older versions, up to 1.14 snapshots.
    LegacyFabric,
    /// This is the API for the Babric project, which aims to support the Fabric loader 
    /// for Minecraft beta 1.7.3 in particular.
    Babric,
}

impl Loader {

    fn default_prefix(self) -> &'static str {
        match self {
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
            Loader::LegacyFabric => "legacyfabric",
            Loader::Babric => "babric",
        }
    }

}

/// Specify the fabric game version to start the loader version.
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

/// A fabric-compatible API, this can be used to list and retrieve loader versions that
/// can be given to the installer for installation.
#[derive(Debug)]
pub struct Api {
    /// Base URL for that API, not ending with a '/'. This API must support the following
    /// endpoints supporting the same API as official Fabric API: 
    /// - `/versions/game`
    /// - `/versions/loader`
    /// - `/versions/loader/<game_version>`
    /// - `/versions/loader/<game_version>/<loader_loader>` (returning status 400)
    /// - `/versions/loader/<game_version>/<loader_loader>/profile/json`
    base_url: &'static str,
}

impl Api {

    /// Initialize the handle to 
    pub fn new(loader: Loader) -> Self {
        Self {
            base_url: match loader {
                Loader::Fabric => "https://meta.fabricmc.net/v2",
                Loader::Quilt => "https://meta.quiltmc.org/v3",
                Loader::LegacyFabric => "https://meta.legacyfabric.net/v2",
                Loader::Babric => "https://meta.babric.glass-launcher.net/v2",
            }
        }
    }

    /// Request supported game versions.
    pub fn request_game_versions(&self) -> reqwest::Result<ApiGameVersions<'_>> {
        self.raw_request_game_versions().map(|versions| ApiGameVersions {
            _api: self,
            versions,
        })
    }

    fn raw_request_game_versions(&self) -> reqwest::Result<Vec<serde::Game>> {
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
    pub fn request_loader_versions(&self, game_version: Option<&str>) -> reqwest::Result<ApiLoaderVersions<'_>> {
        if let Some(game_version) = game_version {
            self.raw_request_game_loader_versions(game_version).map(|versions| ApiLoaderVersions {
                _api: self,
                versions: versions.into_iter().map(|v| v.loader).collect(),
            })
        } else {
            self.raw_request_loader_versions().map(|versions| ApiLoaderVersions {
                _api: self,
                versions,
            })
        }
    }

    fn raw_request_loader_versions(&self) -> reqwest::Result<Vec<serde::Loader>> {
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
    fn raw_request_game_loader_versions(&self, game_version: &str) -> reqwest::Result<Vec<serde::GameLoader>> {
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
    fn raw_request_has_game_loader_versions(&self, game_version: &str) -> reqwest::Result<bool> {
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
    fn raw_request_game_loader_version_metadata(&self, game_version: &str, loader_version: &str) -> reqwest::Result<standard::serde::VersionMetadata> {
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

#[derive(Debug)]
pub struct ApiGameVersions<'a> {
    _api: &'a Api,
    versions: Vec<serde::Game>,
}

impl ApiGameVersions<'_> {

    /// Create an iterator over all game versions.
    pub fn iter(&self) -> impl Iterator<Item = ApiGameVersion<'_>> + use<'_> {
        self.versions.iter().map(|inner| ApiGameVersion { inner })
    }

    /// Get the latest supported version, stable or unstable.
    pub fn find_latest(&self, stable: bool) -> Option<ApiGameVersion<'_>> {
        self.iter().find(|v| !stable || v.is_stable())
    }

}

#[derive(Debug)]
pub struct ApiGameVersion<'d> {
    inner: &'d serde::Game,
}

impl<'d> ApiGameVersion<'d> {

    #[inline]
    pub fn name(&self) -> &'d str {
        &self.inner.version
    }

    #[inline]
    pub fn is_stable(&self) -> bool {
        self.inner.stable
    }

}

#[derive(Debug)]
pub struct ApiLoaderVersions<'a> {
    _api: &'a Api,
    versions: Vec<serde::Loader>,
}

impl ApiLoaderVersions<'_> {

    /// Create an iterator over all loader versions.
    pub fn iter(&self) -> impl Iterator<Item = ApiLoaderVersion<'_>> + use<'_> {
        self.versions.iter().map(|inner| ApiLoaderVersion { inner })
    }

    /// Get the latest supported version, stable or unstable.
    pub fn find_latest(&self, stable: bool) -> Option<ApiLoaderVersion<'_>> {
        self.iter().find(|v| !stable || v.is_stable())
    }

}

#[derive(Debug)]
pub struct ApiLoaderVersion<'d> {
    inner: &'d serde::Loader,
}

impl<'d> ApiLoaderVersion<'d> {

    #[inline]
    pub fn name(&self) -> &'d str {
        &self.inner.version
    }

    #[inline]
    pub fn is_stable(&self) -> bool {
        self.inner.stable.unwrap_or_else(|| {
            !self.inner.version.contains("-beta") && !self.inner.version.contains("-pre")
        })
    }

}

// ========================== //
// Following code is internal //
// ========================== //

/// Internal handler given to the standard installer.
struct InternalHandler<'a> {
    /// Inner handler.
    inner: &'a mut dyn Handler,
    /// If there is an error in the handler.
    error: Result<()>,
    /// The real version is, as defined 
    api: Api,
    root_version: &'a str,
    game_version: &'a str,
    loader_version: &'a str,
}

impl download::Handler for InternalHandler<'_> {

    fn __internal_fallback(&mut self, _token: crate::sealed::Token) -> Option<&mut dyn download::Handler> {
        Some(&mut self.inner)
    }

}

impl standard::Handler for InternalHandler<'_> {
    
    fn __internal_fallback(&mut self, _token: crate::sealed::Token) -> Option<&mut dyn standard::Handler> {
        Some(&mut self.inner)
    }

    fn need_version(&mut self, version: &str, file: &Path) -> bool {
        match self.inner_need_version(version, file) {
            Ok(true) => return true,
            Ok(false) => (),
            Err(e) => self.error = Err(e),
        }
        self.inner.need_version(version, file)
    }

}

impl mojang::Handler for InternalHandler<'_> {

    fn __internal_fallback(&mut self, _token: crate::sealed::Token) -> Option<&mut dyn mojang::Handler> {
        Some(&mut self.inner)
    }

}

impl InternalHandler<'_> {

    fn inner_need_version(&mut self, version: &str, file: &Path) -> Result<bool> {

        if version != self.root_version {
            return Ok(false);
        }

        // self.inner.fetch_version(self.root_version);
        self.inner.fetch_loader_version(self.game_version, self.loader_version);

        // At this point we've not yet checked if either game or loader versions
        // are known by the API, we just wanted to allow the user to input any
        // version if he will. But now that we need to request the prebuilt
        // version metadata, in case of error we'll try to understand what's the
        // issue: unknown game version or unknown loader version?
        let mut metadata = match self.api.raw_request_game_loader_version_metadata(self.game_version, self.loader_version) {
            Ok(metadata) => metadata,
            Err(e) if e.status() == Some(StatusCode::NOT_FOUND) => {
                
                let has_versions = self.api.raw_request_has_game_loader_versions(self.game_version)
                    .map_err(|e| standard::Error::new_reqwest(e, "request fabric has game loader versions"))?;

                if has_versions {
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
            Err(e) => return Err(standard::Error::new_reqwest(e, "request fabric game loader version metadata").into()),
        };

        // Force the version id, the prebuilt one might not be exact.
        metadata.id = version.to_string();
        standard::write_version_metadata(file, &metadata)?;

        // self.inner.fetched_version(self.root_version);
        self.inner.fetched_loader_version(self.game_version, self.loader_version);

        Ok(true)

    }
    
}
