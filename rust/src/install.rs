//! Standard installer actions.

use std::default;

use serde_json::Value;

use crate::context::Context;


/// This is the standard version installer that provides minimal and common installation
/// of Minecraft versions. The install procedure given by this installer is idempotent,
/// which mean that if the installer's configuration has not been modified, running it a
/// second time won't do any modification.
pub struct Installer {
    /// The directory context to install the game.
    context: Context,
    /// The version identifier to ultimately install.
    version: Version,
}

impl Installer {

    /// Create a new installer for the latest release and a default context.
    pub fn new() -> Self {
        Self::with_version(MojangVersion::Release.into())
    }

    /// Create a new installer for the given version and a default context.
    #[inline]
    pub fn with_version(version: Version) -> Self {
        Self::with_context(Context::default(), version)
    }

    /// Create a new installer with the given version and context.
    pub fn with_context(context: Context, version: Version) -> Self {
        Self {
            context,
            version,
        }
    }

    /// Get a reference to the context used by this installer.
    #[inline]
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Get a reference to the version to install.
    #[inline]
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn install(&mut self) -> () {

        let main_id = match &self.version {
            Version::Local(id) => &id[..],
            Version::Mojang(_) => todo!(),
            Version::Fabric(_) => todo!(),
            Version::Forge(_) => todo!(),
        };

        let mut resolving_id = main_id;

        loop {

            let dir = self.context.get_version_dir(main_id);
            

        }

    }

    fn resolve_hierarchy(&mut self) {

    }

}

/// An event handler for installation process.
pub trait Handler {

}

/// Describe a version to install.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Version {
    /// Install a version from its local metadata only.
    Local(String),
    /// Install a Mojang's official version from manifest.
    Mojang(MojangVersion),
    /// Install a Fabric mod loader version.
    Fabric(FabricVersion),
    /// Install a Forge mod loader version.
    Forge(ForgeVersion),
}

/// Describe a Mojang version from manifest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MojangVersion {
    /// Target the latest version, this will be resolved against the manifest.
    Release,
    /// Target the latest snapshot, this will be resolved against the manifest.
    Snapshot,
    /// Target a version from its identifier.
    Specific(String),
}

/// Describe a fabric version to install.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FabricVersion {
    pub api: (),
    pub mojang_version: MojangVersion,
    pub loader_version: Option<String>,
}

/// Describe a forge version to install.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForgeVersion {
    pub mojang_version: MojangVersion,
}

impl From<MojangVersion> for Version {
    #[inline]
    fn from(value: MojangVersion) -> Self {
        Self::Mojang(value)
    }
}

impl From<FabricVersion> for Version {
    #[inline]
    fn from(value: FabricVersion) -> Self {
        Self::Fabric(value)
    }
}

impl From<ForgeVersion> for Version {
    #[inline]
    fn from(value: ForgeVersion) -> Self {
        Self::Forge(value)
    }
}
