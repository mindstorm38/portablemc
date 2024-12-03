//! Extension to the Mojang installer to support fetching and installation of 
//! Forge and NeoForge mod loader versions.

use std::path::PathBuf;

use crate::{standard, mojang};
use crate::gav::Gav;

pub use mojang::Game;


/// An installer that supports Forge and NeoForge
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
    api: Api,
    game_version: GameVersion,
    loader_version: LoaderVersion,
}

impl Installer {

    /// Create a new installer with default configuration.
    pub fn new(main_dir: impl Into<PathBuf>, api: Api) -> Self {
        Self {
            mojang: mojang::Installer::new(main_dir),
            inner: InstallerInner {
                api,
                game_version: GameVersion::Release,
                loader_version: LoaderVersion::Stable,
            }
        }
    }
    
    /// Same as [`Self::new`] but using the default main directory in your system,
    /// returning none if there is no default main directory on your system.
    pub fn new_with_default(api: Api) -> Option<Self> {
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

    /// By default, this Forge and NeoForge installer targets the latest stable version. To also
    /// change the fabric loader's version to use, see [`Self::loader`]. 
    /// 
    /// If this root version is an alias (`Release` the default, or `Snapshot`), it will 
    /// require the online version manifest, if the alias is not found in the manifest 
    /// (which is an issue on Mojang's side) then a 
    /// [`mojang::Error::RootAliasNotFound`] is returned.
    pub fn game_version(&mut self, version: impl Into<GameVersion>) {
        self.inner.game_version = version.into();
    }

    /// Install the currently configured Forge/NeoForge loader with the given handler.
    pub fn install(&mut self, mut handler: ()) -> Result<Game, ()> {

        let Self {
            ref mut mojang,
            ref inner,
        } = self;

        todo!()

    }

}

/// The underlying Forge/NeoForge API.
#[derive(Debug, Clone)]
pub enum Api {
    Forge,
    NeoForge,
}

/// Specify the forge game version to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameVersion {
    /// Use the latest Mojang's release to start the game.
    Release,
    /// Use the specific version.
    Id(String),
}

/// Specify the forge loader version to start. Note that, unlike fabric-like loaders,
/// forge don't have loader versions that are supported by many (or all) game versions.
/// Instead, each forge loader version is tied to a game version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderVersion {
    /// Use the latest stable loader version for the game version.
    Stable,
    /// Use the latest unstable loader version for the game version, unstable version
    /// don't exists
    Unstable,
    /// Use the specific version.
    Id(String),
}

// ========================== //
// Following code is internal //
// ========================== //

