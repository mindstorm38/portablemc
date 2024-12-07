//! Extension to the Mojang installer to support fetching and installation of 
//! Forge and NeoForge mod loader versions.

pub mod serde;

use std::path::{Path, PathBuf};
use std::fmt;

use crate::mojang::{self, Handler as _, RootVersion};
use crate::maven::MavenMetadata;
use crate::{download, standard};

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

    /// By default, this Forge and NeoForge installer targets the latest stable version.
    pub fn game_version(&mut self, version: impl Into<GameVersion>) {
        self.inner.game_version = version.into();
    }

    /// By default, this Forge installer targets the latest stable loader version for the
    /// currently configured game version, use this function to override the loader 
    /// version to use.
    pub fn loader_version(&mut self, version: impl Into<LoaderVersion>) {
        self.inner.loader_version = version.into();
    }

    /// Install the currently configured Forge/NeoForge loader with the given handler.
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {

        let Self {
            ref mut mojang,
            ref inner,
        } = self;

        // Start by getting the game version.
        let game_version_id = match inner.game_version {
            GameVersion::Id(ref id) => id.clone(),
            GameVersion::Release => {
                mojang::request_manifest(handler.as_download_dyn())?
                    .latest.get(&standard::serde::VersionType::Release)
                    .cloned()
                    .ok_or_else(|| Error::AliasGameVersionNotFound { 
                        api: inner.api, 
                        game_version: inner.game_version.clone(),
                    })?
            }
        };
                        
        let Some((major, minor)) = parse_game_version_major_minor(&game_version_id) else {
            return Err(Error::GameVersionNotFound { 
                api: inner.api, 
                game_version_id,
            });
        };

        eprintln!("game_version_id: {game_version_id}, major: {major}, minor: {minor}");

        let artifact = (inner.api.maven_artifact)(&game_version_id, major, minor);

        // Full loader version id, containing the game version.
        let loader_version_id = match inner.loader_version {
            LoaderVersion::Id(ref id) => {
                (inner.api.build_maven_version)(&game_version_id, major, minor, &id)
            }
            LoaderVersion::Stable |
            LoaderVersion::Unstable => {

                let metadata_url = format!("{}/{artifact}/maven-metadata.xml", inner.api.maven_group_base_url);
                let metadata = request_maven_metadata(&metadata_url)?;

                let stable = matches!(inner.loader_version, LoaderVersion::Stable);
                let id;

                let prefix = (inner.api.build_maven_version_prefix)(&game_version_id, major, minor);
                let find_version = |id: &&str| 
                    id.starts_with(&prefix) && 
                    (!stable || (inner.api.is_maven_version_stable)(&game_version_id, major, minor, id));

                if inner.api.maven_manifest_reverse_order {
                    id = metadata.versions().rev().find(find_version).map(str::to_string);
                } else {
                    id = metadata.versions().find(find_version).map(str::to_string);
                }
                
                let Some(id) = id else {
                    return Err(Error::AliasLoaderVersionNotFound { 
                        api: inner.api, 
                        game_version_id, 
                        loader_version: inner.loader_version.clone(),
                    });
                };

                id

            }
        };

        eprintln!("loader_version_id: {loader_version_id}");

        // We need to ensure that the underlying game version is fully installed. Here we
        // just forward the handler as-is, and we check for version not found to warn
        // about an non-existing game version.
        mojang.root_version(RootVersion::Id(game_version_id.clone()));
        handler.handle_forge_event(Event::GameVersionInstalling {  });
        let jvm_file = match mojang.install(handler.as_mojang_dyn()) {
            Err(mojang::Error::Standard(standard::Error::VersionNotFound { id })) if id == game_version_id => {
                return Err(Error::GameVersionNotFound { 
                    api: inner.api, 
                    game_version_id,
                });
            }
            Err(e) => return Err(Error::Mojang(e)),
            Ok(game) => game.jvm_file,
        };
        handler.handle_forge_event(Event::GameVersionInstalled {  });

        // Now that the game version is installed, we can use our internal handler to
        // handle non-existing version, if the 
        let prefix = inner.api.default_prefix;
        let root_version_id = format!("{prefix}-{loader_version_id}");
        mojang.root_version(RootVersion::Id(root_version_id.clone()));

        // Scoping the temporary internal handler.
        let game = {

            let mut handler = InternalHandler {
                inner: &mut handler,
                installer: &inner,
                error: Ok(()),
                jvm_file: &jvm_file,
                artifact,
                root_version_id: &root_version_id,
                game_version_id: &game_version_id,
                loader_version_id: &loader_version_id,
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
    /// The game version is going to be fully installed prior to launching the forge 
    /// loader and potentially installing it. The handle will receive all events from
    /// a normal installation.
    GameVersionInstalling {},
    /// The game version has been installed, the forge loader will now be installed.
    GameVersionInstalled {},
    VersionFetching {
        api: &'static Api,
        game_version_id: &'a str,
        loader_version_id: &'a str,
    },
    VersionFetched {
        api: &'static Api,
        game_version_id: &'a str,
        loader_version_id: &'a str,
    },
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
        api: &'static Api,
        game_version: GameVersion,
    },
    /// An alias loader version, `Stable` or `Unstable` has not been found because the 
    /// alias is missing the for API's versions.
    #[error("alias loader version not found: {game_version_id}/{loader_version:?}")]
    AliasLoaderVersionNotFound {
        api: &'static Api,
        game_version_id: String,
        loader_version: LoaderVersion,
    },
    /// The given game version as requested to launch Forge with is not supported by the
    /// selected API.
    #[error("game version not found: {game_version_id}")]
    GameVersionNotFound {
        api: &'static Api,
        game_version_id: String,
    },
    /// The 'maven-metadata.xml' file requested only is 
    #[error("maven metadata is malformed")]
    MavenMetadataMalformed {  },
    /// The 'install_profile.json' installer file was not found.
    #[error("install profile not found")]
    InstallProfileNotFound {  },
    /// The 'version.json' installer file was not found, it contains the version metadata
    /// to be installed.
    #[error("install version metadata not found")]
    InstallVersionMetadataNotFound {  },
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
    /// The base URL for the maven group directory, without leading slash.
    maven_group_base_url: &'static str,
    /// Get the maven artifact, from 
    maven_artifact: fn(game_version_id: &str, game_major: u8, game_minor: u8) -> &'static str,
    build_maven_version: fn(game_version_id: &str, game_major: u8, game_minor: u8, loader_version_id: &str) -> String,
    build_maven_version_prefix: fn(game_version_id: &str, game_major: u8, game_minor: u8) -> String,
    is_maven_version_stable: fn(game_version_id: &str, game_major: u8, game_minor: u8, loader_version_id: &str) -> bool,

}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Api").finish()
    }
}

/// The original forge API.
pub static FORGE_API: Api = Api {
    default_prefix: "forge",
    maven_manifest_reverse_order: false,
    maven_group_base_url: "https://maven.minecraftforge.net/net/minecraftforge",
    maven_artifact: |_game_version_id: &str, _game_major: u8, _game_minor: u8| {
        "forge"
    },
    build_maven_version: |game_version_id: &str, _game_major: u8, _game_minor: u8, loader_version_id: &str| {
        format!("{game_version_id}-{loader_version_id}")
    },
    build_maven_version_prefix: |game_version_id: &str, _game_major: u8, _game_minor: u8| {
        format!("{game_version_id}-")
    },
    is_maven_version_stable: |_game_version_id: &str, _game_major: u8, _game_minor: u8, _loader_version_id: &str| {
        true  // All versions are stable
    },
};

/// The forked forge API, called NeoForge. The special case of 1.20.1 is properly handled
/// under the legacy artifact ID 'forge', subsequent versions are handled as 'neoforge'
/// with the new loader versioning.
pub static NEO_FORGE_API: Api = Api {
    default_prefix: "neoforge",
    maven_manifest_reverse_order: true,
    maven_group_base_url: "https://maven.neoforged.net/releases/net/neoforged",
    maven_artifact: |_game_version_id: &str, game_major: u8, game_minor: u8| {
        if game_major == 20 && game_minor == 1 {
            "forge"
        } else {
            "neoforge"
        }
    },
    build_maven_version: |_game_version_id: &str, game_major: u8, game_minor: u8, loader_version_id: &str| {
        if game_major == 20 && game_minor == 1 {
            format!("1.20.1-{loader_version_id}")
        } else {
            format!("{game_major}.{game_minor}.{loader_version_id}")
        }
    },
    build_maven_version_prefix: |_game_version_id: &str, game_major: u8, game_minor: u8| {
        if game_major == 20 && game_minor == 1 {
            format!("1.20.1-")
        } else {
            format!("{game_major}.{game_minor}.")
        }
    },
    is_maven_version_stable: |_game_version_id: &str, game_major: u8, game_minor: u8, loader_version_id: &str| {
        if game_major == 20 && game_minor == 1 {
            true
        } else {
            !loader_version_id.ends_with("-beta")
        }
    },
};

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
    /// Use the specific version. The exact meaning of this depends on the actual API
    /// being used:
    /// 
    /// - With [`Api::Forge`], the id represent the full loader version that is appended
    ///   to the game version, like in `1.21-51.0.33`, the loader version id is `51.0.33`.
    ///   Some rare loader versions are strange and are also suffixed by some string
    ///   related to the game version, like `1.11-13.19.0.2129-1.11.x`, because it don't
    ///   make sense to specify a loader version `13.19.0.2129-1.11.x`, you can simply 
    ///   specify `13.19.0.2129` and these case will be handled silently, this suffix
    ///   won't be included in the root version's id.
    /// 
    /// - With [`Api::NeoForge`], the id represent the last "patch" number of the loader.
    ///   NeoForge versioning consists in the Minecraft major and minor version (ignoring
    ///   the first '1.'), and the loader patch. For example NeoForge loader `20.4.181`,
    ///   the game version id is `1.20.4` and loader version id is `181`.
    Id(String),
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
    jvm_file: &'a Path,
    artifact: &'a str,
    root_version_id: &'a str,
    game_version_id: &'a str,
    loader_version_id: &'a str,
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
            standard::Event::VersionLoading { 
                id, 
                file,
            } if id == self.root_version_id => {
                // TODO: Check that every important file is installed.
            }
            standard::Event::VersionNotFound { 
                id, 
                file, 
                error: _, 
                retry,
            } if id == self.root_version_id => {

                self.inner.handle_forge_event(Event::VersionFetching {
                    api: self.installer.api,
                    game_version_id: self.game_version_id,
                    loader_version_id: self.loader_version_id,
                });

                let installer_url = format!("{base}/{art}/{art}-{version}-installer.jar", 
                    base = self.installer.api.maven_group_base_url,
                    art = self.artifact,
                    version = self.loader_version_id);

                let dir = file.parent().unwrap();
                let mut entry = download::single(installer_url, dir.join("installer.jar"))
                    .set_keep_open()
                    .download(self.inner.as_download_dyn())??;

                let installer_file = entry.take_handle().unwrap();
                let mut installer_zip = ZipArchive::new(installer_file)
                    .map_err(|e| standard::Error::new_zip_file(e, entry.file()))?;

                // The install profiles comes in multiples forms:
                // >= 1.12.2-14.23.5.2851
                //      There are two files, 'install_profile.json' which 
                //      contains processors and shared data, and `version.json`
                //      which is the raw version meta to be fetched.
                // <= 1.12.2-14.23.5.2847
                //      There is only an 'install_profile.json' with the version
                //      meta stored in 'versionInfo' object. Each library have
                //      two keys 'serverreq' and 'clientreq' that should be
                //      removed when the profile is returned.
                let profile = match installer_zip.by_name("install_profile.json") {
                    Ok(reader) => {
                        let mut deserializer = serde_json::Deserializer::from_reader(reader);
                        serde_path_to_error::deserialize::<_, serde::InstallProfile>(&mut deserializer)?
                    }
                    Err(_) => return Err(Error::InstallProfileNotFound {  })
                };

                // This is the version metadata;
                let mut metadata;

                match profile {
                    serde::InstallProfile::Modern(profile) => {

                        let metadata_entry = profile.json.strip_prefix('/').unwrap_or(&profile.json);
                        metadata = match installer_zip.by_name(metadata_entry) {
                            Ok(reader) => {
                                let mut deserializer = serde_json::Deserializer::from_reader(reader);
                                serde_path_to_error::deserialize::<_, standard::serde::VersionMetadata>(&mut deserializer)?
                            }
                            Err(_) => return Err(Error::InstallVersionMetadataNotFound {  })
                        };

                    }
                    serde::InstallProfile::Legacy(profile) => {
                        
                        // FIXME: Large copy of bytes here...
                        metadata = profile.version_info;

                    }
                }

                *retry = true;

                self.inner.handle_forge_event(Event::VersionFetched {
                    api: self.installer.api,
                    game_version_id: self.game_version_id,
                    loader_version_id: self.loader_version_id,
                });

                // Note that we never forward the event in any case...

            }
            _ => self.inner.handle_standard_event(event),
        }

        Ok(())

    }
    
}

/// Internal function that parses the game version major and minor version numbers, if
/// the version starts with "1.", returning 0 for minor version is not present.
fn parse_game_version_major_minor(id: &str) -> Option<(u8, u8)> {
    
    let mut it = id.split('.');
    
    if it.next()? != "1" {
        return None;
    }

    let major = it.next()?.parse::<u8>().ok()?;
    let minor = match it.next() {
        Some(minor) => minor.parse::<u8>().ok()?,
        None => 0,
    };

    Some((major, minor))

}

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
