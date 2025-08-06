//! Extension to the Mojang installer to support fetching and installation of 
//! Forge and NeoForge mod loader versions.

mod serde;

use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek};
use std::process::{Command, Output};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::iter::FusedIterator;
use std::fmt::Write;
use std::{env, fs};
use std::fs::File;

use crate::mojang::{self, FetchExclude, HandlerInto as _};
use crate::download::{self, Batch, EntryErrorKind};
use crate::base::{self, Game, LIBRARIES_URL};
use crate::maven::{Gav, MetadataParser};
use crate::path::{PathBufExt, PathExt};

use zip::ZipArchive;

use elsa::sync::FrozenMap;


/// An installer that supports Forge and NeoForge mod loaders.
#[derive(Debug, Clone)]
pub struct Installer {
    /// The underlying Mojang installer logic.
    mojang: mojang::Installer,
    /// The forge loader to install.
    loader: Loader,
    /// The forge installer version description.
    version: Version,
}

impl Installer {

    /// Create a new installer with default configuration.
    pub fn new(loader: Loader, version: impl Into<Version>) -> Self {
        Self {
            // Empty version by default, will be set at install.
            mojang: mojang::Installer::new(String::new()),
            loader,
            version: version.into(),
        }
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

    /// Get the loader version that will be installed.
    #[inline]
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Change the loader version that will be installed.
    #[inline]
    pub fn set_version(&mut self, version: impl Into<Version>) -> &mut Self {
        self.version = version.into();
        self
    }

    /// Install the currently configured Forge/NeoForge loader with the given handler.
    #[inline]
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {
        self.install_dyn(&mut handler)
    }

    #[inline(never)]
    fn install_dyn(&mut self, handler: &mut dyn Handler) -> Result<Game> {

        let Self {
            ref mut mojang,
            loader,
            ref version,
        } = *self;

        // Request the repository if needed!
        let version = match version {
            Version::Name(name) => name.clone(),
            Version::Stable(game_version) |
            Version::Unstable(game_version) => {
                let stable = matches!(version, Version::Stable(_));
                match Repo::request(loader)?.find_latest(&game_version, stable) {
                    Some(v) => v.name().to_string(),
                    None => return Err(Error::LatestVersionNotFound { 
                        game_version: game_version.clone(), 
                        stable,
                    }),
                }
            }
        };

        let config = match loader {
            Loader::Forge => InstallConfig::new_forge(&version),
            Loader::NeoForge => InstallConfig::new_neoforge(&version),
        };
        
        // Shortcut because the version name is invalid and there will be no installer or
        // that installer is not supported.
        let Some(config) = config else {
            return Err(Error::InstallerNotFound { version });
        };

        // Construct the root version id.
        let prefix = config.default_prefix;
        let root_version = format!("{prefix}-{version}");

        // Adding it to fetch exclude, we don't want to try to fetch it from Mojang's 
        // manifest: it's pointless and it avoids trying to fetch the manifest.
        mojang.add_fetch_exclude(FetchExclude::Exact(root_version.clone()));

        // The goal is to run the installer a first time, check potential errors to 
        // know if the error is related to the loader, or not.
        mojang.set_version(root_version.clone());
        let reason = match mojang.install((&mut *handler).into_mojang()) {
            Ok(game) => {

                if !config.check_libraries {
                    return Ok(game);
                }

                // Using this outer loop to break when some reason to install is met.
                loop {

                    fn check_exists(file: &Path) -> bool {
                        fs::exists(file).unwrap_or_default()
                    }

                    let libs_dir = mojang.base().libraries_dir();
                    
                    // Start by checking patched client and universal client.
                    if !check_exists(&config.gav.with_classifier(Some("client")).file(libs_dir)) {
                        break InstallReason::MissingPatchedClient;
                    }

                    if !check_exists(&config.gav.with_classifier(Some("universal")).file(libs_dir)) {
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
                        
                        let mcp_artifact = libs_dir
                            .join("net")
                            .joined("minecraft")
                            .joined("client")
                            .joined(&config.game_version)
                                .appended("-")
                                .appended(mcp_version)
                            .joined("client")
                                .appended("-")
                                .appended(&config.game_version)
                                .appended("-")
                                .appended(mcp_version)
                                .appended("-");

                        if !check_exists(&mcp_artifact.append("srg.jar")) {
                            break InstallReason::MissingClientSrg;
                        }

                        if config.extra_in_mcp {
                            if !check_exists(&mcp_artifact.append("extra.jar")) {
                                break InstallReason::MissingClientExtra;
                            }
                        } else {

                            let mc_artifact = libs_dir
                                .join("net")
                                .joined("minecraft")
                                .joined("client")
                                .joined(&config.game_version)
                                .joined("client")
                                .appended("-")
                                .appended(&config.game_version)
                                .appended("-");

                            if !check_exists(&mc_artifact.append("extra.jar"))
                            && !check_exists(&mc_artifact.append("extra-stable.jar")) {
                                break InstallReason::MissingClientExtra;
                            }

                        }

                    }

                    // No reason to reinstall, we return the game as-is.
                    return Ok(game);

                }

            }
            Err(mojang::Error::Base(base::Error::VersionNotFound { version })) 
            if version == root_version => {
                InstallReason::MissingVersionMetadata
            }
            Err(mojang::Error::Base(base::Error::LibraryNotFound { gav })) 
            if gav.group() == "net.minecraftforge" && gav.artifact() == "forge" => {
                InstallReason::MissingCoreLibrary
            }
            Err(e) => return Err(Error::Mojang(e))
        };

        try_install(&mut *handler, &mut *mojang, &config, &root_version, serde::InstallSide::Client, reason)?;

        // Retrying launch!
        mojang.set_version(root_version);
        let game = mojang.install((&mut *handler).into_mojang())?;
        Ok(game)

    }

}

/// Events happening when installing.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// Forwarding a mojang event.
    Mojang(mojang::Event<'a>),
    /// The loader version failed to start, so this installer will (re)try to install
    /// the mod loader.
    Installing { tmp_dir: &'a Path, reason: InstallReason },
    /// The loader installer will be fetched.
    FetchInstaller { version: &'a str },
    /// The loader installer has been successfully fetched.
    FetchedInstaller { version: &'a str},
    /// Notify that the game will be installed manually before running the installer,
    /// because the installer needs it.
    InstallingGame,
    /// The loader installer libraries will be fetched, either from being download, 
    /// or being extracted from the installer archive.
    FetchInstallerLibraries,
    /// The loader installer libraries has been successfully fetched or extracted.
    FetchedInstallerLibraries,
    /// An installer processor will be run.
    RunInstallerProcessor { name: &'a Gav, task: Option<&'a str> },
    /// The mod loader has been apparently successfully installed, it will be run a 
    /// second time to try...
    Installed,
}

/// A handle for watching an installation.
pub trait Handler {
    /// Handle a single event.
    fn on_event(&mut self, event: Event);
}

// Mutable implementation.
impl<H: Handler + ?Sized> Handler for &mut H {
    #[inline]
    fn on_event(&mut self, event: Event) {
        (**self).on_event(event)
    }
}

impl Handler for () {
    fn on_event(&mut self, event: Event) {
        let _ = event;
    }
}

/// Internal adapter trait for using it like other handlers.
#[allow(unused)]
pub(crate) trait HandlerInto: Handler + Sized {
    
    #[inline]
    fn into_mojang(self) -> impl mojang::Handler {
        pub(crate) struct Adapter<H: Handler>(pub H);
        impl<H: Handler> mojang::Handler for Adapter<H> {
            fn on_event(&mut self, event: mojang::Event) {
                self.0.on_event(Event::Mojang(event));
            }
        }
        Adapter(self)
    }
    
    #[inline]
    fn into_base(self) -> impl base::Handler {
        self.into_mojang().into_base()
    }

    #[inline]
    fn into_download(self) -> impl download::Handler {
        self.into_mojang().into_download()
    }

}

impl<H: Handler> HandlerInto for H {}

/// The Forge installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the Mojang installer.
    #[error("mojang: {0}")]
    Mojang(#[source] mojang::Error),
    /// If the latest stable or unstable version is requested but doesn't exists.
    #[error("latest version not found for {game_version} (stable: {stable})")]
    LatestVersionNotFound {
        game_version: String,
        stable: bool,
    },
    /// The given loader version as requested to launch Forge with has not supported 
    /// installer.
    #[error("installer not found: {version}")]
    InstallerNotFound {
        version: String,
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

/// Type alias for a result with the Forge error type.
pub type Result<T> = std::result::Result<T, Error>;

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

/// Represent the different kind of loaders to install or fetch for versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    /// Targets the original Forge project.
    Forge,
    /// Targets the NeoForge project.
    NeoForge,
}

/// The version to install.
#[derive(Debug, Clone)]
pub enum Version {
    /// Launch the latest stable version for the given game version.
    Stable(String),
    /// Launch the latest stable or unstable version for the given game version.
    Unstable(String),
    /// Raw forge loader version.
    Name(String),
}

impl<T: Into<String>> From<T> for Version {
    fn from(value: T) -> Self {
        Self::Name(value.into())
    }
}

/// The version repository for Forge and NeoForge.
#[derive(Debug)]
pub struct Repo {
    /// The main metadata XML data.
    main_xml: String,
    /// The legacy metadata XML data, it's basically used only 
    legacy_xml: Option<String>,
    /// Special boolean specifying if the repository is the one of NeoForge, this affects
    /// how various things are resolved.
    neoforge: bool,
    /// Major versions temporary string map, used for NeoForge.
    major_versions: FrozenMap<[u16; 2], String>,
}

impl Repo {

    /// Request the repository for a given loader.
    pub fn request(loader: Loader) -> Result<Self> {
        match loader {
            Loader::Forge => Self::request_forge(),
            Loader::NeoForge => Self::request_neoforge(),
        }
    }

    /// Request the online Forge repository.
    fn request_forge() -> Result<Self> {

        // This entry doesn't really support caching, but we use this so we can access
        // the resource while being offline.
        let mut main_entry = download::single_cached("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")
            .set_keep_open()
            .download(())?;

        let main_xml = main_entry.read_handle_to_string().unwrap()
            .map_err(|e| base::Error::new_io_file(e, main_entry.file()))?;

        Ok(Self {
            main_xml,
            legacy_xml: None,
            neoforge: false,
            major_versions: FrozenMap::new(),
        })

    }

    /// Request the online NeoForge repository.
    fn request_neoforge() -> Result<Self> {

        // See comment above about caching.
        let mut batch = download::Batch::new();
        batch.push_cached("https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml").set_keep_open();
        batch.push_cached("https://maven.neoforged.net/releases/net/neoforged/forge/maven-metadata.xml").set_keep_open();
        
        let mut result = batch.download(())
            .map_err(|e| base::Error::new_reqwest(e, "request neoforge repo"))?
            .into_result()?;
        
        let main_entry = result.entry_mut(0).unwrap();
        let main_xml = main_entry.read_handle_to_string().unwrap()
            .map_err(|e| base::Error::new_io_file(e, main_entry.file()))?;

        let legacy_entry = result.entry_mut(1).unwrap();
        let legacy_xml = legacy_entry.read_handle_to_string().unwrap()
            .map_err(|e| base::Error::new_io_file(e, legacy_entry.file()))?;
    
        Ok(Self {
            main_xml,
            legacy_xml: Some(legacy_xml),
            neoforge: true,
            major_versions: FrozenMap::new(),
        })

    }

    /// Return an iterator over all loaders in the repository, the iteration order is not
    /// consistent between Forge and NeoForge.
    pub fn iter(&self) -> RepoIter<'_> {
        RepoIter {
            main: MetadataParser::new(&self.main_xml),
            legacy: self.legacy_xml.as_deref().map(MetadataParser::new),
            repo: self,
        }
    }

    /// Return the repository version that has this exact name.
    pub fn find_by_name(&self, name: &str) -> Option<RepoVersion<'_>> {
        self.iter().find(|v| v.name() == name)
    }

    /// Find the latest loader version given, optionally with a specified game version
    /// and stable or not. Note that the latest stable version is also the latest unstable
    /// one if no version is unstable before it.
    pub fn find_latest(&self, game_version: &str, stable: bool) -> Option<RepoVersion<'_>> {

        // Parse the game version to build a prefix to match versions against.
        let [major, minor] = parse_game_version(game_version)?;
        let prefix = if self.neoforge {
            if major == 20 && minor == 1 {
                format!("1.20.1-")
            } else {
                format!("{major}.{minor}.")
            }
        } else {
            if game_version == "1.7.10-pre4" {
                format!("1.7.10_pre4-")
            } else {
                format!("{game_version}-")
            }
        };

        let mut it = self.iter()
            .filter(|v| v.name().starts_with(&prefix))
            .filter(|v| !stable || v.is_stable());

        // NeoForge has latest versions last.
        if self.neoforge {
            it.last()
        } else {
            it.next()
        }

    }

}

/// An iterator over all loader versions in this repository.
#[derive(Debug)]
pub struct RepoIter<'a> {
    main: MetadataParser<'a>,
    legacy: Option<MetadataParser<'a>>,
    repo: &'a Repo,
}

impl<'a> Iterator for RepoIter<'a> {

    type Item = RepoVersion<'a>;
    
    fn next(&mut self) -> Option<Self::Item> {
        
        let version = match self.main.next() {
            Some(v) => v,
            None => self.legacy.as_mut()?.next()?,
        };

        Some(RepoVersion {
            repo: self.repo,
            version,
        })

    }

}

// Because 'MetadataParser' also implement this.
impl FusedIterator for RepoIter<'_> {  }

/// Reference to a version owned by the requested repository.
#[derive(Debug)]
pub struct RepoVersion<'a> {
    version: &'a str,
    repo: &'a Repo,
}

impl<'a> RepoVersion<'a> {

    /// Return the full name of this loader, containing both game and loader versions.
    /// Note that this naming is inconsistent.
    pub fn name(&self) -> &'a str {
        self.version
    }

    /// Get the game version from this version, the returned value might be allocated if
    /// the game version needs to be reconstructed.
    pub fn game_version(&self) -> &'a str {
        if self.repo.neoforge {
            // Special case from the legacy NeoForge repository where '1.20.1' is missing.
            if self.version == "47.1.82" || self.version.starts_with("1.20.1-") {
                "1.20.1"
            } else if let Some([major, minor]) = parse_generic_version::<2, 2>(self.version) {
                self.repo.major_versions.insert_with([major, minor], || {
                    if minor == 0 {
                        format!("1.{major}")
                    } else {
                        format!("1.{major}.{minor}")
                    }
                })
            } else {
                ""  // Should not happen
            }
        } else {
            match self.version.split_once('-') {
                // Special case with forge, this is the only pre-release supported.
                Some(("1.7.10_pre4", _)) => "1.7.10-pre4",
                Some((game_version, _)) => game_version,
                None => ""  // Should not happen
            }
        }
    }

    /// Return true if this version is stable.
    pub fn is_stable(&self) -> bool {
        if self.repo.neoforge {
            !self.version.ends_with("-beta")
        } else {
            true  // Forge is always stable
        }
    }

}

// ========================== //
// Following code is internal //
// ========================== //

/// Represent an abstract version that can be provided to the common Forge installer.
#[derive(Debug, Clone)]
struct InstallConfig {
    /// Default prefix for the full root version id of the format 
    /// '<default prefix>-<game version>-<loader version>.
    default_prefix: &'static str,
    /// The full name of this version.
    gav: Gav,
    /// The main maven repository URL where the installer artifact can be downloaded.
    /// Should not have a leading slash.
    repo_url: &'static str,
    /// The game version this loader version is patching.
    game_version: String,
    /// If the [`base::Installer`] runs successfully, this bool is used to determine
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

impl InstallConfig {

    /// Create a new Forge version from its raw name. 
    /// 
    /// This constructor will parse the version to internally change the installer 
    /// behavior.
    fn new_forge(name: &str) -> Option<Self> {

        let (game_version, loader_version) = name.split_once('-')?;
        let (loader_version, _) = loader_version.split_once('-').unwrap_or((loader_version, ""));
        let loader_version = parse_generic_version::<4, 2>(loader_version);

        Some(Self {
            default_prefix: "forge",
            gav: Gav::new("net.minecraftforge", "forge", name, None, None),
            repo_url: "https://maven.minecraftforge.net",
            game_version: if game_version == "1.7.10_pre4" {
                "1.7.10-pre4".to_string()  // The only pre-release supported.
            } else {
                game_version.to_string()
            },
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
        })

    }

    /// Create a new NeoForge version from its name.
    /// 
    /// This constructor will parse the version to internally change the installer 
    /// behavior.
    fn new_neoforge(name: &str) -> Option<Self> {

        let gav;
        let game_version;

        if name == "47.1.82" || name.starts_with("1.20.1-") {
            gav = Gav::new("net.neoforged", "forge", name, None, None);
            game_version = "1.20.1".to_string();
        } else {
            gav = Gav::new("net.neoforged", "neoforge", name, None, None);
            game_version = match parse_generic_version::<2, 2>(name)? {
                [major, 0] => format!("1.{major}"),
                [major, minor] => format!("1.{major}.{minor}"),
            };
        };

        Some(Self {
            default_prefix: "neoforge",
            gav,
            repo_url: "https://maven.neoforged.net/releases",
            game_version,
            check_libraries: true,
            extra_in_mcp: true,
            legacy_install_profile: false,
            check_processor_outputs: true,
        })

    }

}

/// Try installing the mod loader.
fn try_install(
    handler: &mut dyn Handler,
    mojang: &mut mojang::Installer,
    config: &InstallConfig,
    root_version: &str,
    side: serde::InstallSide,
    reason: InstallReason,
) -> Result<()> {

    let tmp_dir = env::temp_dir().joined(root_version);
    handler.on_event(Event::Installing { tmp_dir: &tmp_dir, reason });

    // The first thing we do is fetching the installer, so it ends early if there is 
    // simply no installer for this version!
    handler.on_event(Event::FetchInstaller { version: config.gav.version() });

    let installer_gav = config.gav.with_classifier(Some("installer"));
    let installer_url = format!("{}/{}", config.repo_url, installer_gav.url());
    
    // Download and check result in case installer is just not found.
    let entry = download::single(installer_url, tmp_dir.join("installer.jar"))
        .set_keep_open()
        .download((&mut *handler).into_download());

    let mut entry = match entry {
        Ok(entry) => entry,
        Err(e) => {
            if let EntryErrorKind::InvalidStatus(404) = e.kind() {
                return Err(Error::InstallerNotFound { 
                    version: config.gav.version().to_string(),
                });
            } else {
                return Err(e.into());
            }
        }
    };

    let installer_reader = BufReader::new(entry.take_handle().unwrap());
    let installer_file = entry.file();
    let mut installer_zip = ZipArchive::new(installer_reader)
        .map_err(|e| base::Error::new_zip_file(e, installer_file))?;

    handler.on_event(Event::FetchedInstaller { version: config.gav.version() });
    
    // We need to ensure that the underlying game version is fully installed. Here we
    // just forward the handler as-is, and we check for version not found to warn
    // about an non-existing game version. We keep the installed, or found, JVM exec
    // for later execution of installer processors. Note that the JVM exec path should
    // be already canonicalized.
    handler.on_event(Event::InstallingGame);
    mojang.set_version(config.game_version.clone());
    let jvm_file = match mojang.install((&mut *handler).into_mojang()) {
        Err(e) => return Err(Error::Mojang(e)),
        Ok(game) => game.jvm_file,
    };

    const PROFILE_ENTRY: &str = "install_profile.json";
    let profile = match installer_zip.by_name(PROFILE_ENTRY) {
        Ok(reader) => {
            
            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            let res = if config.legacy_install_profile {
                serde_path_to_error::deserialize::<_, serde::LegacyInstallProfile>(&mut deserializer)
                    .map(InstallProfileKind::Legacy)
            } else {
                serde_path_to_error::deserialize::<_, serde::ModernInstallProfile>(&mut deserializer)
                    .map(InstallProfileKind::Modern)
            };

            res.map_err(|e| base::Error::new_json(e, format!("entry: {}, from: {}", 
                PROFILE_ENTRY, 
                installer_file.display())))?

        }
        Err(_) => return Err(Error::InstallerProfileNotFound {  })
    };

    // The installer directly installs libraries to these directories.
    // We canonicalize the libs path here, this avoids doing it after each join.
    let libraries_dir = base::canonicalize_file(mojang.base().libraries_dir())?;
    let game_version_dir = mojang.base().versions_dir().join(&config.game_version);
    let game_client_file = game_version_dir.join_with_extension(&config.game_version, "jar");
    let root_version_dir = mojang.base().versions_dir().join(&root_version);
    let metadata_file = root_version_dir.join_with_extension(&root_version, "json");
    let mut metadata;

    match profile {
        InstallProfileKind::Modern(profile) => {
            
            if profile.minecraft != config.game_version {
                return Err(Error::InstallerProfileIncoherent {  });
            }

            // Immediately try, and keep the version metadata, this avoid launching this
            // error at the end after all the processing happened.
            let metadata_entry = profile.json.strip_prefix('/').unwrap_or(&profile.json);
            metadata = match installer_zip.by_name(metadata_entry) {
                Ok(reader) => {
                    let mut deserializer = serde_json::Deserializer::from_reader(reader);
                    serde_path_to_error::deserialize::<_, Box<base::serde::VersionMetadata>>(&mut deserializer)
                        .map_err(|e| base::Error::new_json(e, format!("entry: {}, from: {}",
                            metadata_entry,
                            installer_file.display())))?
                }
                Err(_) => return Err(Error::InstallerVersionMetadataNotFound {  })
            };

            handler.on_event(Event::FetchInstallerLibraries);
            
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
                    // FIXME: Insecure joining!
                    libraries_dir.join(lib_path)
                } else {
                    lib.name.file(&libraries_dir)
                };

                libraries.insert(&lib.name, lib_file.clone());
                
                if !lib_dl.download.url.is_empty() {
                    batch.push(lib_dl.download.url.to_string(), lib_file)
                        .set_expected_size(lib_dl.download.size)
                        .set_expected_sha1(lib_dl.download.sha1.as_deref().copied());
                } else {
                    extract_installer_maven_artifact(installer_file, &mut installer_zip, &lib.name, &lib_file)?;
                }

            }

            // Download all libraries just before running post processors.
            if !batch.is_empty() {
                batch.download((&mut *handler).into_download())
                    .map_err(|e| base::Error::new_reqwest(e, "download forge libraries"))?
                    .into_result()?;
            }

            handler.on_event(Event::FetchedInstallerLibraries);

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
                        // FIXME: Insecure joining.
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
            data.insert("MINECRAFT_VERSION".to_string(), InstallDataTypedEntry::Literal(config.game_version.to_string()));
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

                handler.on_event(Event::RunInstallerProcessor { name: &processor.jar, task });

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
                    .map_err(|e| base::Error::new_io(e, format!("spawn: {}", jvm_file.display())))?;

                if !output.status.success() {
                    return Err(Error::InstallerProcessorFailed {
                        name: processor.jar.clone(),
                        output: Box::new(output),
                    });
                }

                // If process SHA-1 check is enabled...
                if config.check_processor_outputs {
                    for (file, sha1) in &processor.outputs {
                        let Some(file) = format_processor_arg(&file, &libraries_dir, &data) else { continue };
                        let Some(sha1) = format_processor_arg(&sha1, &libraries_dir, &data) else { continue };
                        let Some(sha1) = crate::serde::parse_hex_bytes::<20>(&sha1) else { continue };
                        let file = Path::new(&file);
                        if !base::check_file(file, None, Some(&sha1))? {
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
                metadata.inherits_from = Some(config.game_version.clone());
            }

            // Extract the universal JAR file of the mod loader.
            let jar_file = profile.install.path.file(libraries_dir);
            let jar_entry = &profile.install.file_path[..];
            extract_installer_file(installer_file, &mut installer_zip, &jar_entry, &jar_file)?;

        }
    }

    metadata.id = root_version.to_string();
    base::write_version_metadata(&metadata_file, &metadata)?;

    handler.on_event(Event::Installed);

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
    let src_entry = format!("maven/{}", src_name.url());
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
        .map_err(|e| base::Error::new_io_file(e, parent_dir))?;

    let mut writer = File::create(dst_file)
        .map_err(|e| base::Error::new_io_file(e, dst_file))
        .map(BufWriter::new)?;

    io::copy(&mut reader, &mut writer)
        .map_err(|e| base::Error::new_io(e, format!("extract: {}, from: {}", 
            src_entry, 
            installer_file.display())))?;

    Ok(())

}

/// From a JAR file path, open it and try to find the main class path from the manifest.
fn find_jar_main_class(jar_file: &Path) -> Result<Option<String>> {

    let jar_reader = File::open(jar_file)
        .map_err(|e| base::Error::new_io_file(e, jar_file))
        .map(BufReader::new)?;

    let mut jar_zip = ZipArchive::new(jar_reader)
        .map_err(|e| base::Error::new_zip_file(e, jar_file))?;

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

#[cfg(test)]
mod test {

    use super::*;
    
    #[test]
    fn parse_version() {

        assert_eq!(parse_generic_version::<4, 2>("1"), None);
        assert_eq!(parse_generic_version::<4, 2>("1.2"), Some([1, 2, 0, 0]));
        assert_eq!(parse_generic_version::<4, 2>("1.2.3"), Some([1, 2, 3, 0]));
        assert_eq!(parse_generic_version::<4, 2>("1.2.3.4"), Some([1, 2, 3, 4]));
        assert_eq!(parse_generic_version::<4, 2>("1.2.3.4.5"), Some([1, 2, 3, 4]));

        assert_eq!(parse_game_version("1"), None);
        assert_eq!(parse_game_version("1.2"), Some([2, 0]));
        assert_eq!(parse_game_version("1.2-pre3"), Some([2, 0]));
        assert_eq!(parse_game_version("1.2.5"), Some([2, 5]));
        assert_eq!(parse_game_version("1.2.5-pre3"), Some([2, 5]));

    }

}
