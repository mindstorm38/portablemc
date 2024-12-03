//! Extension to the Mojang installer to support fetching and installation of 
//! Forge and NeoForge mod loader versions.

use crate::mojang;


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
    game_version: GameVersion,
    loader_version: LoaderVersion,
}

/// Specify the forge game version to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameVersion {
    /// Use the latest Mojang's release to start the game.
    Release,
    /// Use the specific version.
    Id(String),
}

/// Specify the forge loader version to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderVersion {
    /// Use the latest stable loader version for the game version.
    Stable,
    /// Use the latest unstable loader version for the game version. Falling back to
    /// [`Self::Stable`] if no unstable is version is available before the first stable.
    Unstable,
    /// Use the specific version.
    Id(String),
}
