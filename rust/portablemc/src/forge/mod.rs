//! Extension to the Mojang installer to support fetching and installation of 
//! Forge and NeoForge mod loader versions.

pub mod serde;

use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek};
use std::process::{Command, Output};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::{env, fmt, fs};
use std::fmt::Write;
use std::fs::File;

use crate::path::{const_path, PathBufExt, PathExt};
use crate::download::{self, Batch, EntryErrorKind};
use crate::standard::{self, LIBRARIES_URL};
use crate::mojang::{self, RootVersion};
use crate::maven::{Gav, MavenMetadata};

use reqwest::StatusCode;
use zip::ZipArchive;

pub use mojang::Game;


/// An installer that supports Forge and NeoForge mod loaders.
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
                game_version: GameVersion::Release,
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

    /// By default, this Forge and NeoForge installer targets the latest stable version.
    pub fn set_game_version(&mut self, version: impl Into<GameVersion>) {
        self.inner.game_version = version.into();
    }

    /// See [`Self::set_game_version`].   
    #[inline]
    pub fn game_version(&self) -> &GameVersion {
        &self.inner.game_version
    }

    /// By default, this Forge installer targets the latest stable loader version for the
    /// currently configured game version, use this function to override the loader 
    /// version to use.
    pub fn set_loader_version(&mut self, version: impl Into<LoaderVersion>) {
        self.inner.loader_version = version.into();
    }

    /// See [`Self::set_loader_version`].   
    #[inline]
    pub fn loader_version(&self) -> &LoaderVersion {
        &self.inner.loader_version
    }

    /// Install the currently configured Forge/NeoForge loader with the given handler.
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {

        let Self {
            ref mut mojang,
            ref inner,
        } = self;

        // Start by getting the game version.
        let game_version = match inner.game_version {
            GameVersion::Name(ref name) => name.clone(),
            GameVersion::Release => {
                mojang::request_manifest(handler.as_download_dyn())?
                    .latest.get(&standard::serde::VersionType::Release)
                    .cloned()
                    .ok_or_else(|| Error::AliasGameVersionNotFound { 
                        game_version: inner.game_version.clone(),
                    })?
            }
        };
                        
        let Some([game_major, game_minor]) = parse_game_version(&game_version) else {
            return Err(Error::GameVersionNotFound { 
                game_version,
            });
        };

        let artifact = (inner.api.maven_artifact)(&game_version, game_major, game_minor);

        // Full loader version id, containing the game version.
        let loader_version = match inner.loader_version {
            LoaderVersion::Name(ref name) => {
                (inner.api.build_loader_version)(&game_version, game_major, game_minor, &name)
            }
            LoaderVersion::Stable |
            LoaderVersion::Unstable => {

                let metadata_url = format!("{}/{artifact}/maven-metadata.xml", inner.api.maven_group_base_url);
                let metadata = request_maven_metadata(&metadata_url)?;

                let stable = matches!(inner.loader_version, LoaderVersion::Stable);
                let prefix = (inner.api.build_loader_version_prefix)(&game_version, game_major, game_minor);
                
                // This common closure will set the 'found_version' external variable to 
                // true if some loader version contains the game version prefix, so at
                // least on loader is supported for the game version, this is used to 
                // have a more precise error returned.
                let mut found_game_version = false;
                let find_version = |version: &&str| {
                    if !version.starts_with(&prefix) {
                        return false;
                    }
                    found_game_version = true;
                    // Either stable is not required, to we return any version, or the 
                    // version must be stable.
                    !stable || (inner.api.is_loader_version_stable)(&game_version, game_major, game_minor, version)
                };

                let version;
                if inner.api.maven_manifest_reverse_order {
                    version = metadata.versions().rev().find(find_version).map(str::to_string);
                } else {
                    version = metadata.versions().find(find_version).map(str::to_string);
                }
                
                let Some(version) = version else {
                    // Check if at least one loader with the game version has been found,
                    // if not the case we can return that the game version is not yet
                    // supported by the loader.
                    if found_game_version {
                        return Err(Error::AliasLoaderVersionNotFound { 
                            game_version, 
                            loader_version: inner.loader_version.clone(),
                        });
                    } else {
                        return Err(Error::GameVersionNotFound { 
                            game_version,
                        });
                    }
                };

                version

            }
        };

        // Construct the root version id, and adding it to fetch exclude, we don't want
        // to try to fetch it from Mojang's manifest: it's pointless.
        let prefix = inner.api.default_prefix;
        let root_version = format!("{prefix}-{loader_version}");
        mojang.add_fetch_exclude(root_version.clone());

        // Get the check configuration for this forge version.
        let install_config = (inner.api.install_config)(&game_version, game_major, game_minor, &loader_version);

        // The goal is to run the installer a first time, check potential errors to 
        // know if the error is related to the loader, or not.
        mojang.set_root_version(RootVersion::Name(root_version.clone()));
        let reason = match mojang.install(handler.as_mojang_dyn()) {
            Ok(game) => {

                if !install_config.check_libraries {
                    return Ok(game);
                }

                // Using this outer loop to break when some reason to install is met.
                loop {

                    fn check_exists(base: &PathBuf, suffix: &str) -> bool {
                        let file = base.clone().appended(suffix);
                        fs::exists(file).unwrap_or_default()
                    }
                    
                    // Start by checking patched client and universal client
                    let loader_artifact = mojang.standard().libraries_dir()
                        .join(inner.api.maven_group_base_dir)
                        .joined(artifact)
                        .joined(&loader_version)
                        .joined(artifact)
                            .appended("-")
                            .appended(&loader_version)
                            .appended("-");
                    
                    if !check_exists(&loader_artifact, "client.jar") {
                        break InstallReason::MissingPatchedClient;
                    }
    
                    if !check_exists(&loader_artifact, "universal.jar") {
                        break InstallReason::MissingUniversalClient;
                    }

                    // We analyze game argument to try find which libraries are absolutely
                    // required for the game to run, there has been so many way of launching
                    // the game in the Forge/NeoForge history that it's complicated to ensure
                    // that we can accurately determine if the mod loader is properly 
                    // installed.
                    let mut mcp_version = None;
                    let mut args_iter = game.game_args.iter();
                    while let Some(arg) = args_iter.next() {
                        match arg.as_str() {
                            "--fml.neoFormVersion" |
                            "--fml.mcpVersion" => {
                                let Some(version) = args_iter.next() else { continue };
                                mcp_version = Some(version.as_str());
                            }
                            _ => {}
                        }
                    }

                    // If there is a MCP version to check, we go check if client extra, slim
                    // and srg files are present, or not, they are loaded dynamically by the 
                    // mod loader.
                    if let Some(mcp_version) = mcp_version {

                        let mcp_artifact = mojang.standard().libraries_dir()
                            .join("net")
                            .joined("minecraft")
                            .joined("client")
                            .joined(&game_version)
                                .appended("-")
                                .appended(mcp_version)
                            .joined("client")
                                .appended("-")
                                .appended(&game_version)
                                .appended("-")
                                .appended(mcp_version)
                                .appended("-");

                        if !check_exists(&mcp_artifact, "srg.jar") {
                            break InstallReason::MissingClientSrg;
                        }

                        if install_config.extra_in_mcp {
                            if !check_exists(&mcp_artifact, "extra.jar") {
                                break InstallReason::MissingClientExtra;
                            }
                        } else {

                            let mc_artifact = mojang.standard().libraries_dir()
                                .join("net")
                                .joined("minecraft")
                                .joined("client")
                                .joined(&game_version)
                                .joined("client")
                                .appended("-")
                                .appended(&game_version)
                                .appended("-");

                            if !check_exists(&mc_artifact, "extra.jar")
                            && !check_exists(&mc_artifact, "extra-stable.jar") {
                                break InstallReason::MissingClientExtra;
                            }

                        }

                    }

                    // No reason to reinstall, we return the game as-is.
                    return Ok(game);

                }

            }
            Err(mojang::Error::Standard(standard::Error::VersionNotFound { version })) 
            if version == root_version => {
                InstallReason::MissingVersionMetadata
            }
            Err(mojang::Error::Standard(standard::Error::LibraryNotFound { gav })) 
            if gav.group() == "net.minecraftforge" && gav.artifact() == "forge" => {
                InstallReason::MissingCoreLibrary
            }
            Err(e) => return Err(Error::Mojang(e))
        };

        try_install(&mut handler, 
            &mut *mojang, 
            inner.api, 
            artifact, 
            &root_version, 
            &game_version, 
            &loader_version,
            &install_config,
            serde::InstallSide::Client,
            reason)?;

        // Retrying launch!
        mojang.set_root_version(RootVersion::Name(root_version.clone()));
        let game = mojang.install(handler.as_mojang_dyn())?;
        Ok(game)

    }

}

/// Handler for events happening when installing.
pub trait Handler: mojang::Handler {

    /// Handle an even from the mojang installer.
    fn handle_forge_event(&mut self, event: Event) {
        let _ = event;
    }

    fn as_forge_dyn(&mut self) -> &mut dyn Handler 
    where Self: Sized {
        self
    }

}

/// Blanket implementation that does nothing.
impl Handler for () { }

impl<H: Handler + ?Sized> Handler for  &'_ mut H {
    fn handle_forge_event(&mut self, event: Event) {
        (*self).handle_forge_event(event)
    }
}

/// An event produced by the installer that can be handled by the install handler.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// The loader version failed to start, so this installer will (re)try to install
    /// the mod loader.
    Installing {
        tmp_dir: &'a Path,
        reason: InstallReason,
    },
    /// The loader installer will be fetched.
    InstallerFetching {
        game_version: &'a str,
        loader_version: &'a str,
    },
    /// The loader installer has been successfully fetched.
    InstallerFetched {
        game_version: &'a str,
        loader_version: &'a str,
    },
    /// Notify that the game will be installed manually before running the installer,
    /// because the installer needs it.
    GameInstalling {  },
    /// The loader installer libraries will be fetched, either from being download, or 
    /// being extracted from the installer archive.
    InstallerLibrariesFetching { },
    /// The loader installer libraries has been successfully fetched or extracted.
    InstallerLibrariesFetched { },
    /// An installer processor will be run.
    InstallerProcessor {
        name: &'a Gav,
        task: Option<&'a str>,
    },
    /// The mod loader has been apparently successfully installed, it will be run a 
    /// second time to try...
    Installed {  },
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Mojang(#[source] mojang::Error),
    /// An alias game version, `Release` has not been found because the no version is 
    /// matching this criteria.
    #[error("alias game version not found: {game_version:?}")]
    AliasGameVersionNotFound {
        game_version: GameVersion,
    },
    /// An alias loader version, `Stable` or `Unstable` has not been found because the 
    /// alias is missing the for API's versions.
    #[error("alias loader version not found: {game_version}/{loader_version:?}")]
    AliasLoaderVersionNotFound {
        game_version: String,
        loader_version: LoaderVersion,
    },
    /// The given game version as requested to launch Forge with is not supported by the
    /// selected API.
    #[error("game version not found: {game_version}")]
    GameVersionNotFound {
        game_version: String,
    },
    /// The given loader version as requested to launch Forge with has not supported 
    /// installer.
    #[error("loader version not found: {game_version}")]
    LoaderVersionNotFound {
        game_version: String,
        loader_version: String,
    },
    /// The 'maven-metadata.xml' file requested only is 
    #[error("maven metadata is malformed")]
    MavenMetadataMalformed {  },
    /// The 'install_profile.json' installer file was not found.
    #[error("installer profile not found")]
    InstallerProfileNotFound {  },
    /// The 'install_profile.json' installer file is present but its versions are 
    /// incoherent with the expected loader and game versions that should've been 
    /// downloaded.
    #[error("installer profile incoherent")]
    InstallerProfileIncoherent {  },
    /// The 'version.json' installer file was not found, it contains the version metadata
    /// to be installed.
    #[error("installer version metadata not found")]
    InstallerVersionMetadataNotFound {  },
    /// A file needed to be extracted from the installer but was not found.
    #[error("installer file to extract not found")]
    InstallerFileNotFound {
        entry: String,
    },
    /// Failed to execute so process.
    #[error("installer invalid processor")]
    InstallerInvalidProcessor {
        name: Gav,
    },
    /// A processor has failed while running, the process output is linked.
    #[error("installer processor failed")]
    InstallerProcessorFailed {
        name: Gav,
        output: Box<Output>,
    },
    #[error("installer processor invalid output")]
    InstallerProcessorInvalidOutput {
        name: Gav,
        file: Box<Path>,
        expected_sha1: Box<[u8; 20]>,
    }
}

impl<T: Into<mojang::Error>> From<T> for Error {
    fn from(value: T) -> Self {
        Self::Mojang(value.into())
    }
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Represent an abstract maven-based and installer-based forge-like loader API. There
/// are currently only two APIs, Forge and NeoForge and this cannot be implemented by
/// other crates because this APIs is unstable.
/// 
/// Internally, we are using function pointers, like a vtable but without any data, this 
/// avoid using a trait with dyn references and sealed traits.
pub struct Api {
    /// Default prefix for the full root version id of the format 
    /// '<default prefix>-<game version>-<loader version>.
    default_prefix: &'static str,
    /// If version in the maven-manifest.xml file are known to be in reverse order, this
    /// helps iterating versions from the more recent ones to older ones.
    maven_manifest_reverse_order: bool,
    /// The base path for the maven group directory, relative to libraries directory.
    maven_group_base_dir: &'static str,
    /// The base URL for the maven group directory, without leading slash.
    /// This should've been a `&'static Path` but apparently we can't..
    maven_group_base_url: &'static str,
    /// Get the maven artifact, from 
    maven_artifact: fn(game_version: &str, game_major: u16, game_minor: u16) -> &'static str,
    /// Build the full loader version from the short loader version given explicitly.
    build_loader_version: fn(game_version: &str, game_major: u16, game_minor: u16, short_loader_version: &str) -> String,
    /// Build the expected prefix to all maven version for the given game version.
    build_loader_version_prefix: fn(game_version: &str, game_major: u16, game_minor: u16) -> String,
    /// Return true if the given loader version is stable.
    is_loader_version_stable: fn(game_version: &str, game_major: u16, game_minor: u16, loader_version: &str) -> bool,
    /// See [`InstallConfig`].
    install_config: fn(game_version: &str, game_major: u16, game_minor: u16, loader_version: &str) -> InstallConfig,
}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Api").finish()
    }
}

/// Various configuration options for a loader version, it's used to enable or disable
/// various checks
#[derive(Debug)]
struct InstallConfig {
    /// If the [`standard::Installer`] runs successfully, this bool is used to determine
    /// if some important libraries should be checked anyway, if those libraries are 
    /// absent the installer tries to reinstall.
    check_libraries: bool,
    /// If `check_libraries` is true, and this is true, the given ladder version is known
    /// to put its "extra" generated artifact inside the MCP-versioned game version
    /// inside `net.minecraft:client`.
    extra_in_mcp: bool,
    /// Set to true if this loader is expected to have a legacy install profile.
    legacy_install_profile: bool,
    /// Set to true when the installer processors should be checked, this exists because
    /// some old versions systematically generate wrong SHA-1 and we prefer allowing 
    /// these versions to be installed even if files might be invalid.
    check_processor_outputs: bool,
}

/// The original forge API.
pub static FORGE_API: Api = Api {
    default_prefix: "forge",
    maven_manifest_reverse_order: false,
    maven_group_base_dir: const_path!("net", "minecraftforge"),
    maven_group_base_url: "https://maven.minecraftforge.net/net/minecraftforge",
    maven_artifact: |_game_version: &str, _game_major: u16, _game_minor: u16| {
        "forge"
    },
    build_loader_version: |game_version: &str, _game_major: u16, _game_minor: u16, short_loader_version: &str| {
        if game_version == "1.7.10-pre4" {
            // This is the only prerelease ever supported by Forge.
            // Note however that this version seems to be broken, but anyway we support it!
            format!("1.7.10_pre4-{short_loader_version}")
        } else {
            format!("{game_version}-{short_loader_version}")
        }
    },
    build_loader_version_prefix: |game_version: &str, _game_major: u16, _game_minor: u16| {
        if game_version == "1.7.10-pre4" {
            "1.7.10_pre4".to_string()
        } else {
            format!("{game_version}-")
        }
    },
    is_loader_version_stable: |_game_version: &str, _game_major: u16, _game_minor: u16, _loader_version: &str| {
        true  // All versions are stable
    },
    install_config: |_game_version: &str, _game_major: u16, _game_minor: u16, loader_version: &str| {
        let loader_version = parse_forge_loader_version(loader_version);
        InstallConfig {
            // The first version to actually use processors was 1.13.2-25.0.9, therefore
            // we only check libraries for this version after onward.
            check_libraries: loader_version.map(|v| v >= [25, 0, 0, 0]).unwrap_or(false),
            // The 'extra' classifier is stored in different directories depending on version:
            // v >= 1.16.1-32.0.20: inside '<game_version>-<mcp_version>'
            // v <= 1.16.1-32.0.19: inside '<game_version>'
            extra_in_mcp: loader_version.map(|v| v >= [32, 0, 20, 0]).unwrap_or(false),
            // The install profiles comes in multiples forms:
            // >= 1.12.2-14.23.5.2851: There are two files, 'install_profile.json' which 
            //  contains processors and shared data, and `version.json` which is the raw 
            //  version meta to be fetched.
            // <= 1.12.2-14.23.5.2847: There is only an 'install_profile.json' with the
            //  version meta stored in 'versionInfo' object. Each library have two keys 
            //  'serverreq' and 'clientreq' that should be removed when the profile is 
            //  returned.
            legacy_install_profile: loader_version.map(|v| v <= [14, 23, 5, 2847]).unwrap_or(false),
            // v >= 1.14.4-28.1.16: hashes are valid
            // 1.13 <= v <= 1.14.4-28.1.15: hashes are invalid
            // 1.12.2-14.23.5.2851 <= v < 1.13: no processor therefore no hash to check
            // v <= 1.12.2-14.23.5.2847: legacy installer, no processor
            check_processor_outputs: loader_version.map(|v| v >= [28, 1, 16, 0]).unwrap_or(false),
        }
    },
};

/// The forked forge API, called NeoForge. The special case of 1.20.1 is properly handled
/// under the legacy artifact ID 'forge', subsequent versions are handled as 'neoforge'
/// with the new loader versioning.
pub static NEO_FORGE_API: Api = Api {
    default_prefix: "neoforge",
    maven_manifest_reverse_order: true,
    maven_group_base_dir: const_path!("net", "neoforged"),
    maven_group_base_url: "https://maven.neoforged.net/releases/net/neoforged",
    maven_artifact: |_game_version: &str, game_major: u16, game_minor: u16| {
        if game_major == 20 && game_minor == 1 {
            "forge"
        } else {
            "neoforge"
        }
    },
    build_loader_version: |_game_version: &str, game_major: u16, game_minor: u16, short_loader_version: &str| {
        if game_major == 20 && game_minor == 1 {
            format!("1.20.1-{short_loader_version}")
        } else {
            format!("{game_major}.{game_minor}.{short_loader_version}")
        }
    },
    build_loader_version_prefix: |_game_version: &str, game_major: u16, game_minor: u16| {
        if game_major == 20 && game_minor == 1 {
            format!("1.20.1-")
        } else {
            format!("{game_major}.{game_minor}.")
        }
    },
    is_loader_version_stable: |_game_version: &str, game_major: u16, game_minor: u16, loader_version: &str| {
        if game_major == 20 && game_minor == 1 {
            true
        } else {
            !loader_version.ends_with("-beta")
        }
    },
    install_config: |_game_version: &str, _game_major: u16, _game_minor: u16, _loader_version: &str| {
        InstallConfig {
            check_libraries: true,
            extra_in_mcp: true,
            legacy_install_profile: false,
            check_processor_outputs: true,
        }
    },
};

/// Specify the forge game version to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameVersion {
    /// Use the latest Mojang's release to start the game.
    Release,
    /// Use the specific version.
    Name(String),
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
    /// Use the specific version. The exact meaning of this depends on the actual API
    /// being used:
    /// 
    /// - With [`Api::Forge`], the name represent the full loader version that is appended
    ///   to the game version, like in `1.21-51.0.33`, the loader version is `51.0.33`.
    ///   Some rare loader versions are strange and are also suffixed by some string
    ///   related to the game version, like `1.11-13.19.0.2129-1.11.x`, because it don't
    ///   make sense to specify a loader version `13.19.0.2129-1.11.x`, you can simply 
    ///   specify `13.19.0.2129` and these case will be handled silently, this suffix
    ///   won't be included in the root version.
    /// 
    /// - With [`Api::NeoForge`], the name represent the last "patch" number of the loader.
    ///   NeoForge versioning consists in the Minecraft major and minor version (ignoring
    ///   the first '1.'), and the loader patch. For example NeoForge loader `20.4.181`,
    ///   the game version is `1.20.4` and loader version name is `181`.
    Name(String),
}

/// The reason for (re)installing the mod loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallReason {
    /// The root version metadata is missing, the load was probably not installed before.
    MissingVersionMetadata,
    /// The core library is missing, this exists on some loader versions and should've
    /// been extracted from the installer. Reinstalling.
    MissingCoreLibrary,
    /// The client extra artifact is missing.
    MissingClientExtra,
    /// The client srg artifact is missing.
    MissingClientSrg,
    /// The patched client is missing.
    MissingPatchedClient,
    /// The universal client is missing.
    MissingUniversalClient,
}

// ========================== //
// Following code is internal //
// ========================== //

/// Internal function to request, parse and create an iterator over all versions specified
/// in a maven metadata.
fn request_maven_metadata(xml_url: &str) -> Result<MavenMetadata> {
    crate::tokio::sync(async move {
        
        let text = crate::http::client()?
            .get(xml_url)
            .header(reqwest::header::ACCEPT, "application/xml")
            .send().await?
            .error_for_status()?
            .text().await?;

        MavenMetadata::try_from_xml(text)
            .ok_or_else(|| Error::MavenMetadataMalformed {  })

    })
}

/// Try installing the mod loader.
fn try_install(
    handler: &mut impl Handler,
    mojang: &mut mojang::Installer,
    api: &'static Api,
    artifact: &str,
    root_version: &str,
    game_version: &str,
    loader_version: &str,
    install_config: &InstallConfig,
    side: serde::InstallSide,
    reason: InstallReason,
) -> Result<()> {

    let tmp_dir = env::temp_dir().joined(root_version);
    handler.handle_forge_event(Event::Installing {
        tmp_dir: &tmp_dir,
        reason,
    });

    // The first thing we do is fetching the installer, so it ends early if there is 
    // simply no installer for this version!
    handler.handle_forge_event(Event::InstallerFetching {
        game_version,
        loader_version,
    });

    let installer_url = format!("{base}/{artifact}/{loader_version}/{artifact}-{loader_version}-installer.jar", 
        base = api.maven_group_base_url);
    
    // Download and check result in case installer is just not found.
    let entry = download::single(installer_url, tmp_dir.join("installer.jar"))
        .set_keep_open()
        .download(handler.as_download_dyn())?;

    let mut entry = match entry {
        Ok(entry) => entry,
        Err(e) => {
            if let EntryErrorKind::InvalidStatus(StatusCode::NOT_FOUND) = e.kind() {
                return Err(Error::LoaderVersionNotFound { 
                    game_version: game_version.to_string(), 
                    loader_version: loader_version.to_string(),
                });
            } else {
                return Err(e.into());
            }
        }
    };

    let installer_reader = BufReader::new(entry.take_handle().unwrap());
    let installer_file = entry.file();
    let mut installer_zip = ZipArchive::new(installer_reader)
        .map_err(|e| standard::Error::new_zip_file(e, installer_file))?;

    handler.handle_forge_event(Event::InstallerFetched {
        game_version,
        loader_version,
    });

    handler.handle_forge_event(Event::GameInstalling {  });

    // We need to ensure that the underlying game version is fully installed. Here we
    // just forward the handler as-is, and we check for version not found to warn
    // about an non-existing game version. We keep the installed, or found, JVM exec
    // for later execution of installer processors. Note that the JVM exec path should
    // be already canonicalized.
    mojang.set_root_version(RootVersion::Name(game_version.to_string()));
    let jvm_file = match mojang.install(handler.as_mojang_dyn()) {
        Err(e) => return Err(Error::Mojang(e)),
        Ok(game) => game.jvm_file,
    };

    const PROFILE_ENTRY: &str = "install_profile.json";
    let profile = match installer_zip.by_name(PROFILE_ENTRY) {
        Ok(reader) => {
            
            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            let res = if install_config.legacy_install_profile {
                serde_path_to_error::deserialize::<_, serde::LegacyInstallProfile>(&mut deserializer)
                    .map(InstallProfileKind::Legacy)
            } else {
                serde_path_to_error::deserialize::<_, serde::ModernInstallProfile>(&mut deserializer)
                    .map(InstallProfileKind::Modern)
            };

            res.map_err(|e| standard::Error::new_json(e, format!("entry: {}, from: {}", 
                PROFILE_ENTRY, 
                installer_file.display())))?

        }
        Err(_) => return Err(Error::InstallerProfileNotFound {  })
    };

    // The installer directly installs libraries to these directories.
    // We canonicalize the libs path here, this avoids doing it after each join.
    let libraries_dir = standard::canonicalize_file(mojang.standard().libraries_dir())?;
    let game_version_dir = mojang.standard().versions_dir().join(&game_version);
    let game_client_file = game_version_dir.join_with_extension(&game_version, "jar");
    let root_version_dir = mojang.standard().versions_dir().join(&root_version);
    let metadata_file = root_version_dir.join_with_extension(&root_version, "json");
    let mut metadata;

    match profile {
        InstallProfileKind::Modern(profile) => {
            
            if profile.minecraft != game_version {
                return Err(Error::InstallerProfileIncoherent {  });
            }

            // Immediately try, and keep the version metadata, this avoid launching this
            // error at the end after all the processing happened.
            let metadata_entry = profile.json.strip_prefix('/').unwrap_or(&profile.json);
            metadata = match installer_zip.by_name(metadata_entry) {
                Ok(reader) => {
                    let mut deserializer = serde_json::Deserializer::from_reader(reader);
                    serde_path_to_error::deserialize::<_, Box<standard::serde::VersionMetadata>>(&mut deserializer)
                        .map_err(|e| standard::Error::new_json(e, format!("entry: {}, from: {}",
                            metadata_entry,
                            installer_file.display())))?
                }
                Err(_) => return Err(Error::InstallerVersionMetadataNotFound {  })
            };

            handler.handle_forge_event(Event::InstallerLibrariesFetching {  });
            
            // Some early (still modern) installers (<= 1.16.5) embed the forge universal
            // JAR, we need to extract it given its path. It also appears that more modern
            // versions have this property back...
            if let Some(name) = &profile.path {
                let lib_file = name.file(&libraries_dir);
                extract_installer_maven_artifact(installer_file, &mut installer_zip, name, &lib_file)?;
            }

            // We keep as map of libraries to their file path, this is also used because
            // some NeoForge installers have been seen to have duplicated library.
            let mut libraries = HashMap::new();
            let mut batch = Batch::new();

            for lib in &profile.libraries {

                // Ignore duplicated libs, see above.
                if libraries.contains_key(&lib.name) {
                    continue
                }

                let lib_dl = &lib.downloads.artifact;

                let lib_file = if let Some(lib_path) = &lib_dl.path {
                    // NOTE: Unsafe joining!
                    libraries_dir.join(lib_path)
                } else {
                    lib.name.file(&libraries_dir)
                };

                libraries.insert(&lib.name, lib_file.clone());
                
                if !lib_dl.download.url.is_empty() {
                    batch.push(lib_dl.download.url.to_string(), lib_file)
                        .set_expect_size(lib_dl.download.size)
                        .set_expect_sha1(lib_dl.download.sha1.as_deref().copied());
                } else {
                    extract_installer_maven_artifact(installer_file, &mut installer_zip, &lib.name, &lib_file)?;
                }

            }

            // Download all libraries just before running post processors.
            if !batch.is_empty() {
                batch.download(handler.as_download_dyn())?.into_result()?;
            }

            handler.handle_forge_event(Event::InstallerLibrariesFetched {  });

            // Parse data entries...
            let mut data = HashMap::with_capacity(profile.data.len());
            for (name, entry) in &profile.data {
                let entry = entry.get(side);
                let kind = match entry.as_bytes() {
                    [b'[', .., b']'] => {
                        if let Ok(gav) = entry[1..entry.len() - 1].parse::<Gav>() {
                            InstallDataTypedEntry::Library(gav)
                        } else {
                            // Gently ignore the error as it should never happen.
                            continue;
                        }
                    }
                    [b'\'', .., b'\''] => {
                        InstallDataTypedEntry::Literal(entry[1..entry.len() - 1].to_string())
                    }
                    _ => {
                        // This is a file that we should extract to the temp directory.
                        // NOTE: Unsafe joining.
                        let entry = entry.strip_prefix('/').unwrap_or(entry);
                        let tmp_file = tmp_dir.join(entry);
                        extract_installer_file(installer_file, &mut installer_zip, entry, &tmp_file)?;
                        InstallDataTypedEntry::File(tmp_file)
                    }
                };
                data.insert(name.clone(), kind);
            }

            // Builtin entries.
            data.insert("SIDE".to_string(), InstallDataTypedEntry::Literal(side.as_str().to_string()));
            data.insert("MINECRAFT_JAR".to_string(), InstallDataTypedEntry::File(game_client_file));
            data.insert("MINECRAFT_VERSION".to_string(), InstallDataTypedEntry::Literal(game_version.to_string()));
            // Currently no support for ROOT because it's apparently used only for server...
            // data.insert("ROOT".to_string(), InstallDataTypedEntry::File(mojang.standard().));
            data.insert("INSTALLER".to_string(), InstallDataTypedEntry::File(installer_file.to_path_buf()));
            data.insert("LIBRARY_DIR".to_string(), InstallDataTypedEntry::File(libraries_dir.to_path_buf()));

            // Now we process each post-processor in order, each processor will refer to
            // one of the library installed earlier.
            for processor in &profile.processors {

                if let Some(processor_sides) = &processor.sides {
                    if !processor_sides.iter().copied().any(|processor_side| processor_side == side) {
                        continue
                    }
                }

                let Some(jar_file) = libraries.get(&processor.jar) else {
                    return Err(Error::InstallerInvalidProcessor {
                        name: processor.jar.clone(),
                    });
                };

                let Some(main_class) = find_jar_main_class(&jar_file)? else {
                    return Err(Error::InstallerInvalidProcessor {
                        name: processor.jar.clone(),
                    });
                };

                let mut classes = vec![jar_file.as_path()];
                for dep_name in &processor.classpath {
                    if let Some(dep_path) = libraries.get(dep_name) {
                        classes.push(dep_path.as_path());
                    } else {
                        return Err(Error::InstallerInvalidProcessor {
                            name: processor.jar.clone(),
                        });
                    }
                }

                let class_path = env::join_paths(classes).unwrap();

                // Find a debug-purpose processor task name...
                let task = if processor.args.len() >= 2 && processor.args[0] == "--task" {
                    Some(processor.args[1].as_str())
                } else {
                    None
                };

                handler.handle_forge_event(Event::InstallerProcessor {
                    name: &processor.jar,
                    task,
                });

                // Construct the command to run the processor.
                let mut command = Command::new(&jvm_file);
                command
                    .arg("-cp")
                    .arg(class_path)
                    .arg(&main_class);

                for arg in &processor.args {
                    if let Some(arg) = format_processor_arg(&arg, &libraries_dir, &data) {
                        command.arg(arg);
                    } else {
                        // Ignore malformed arguments for now.
                        command.arg(arg);
                    }
                }

                let output = command.output()
                    .map_err(|e| standard::Error::new_io(e, format!("spawn: {}", jvm_file.display())))?;

                if !output.status.success() {
                    return Err(Error::InstallerProcessorFailed {
                        name: processor.jar.clone(),
                        output: Box::new(output),
                    });
                }

                // If process SHA-1 check is enabled...
                if install_config.check_processor_outputs {
                    for (file, sha1) in &processor.outputs {
                        let Some(file) = format_processor_arg(&file, &libraries_dir, &data) else { continue };
                        let Some(sha1) = format_processor_arg(&sha1, &libraries_dir, &data) else { continue };
                        let Some(sha1) = crate::serde::parse_hex_bytes::<20>(&sha1) else { continue };
                        let file = Path::new(&file);
                        if !standard::check_file(file, None, Some(&sha1))? {
                            return Err(Error::InstallerProcessorInvalidOutput {
                                name: processor.jar.clone(),
                                file: file.to_path_buf().into_boxed_path(),
                                expected_sha1: Box::new(sha1),
                            });
                        }
                    }
                }
                
            }

        }
        InstallProfileKind::Legacy(profile) => {
            
            metadata = profile.version_info;

            // Older versions used to require libraries that are no longer installed
            // by parent versions, therefore it's required to add url if not 
            // provided, pointing to maven central repository, for downloading.
            for lib in &mut metadata.libraries {
                if lib.url.is_none() {
                    lib.url = Some(LIBRARIES_URL.to_string());
                }
            }

            // Old version (<= 1.6.4) of forge are broken, even on official launcher.
            // So we fix them by manually adding the correct inherited version.
            if metadata.inherits_from.is_none() {
                metadata.inherits_from = Some(game_version.to_string());
            }

            // Extract the universal JAR file of the mod loader.
            let jar_file = profile.install.path.file(libraries_dir);
            let jar_entry = &profile.install.file_path[..];
            extract_installer_file(installer_file, &mut installer_zip, &jar_entry, &jar_file)?;

        }
    }

    metadata.id = root_version.to_string();
    standard::write_version_metadata(&metadata_file, &metadata)?;

    handler.handle_forge_event(Event::Installed {  });

    Ok(())

}

#[derive(Debug)]
enum InstallProfileKind {
    Modern(serde::ModernInstallProfile),
    Legacy(serde::LegacyInstallProfile),
}

/// Internal install data.
#[derive(Debug)]
enum InstallDataTypedEntry {
    /// The data is referencing a library.
    Library(Gav),
    /// The value is a literal value.
    Literal(String),
    /// The value is a file.
    File(PathBuf),
}

/// Format a processor argument, NOTE THAT it is directly implemented, especially from
/// `net.minecraftforge.installer.json.Util.replaceToken` class inside the installer.
fn format_processor_arg(
    input: &str, 
    libraries_dir: &Path, 
    data: &HashMap<String, InstallDataTypedEntry>
) -> Option<String> {

    if matches!(input.as_bytes(), [b'[', .., b']']) {
        let gav = input[1..input.len() - 1].parse::<Gav>().ok()?;
        return Some(format!("{}", gav.file(libraries_dir).display()));
    }

    #[derive(Debug)]
    enum TokenKind {
        Data,
        Literal,
    }

    let mut global_buf = String::new();
    let mut token_buf = String::new();
    let mut token = None;
    let mut escape = false;

    for (index, ch) in input.char_indices() {
        match ch {
            '\\' if !escape => {
                if index == input.len() - 1 {
                    return None;
                }
                escape = true;
            }
            '{' if !escape && token.is_none() => {
                token = Some(TokenKind::Data);
            }
            '}' if !escape && matches!(token, Some(TokenKind::Data)) => {
                match data.get(&token_buf)? {
                    InstallDataTypedEntry::Library(gav) => {
                        write!(global_buf, "{}", gav.file(libraries_dir).display()).unwrap();
                    }
                    InstallDataTypedEntry::Literal(lit) => {
                        global_buf.push_str(lit);
                    }
                    InstallDataTypedEntry::File(path_buf) => {
                        write!(global_buf, "{}", path_buf.display()).unwrap();
                    }
                }
                token_buf.clear();
                token = None;
            }
            '\'' if !escape && token.is_none() => {
                token = Some(TokenKind::Literal);
            }
            '\'' if !escape && matches!(token, Some(TokenKind::Literal)) => {
                global_buf.push_str(&token_buf);
                token_buf.clear();
                token = None;
            }
            _ => {
                if token.is_none() {
                    global_buf.push(ch);
                } else {
                    token_buf.push(ch);
                }
                escape = false;
            }
        }
    }

    Some(global_buf)

}


/// For the modern installer, extract from its archive the given artifact to the library
/// directory.
fn extract_installer_maven_artifact<R: Read + Seek>(
    installer_file: &Path,
    installer_zip: &mut ZipArchive<R>,
    src_name: &Gav,
    dst_file: &Path,
) -> Result<()> {

    let src_entry = {
        let mut entry_buf = "maven".to_string();
        for comp in src_name.file_components() {
            entry_buf.push('/');
            entry_buf.push_str(&*comp);
        }
        entry_buf
    };

    extract_installer_file(installer_file, installer_zip, &src_entry, dst_file)

}

/// Extract an installer file from its archive.
fn extract_installer_file<R: Read + Seek>(
    installer_file: &Path,
    installer_zip: &mut ZipArchive<R>,
    src_entry: &str,
    dst_file: &Path,
) -> Result<()> {

    let mut reader = installer_zip.by_name(&src_entry)
        .map_err(|_| Error::InstallerFileNotFound { 
            entry: src_entry.to_string(),
        })?;

    let parent_dir = dst_file.parent().unwrap();
    fs::create_dir_all(parent_dir)
        .map_err(|e| standard::Error::new_io_file(e, parent_dir))?;

    let mut writer = File::create(dst_file)
        .map_err(|e| standard::Error::new_io_file(e, dst_file))
        .map(BufWriter::new)?;

    io::copy(&mut reader, &mut writer)
        .map_err(|e| standard::Error::new_io(e, format!("extract: {}, from: {}", 
            src_entry, 
            installer_file.display())))?;

    Ok(())

}

/// From a JAR file path, open it and try to find the main class path from the manifest.
fn find_jar_main_class(jar_file: &Path) -> Result<Option<String>> {

    let jar_reader = File::open(jar_file)
        .map_err(|e| standard::Error::new_io_file(e, jar_file))
        .map(BufReader::new)?;

    let mut jar_zip = ZipArchive::new(jar_reader)
        .map_err(|e| standard::Error::new_zip_file(e, jar_file))?;

    let Ok(mut manifest_reader) = jar_zip.by_name("META-INF/MANIFEST.MF")
        .map(BufReader::new) else {
            // The manifest was not found, is should NEVER happen, we ignore this.
            return Ok(None);
        };
    
    const MAIN_CLASS_KEY: &str = "Main-Class: ";

    let mut line = String::new();
    while manifest_reader.read_line(&mut line).unwrap_or(0) != 0 {
        if line.starts_with(MAIN_CLASS_KEY) {
            if let Some(last_non_whitespace) = line.rfind(|c: char| !c.is_whitespace()) {
                line.truncate(last_non_whitespace + 1);
                line.drain(0..MAIN_CLASS_KEY.len());
                return Ok(Some(line))
            } else {
                // The main class is empty?
                return Ok(None);
            }
        }
        line.clear();
    }

    Ok(None)
    
}

/// Generic version parsing with dot separator and default value to zero.
fn parse_generic_version<const MAX: usize, const MIN: usize>(version: &str) -> Option<[u16; MAX]> {
    let mut it = version.split('.');
    let mut ret = [0; MAX];
    for i in 0..MAX {
        ret[i] = match it.next() {
            Some(raw) => raw.parse::<u16>().ok()?,
            None if i < MIN => return None,
            None => 0,
        };
    }
    Some(ret)
}

/// Internal function that parses the game version major and minor version numbers, if
/// the version starts with "1.", returning 0 for minor version is not present.
fn parse_game_version(version: &str) -> Option<[u16; 2]> {
    let version = version.strip_prefix("1.")?;
    let (version, _rest) = version.split_once("-pre").unwrap_or((version, ""));
    parse_generic_version::<2, 1>(version)
}

/// Parse the 4 digits of a loader version, ignoring the game version and optional suffix.
fn parse_forge_loader_version(version: &str) -> Option<[u16; 4]> {
    let (_game_version, version) = version.split_once('-')?;
    let (version, _rest) = version.split_once('-').unwrap_or((version, ""));
    parse_generic_version::<4, 2>(version)
}
