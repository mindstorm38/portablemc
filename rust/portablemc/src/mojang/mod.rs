//! Extension to the standard installer with verification and installation of missing
//! Mojang versions, it also provides support for common arguments and fixes on legacy
//! versions.

pub mod serde;

use std::collections::{HashMap, HashSet};
use std::io::{Write as _, BufReader};
use std::fs::{self, File};
use std::path::PathBuf;
use std::env;

use crate::standard::{self, LIBRARIES_URL, Handler as _, Library, check_file, replace_strings_args};
use crate::download::{self, Entry, EntrySource};
use crate::gav::Gav;
use crate::msa;

pub use standard::Game;
use uuid::Uuid;


/// Static URL to the version manifest provided by Mojang.
pub(crate) const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// An installer for supporting Mojang-provided versions. It provides support for various
/// standard arguments such as demo mode, window resolution and quick play, it also 
/// provides various fixes for known issues of old versions.
/// 
/// Notes about various versions:
/// - 1.19.3 metadata adds no parameter to specify extract directory for LWJGL (version
///   3.3.1-build-7), therefore natives are extracted to '/tmp/lwjgl<username>/<version>'.
#[derive(Debug, Clone)]
pub struct Installer {
    /// The underlying standard installer logic.
    standard: standard::Installer,
    /// Inner installer data, put in a sub struct to fix borrow issue.
    inner: InstallerInner,
}

/// Internal installer data.
#[derive(Debug, Clone)]
struct InstallerInner {
    root: Root,
    fetch: bool,
    fetch_exclude: Vec<String>,
    demo: bool,
    quick_play: Option<QuickPlay>,
    resolution: Option<(u16, u16)>,
    disable_multiplayer: bool,
    disable_chat: bool,
    auth_type: String,  // Empty to trigger default auth.
    auth_uuid: Uuid,
    auth_username: String,
    auth_token: String,
    auth_xuid: String,
    auth_client_id: String,
    fix_legacy_quick_play: bool,
    fix_legacy_proxy: bool,
    fix_legacy_merge_sort: bool,
    fix_legacy_resolution: bool,
    fix_broken_authlib: bool,
    fix_lwjgl: Option<String>,
}

impl Installer {

    /// Create a new installer with default configuration, using defaults directories and
    /// the given root version to load and then install. This Mojang installer has all 
    /// fixes enabled except LWJGL and missing version fetching is enabled.
    pub fn new(main_dir: impl Into<PathBuf>) -> Self {
        Self {
            standard: standard::Installer::new(String::new(), main_dir),
            inner: InstallerInner {
                root: Root::Release,
                fetch: true,
                fetch_exclude: Vec::new(),
                demo: false,
                quick_play: None,
                resolution: None,
                disable_multiplayer: false,
                disable_chat: false,
                auth_type: String::new(),
                auth_uuid: Uuid::nil(),
                auth_username: String::new(),
                auth_token: String::new(),
                auth_xuid: String::new(),
                auth_client_id: String::new(),
                fix_legacy_quick_play: true,
                fix_legacy_proxy: true,
                fix_legacy_merge_sort: true,
                fix_legacy_resolution: true,
                fix_broken_authlib: true,
                fix_lwjgl: None,
            }
        }
    }

    /// Same as [`Self::new`] but using the default main directory in your system,
    /// returning none if there is no default main directory on your system.
    pub fn new_with_default() -> Option<Self> {
        Some(Self::new(standard::default_main_dir()?))
    }

    /// Execute some callback to alter the standard installer.
    #[inline]
    pub fn with_standard<F>(&mut self, func: F) -> &mut Self
    where
        F: FnOnce(&mut standard::Installer) -> &mut standard::Installer,
    {
        func(&mut self.standard);
        self
    }

    /// By default, this Mojang installer targets the latest release version, use this
    /// function to change the version to install.
    #[inline]
    pub fn root(&mut self, id: impl Into<Root>) -> &mut Self {
        self.inner.root = id.into();
        self
    }

    /// Set to true if this installer should use the online versions manifest to check
    /// already installed version and fetch them if outdated or not installed. This will
    /// do that for every version in the hierarchy of loaded version, but versions can
    /// be excluded from this fetching using [`Self::fetch_exclude`].
    /// 
    /// This is enabled by default.
    #[inline]
    pub fn fetch(&mut self, fetch: bool) -> &mut Self {
        self.inner.fetch = fetch;
        self
    }

    /// Exclude the given version id from versions that should be fetched, this is 
    /// not used if fetching is fully disabled.
    #[inline]
    pub fn fetch_exclude(&mut self, id: impl Into<String>) -> &mut Self {
        self.inner.fetch_exclude.push(id.into());
        self
    }

    /// Set to true to enable the demo mode of the game.
    #[inline]
    pub fn demo(&mut self, demo: bool) -> &mut Self {
        self.inner.demo = demo;
        self
    }

    /// Enables Quick Play when launching the game, from 1.20 (23w14a).
    #[inline]
    pub fn quick_play(&mut self, quick_play: QuickPlay) -> &mut Self {
        self.inner.quick_play = Some(quick_play);
        self
    }

    #[inline]
    pub fn no_quick_play(&mut self) -> &mut Self {
        self.inner.quick_play = None;
        self
    }

    /// Set an initial resolution for the game's window.
    #[inline]
    pub fn resolution(&mut self, width: u16, height: u16) -> &mut Self {
        self.inner.resolution = Some((width, height));
        self
    }

    #[inline]
    pub fn no_resolution(&mut self) -> &mut Self {
        self.inner.resolution = None;
        self
    }

    /// Disable or not the multiplayer when launching the game.
    #[inline]
    pub fn disable_multiplayer(&mut self, disable_multiplayer: bool) -> &mut Self {
        self.inner.disable_multiplayer = disable_multiplayer;
        self
    }

    /// Disable or not the chat when launching the game.
    #[inline]
    pub fn disable_chat(&mut self, disable_chat: bool) -> &mut Self {
        self.inner.disable_chat = disable_chat;
        self
    }

    /// Manually set the authentication UUID, not touching any other parameter.
    pub fn auth_raw_uuid(&mut self, uuid: Uuid) -> &mut Self {
        self.inner.auth_uuid = uuid;  // TODO: add missing other methods
        self
    }

    /// Internal function to reset to zero-length all online-related auth variables.
    #[inline(always)]
    fn reset_auth_online(&mut self) -> &mut Self {
        self.inner.auth_type = String::new();
        self.inner.auth_token = String::new();
        self.inner.auth_xuid = String::new();
        self.inner.auth_client_id = String::new();
        self
    }

    /// Use offline session with the given UUID and username, note that the username will
    /// be truncated 16 bytes at most (this function will panic if the truncation is not 
    /// on a valid UTF-8 character boundary).
    pub fn auth_offline(&mut self, uuid: Uuid, username: impl Into<String>) -> &mut Self {
        self.inner.auth_uuid = uuid;
        self.inner.auth_username = username.into();
        self.inner.auth_username.truncate(16);
        self.reset_auth_online()
    }

    /// Use offline session with the given UUID, the username is derived from the first
    /// 8 characters of the rendered UUID.
    pub fn auth_offline_uuid(&mut self, uuid: Uuid) -> &mut Self {
        self.inner.auth_username = uuid.to_string();
        self.inner.auth_username.truncate(8);
        self.reset_auth_online()
    }

    /// Use offline session with a deterministic UUID, derived from this machine's 
    /// hostname, the username is then derived from the UUID following the same logic
    /// as for [`Self::auth_offline_uuid`].
    /// 
    /// **This is the default UUID/username used if no auth is specified, so you don't
    /// need to call this function, except if you want to override previous auth.**
    pub fn auth_offline_hostname(&mut self) -> &mut Self {
        self.auth_offline_uuid(Uuid::new_v5(&standard::UUID_NAMESPACE, gethostname::gethostname().as_encoded_bytes()))
    }

    /// Use offline session with the given username (initially truncated to 16 chars), 
    /// the UUID is then derived from this username using a PMC-specific derivation of 
    /// the username and the PMC namespace with SHA-1 (UUID v5).
    /// 
    /// Note that the produced UUID will not be used when playing on multiplayer servers
    /// (the server must also be in offline-mode), in this case the server gives you an
    /// arbitrary UUID that is not the one your game has been launched with. Most servers
    /// uses the UUID derivation embedded in Mojang's authlib, deriving the UUID from the
    /// username, if you want the UUID to be coherent with this derivation, you can use
    /// [`Self::auth_offline_username_authlib`] instead.
    pub fn auth_offline_username(&mut self, username: impl Into<String>) -> &mut Self {
        self.inner.auth_username = username.into();
        self.inner.auth_username.truncate(16);
        self.inner.auth_uuid = Uuid::new_v5(&standard::UUID_NAMESPACE, self.inner.auth_username.as_bytes());
        self.reset_auth_online()
    }

    /// Use offline session with the given username (initially truncated to 16 chars), 
    /// the UUID is then derived from this username using the same derivation used by 
    /// most Mojang clients (versions to be defined), this produces a MD5 (v3) UUID 
    /// with `OfflinePlayer:{username}` as the hashed string.
    /// 
    /// The advantage of this method is to produce the same UUID as the one that will
    /// be produced by Mojang's authlib when connecting to an offline-mode multiplayer
    /// server.
    pub fn auth_offline_username_authlib(&mut self, username: impl Into<String>) -> &mut Self {
        
        self.inner.auth_username = username.into();
        self.inner.auth_username.truncate(16);

        let mut context = md5::Context::new();
        context.write_fmt(format_args!("OfflinePlayer:{}", self.inner.auth_username)).unwrap();
        
        self.inner.auth_uuid = uuid::Builder::from_bytes(context.compute().0)
            .with_variant(uuid::Variant::RFC4122)
            .with_version(uuid::Version::Md5)
            .into_uuid();

        self.reset_auth_online()

    }

    /// Use online authentication with the given Microsoft Account.
    pub fn auth_msa(&mut self, account: &msa::Account) -> &mut Self {
        self.inner.auth_uuid = account.uuid().clone();
        self.inner.auth_username = account.username().to_string();
        self.inner.auth_token = account.access_token().to_string();
        self.inner.auth_xuid = account.xuid().to_string();
        self.inner.auth_type = "msa".to_string();
        self
    }

    /// When starting versions older than 1.20 (23w14a) where Quick Play was not supported
    /// by the client, this fix tries to use legacy arguments instead, such as --server
    /// and --port, this is enabled by default.
    #[inline]
    pub fn fix_legacy_quick_play(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_quick_play = fix;
        self
    }

    /// When starting older alpha, beta and release up to 1.5, this allows legacy online
    /// resources such as skins to be properly requested. The implementation is currently 
    /// using `betacraft.uk` proxies.
    #[inline]
    pub fn fix_legacy_proxy(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_proxy = fix;
        self
    }

    /// When starting older alpha and beta versions, this adds a JVM argument to use the
    /// legacy merge sort `java.util.Arrays.useLegacyMergeSort=true`, this is required on
    /// some old versions to avoid crashes.
    #[inline]
    pub fn fix_legacy_merge_sort(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_merge_sort = fix;
        self
    }

    /// When starting older versions that don't support modern resolution arguments, this
    /// fix will add arguments to force resolution of the initial window.
    #[inline]
    pub fn fix_legacy_resolution(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_resolution = fix;
        self
    }

    /// Versions 1.16.4 and 1.16.5 uses authlib:2.1.28 which cause multiplayer button
    /// (and probably in-game chat) to be disabled, this can be fixed by switching to
    /// version 2.2.30 of authlib.
    #[inline]
    pub fn fix_broken_authlib(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_broken_authlib = fix;
        self
    }

    /// Changing the version of LWJGL, this support versions greater or equal to 3.2.3,
    /// and also provides ARM support when the LWJGL version supports it. It's not 
    /// guaranteed to work with every version of Minecraft, and downgrading LWJGL version
    /// is not recommended.
    /// 
    /// If the given version is less than 3.2.3 this will do nothing.
    #[inline]
    pub fn fix_lwjgl(&mut self, lwjgl_version: impl Into<String>) -> &mut Self {
        self.inner.fix_lwjgl = Some(lwjgl_version.into());
        self
    }
    
    /// Don't fix LWJGL version (see [`Self::fix_lwjgl`]).
    #[inline]
    pub fn no_fix_lwjgl(&mut self) -> &mut Self {
        self.inner.fix_lwjgl = None;
        self
    }

    /// Install the given Mojang version from its identifier. This also supports alias
    /// identifiers such as "release" and "snapshot" that will be resolved, note that
    /// these identifiers are just those presents in the "latest" mapping of the
    /// Mojang versions manifest. 
    /// 
    /// If the given version is not found in the manifest then it's silently ignored and
    /// the version metadata must already exists.
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {
        
        // Apply default offline auth, derived from hostname.
        if self.inner.auth_username.is_empty() {
            self.auth_offline_hostname();
        }

        let Self {
            ref mut standard,
            ref inner,
        } = self;

        // Cached manifest, will only be used if fetch is enabled.
        let mut manifest = None::<serde::MojangManifest>;

        // Resolve aliases such as "release" or "snapshot" if fetch is enabled.
        let alias = match self.inner.root {
            Root::Release => Some(standard::serde::VersionType::Release),
            Root::Snapshot => Some(standard::serde::VersionType::Snapshot),
            _ => None,
        };

        // If we need an alias then we need to load the manifest.
        let id;
        if let Some(alias) = alias {
            if inner.fetch {
                let new_manifest = request_manifest(handler.as_download_dyn())?;
                id = new_manifest.latest.get(&alias).cloned();
                manifest = Some(new_manifest);
            } else {
                id = None;
            }
        } else {
            id = match self.inner.root {
                Root::Id(ref new_id) => Some(new_id.clone()),
                _ => unreachable!(),
            };
        }

        let Some(id) = id else {
            return Err(Error::AliasVersionNotFound { root: self.inner.root.clone() });
        };

        standard.root(id);
        
        // Let the handler find the "leaf" version.
        let mut leaf_id = String::new();

        // Scoping the temporary internal handler.
        let mut game = {

            let mut handler = InternalHandler {
                inner: &mut handler,
                installer: &inner,
                manifest: &mut manifest,
                downloads: HashMap::new(),
                leaf_id: &mut leaf_id,
                error: Ok(()),
            };
    
            // Same as above, we are giving a &mut dyn ref to avoid huge monomorphization.
            let res = standard.install(handler.as_standard_dyn());
            handler.error?;
            res?

        };

        // Apply auth parameters.
        replace_strings_args(&mut game.game_args, |arg| {
            match arg {
                "auth_player_name" => Some(inner.auth_username.clone()),
                "auth_uuid" => Some(inner.auth_uuid.as_simple().to_string()),
                "auth_access_token" => Some(inner.auth_token.clone()),
                "auth_xuid" => Some(inner.auth_xuid.clone()),
                // Legacy parameter
                "auth_session" if !inner.auth_token.is_empty() => 
                    Some(format!("token:{}:{}", inner.auth_token, inner.auth_uuid.as_simple())),
                "auth_session" => Some(String::new()),
                "user_type" => Some(inner.auth_type.clone()),
                "clientid" => Some(inner.auth_client_id.clone()),
                _ => None
            }
        });

        // If Quick Play is enabled, we know that the feature has been enabled by the
        // handler, and if the feature is actually present (1.20 and after), if not
        // present we can try to use legacy arguments for supported quick play types.
        if let Some(quick_play) = &inner.quick_play {

            let quick_play_arg = match quick_play {
                QuickPlay::Path { .. } => "quickPlayPath",
                QuickPlay::Singleplayer { .. } => "quickPlaySingleplayer",
                QuickPlay::Multiplayer { .. } => "quickPlayMultiplayer",
                QuickPlay::Realms { .. } => "quickPlayRealms",
            };

            let mut quick_play_supported = false;

            replace_strings_args(&mut game.game_args, |arg| {
                if arg == quick_play_arg {
                    quick_play_supported = true;
                    Some(match quick_play {
                        QuickPlay::Path { path } => path.display().to_string(),
                        QuickPlay::Singleplayer { name } => name.clone(),
                        QuickPlay::Multiplayer { host, port } => format!("{host}:{port}"),
                        QuickPlay::Realms { id } => id.clone(),
                    })
                } else {
                    None
                }
            });

            if !quick_play_supported && inner.fix_legacy_quick_play {
                if let QuickPlay::Multiplayer { host, port } = quick_play {

                    game.game_args.extend([
                        "--server".to_string(), host.clone(),
                        "--port".to_string(), port.to_string(),
                    ]);

                    quick_play_supported = true;
                    handler.handle_mojang_event(Event::FixLegacyQuickPlay {  });

                }
            }

            if !quick_play_supported {
                handler.handle_mojang_event(Event::QuickPlayNotSupported {  });
            }

        }

        if inner.fix_legacy_proxy {

            // Checking as bytes because it's ASCII and we simply matching.
            let proxy_port = match leaf_id.as_bytes() {
                [b'1', b'.', b'0' | b'1' | b'3' | b'4' | b'5'] |
                [b'1', b'.', b'2' | b'3' | b'4' | b'5', b'.', ..] |
                b"13w16a" | b"13w16b" => Some(11707),
                id if id.starts_with(b"a1.0.") => Some(80),
                id if id.starts_with(b"a1.1.") => Some(11702),
                id if id.starts_with(b"a1.") => Some(11705),
                id if id.starts_with(b"b1.") => Some(11705),
                _ => None,
            };

            if let Some(proxy_port) = proxy_port {
                game.jvm_args.push(format!("-Dhttp.proxyHost=betacraft.uk"));
                game.jvm_args.push(format!("-Dhttp.proxyPort={proxy_port}"));
                handler.handle_mojang_event(Event::FixLegacyProxy { 
                    host: "betacraft.uk", 
                    port: proxy_port,
                });
            }

        }

        if inner.fix_legacy_merge_sort && (leaf_id.starts_with("a1.") || leaf_id.starts_with("b1.")) {
            game.jvm_args.push("-Djava.util.Arrays.useLegacyMergeSort=true".to_string());
            handler.handle_mojang_event(Event::FixLegacyMergeSort {  });
        }

        if let Some((width, height)) = inner.resolution {

            let mut resolution_supported = false;
            replace_strings_args(&mut game.game_args, |arg| {
                match arg {
                    "resolution_width" => Some(width.to_string()),
                    "resolution_height" => Some(height.to_string()),
                    _ => None
                }
            });

            if !resolution_supported && inner.fix_legacy_resolution {

                game.game_args.extend([
                    "--width".to_string(), width.to_string(),
                    "--height".to_string(), height.to_string(),
                ]);

                resolution_supported = true;
                handler.handle_mojang_event(Event::FixLegacyResolution {  });

            }

            if !resolution_supported {
                handler.handle_mojang_event(Event::QuickPlayNotSupported {  });
            }

        }

        if inner.disable_multiplayer {
            game.game_args.push("--disableMultiplayer".to_string());
        }

        if inner.disable_chat {
            game.game_args.push("--disableChat".to_string());
        }

        Ok(game)

    }

}

/// Handler for events happening when installing.
pub trait Handler: standard::Handler {

    /// Handle an even from the mojang installer.
    fn handle_mojang_event(&mut self, event: Event) {
        let _ = event;
    }

    fn as_mojang_dyn(&mut self) -> &mut dyn Handler 
    where Self: Sized {
        self
    }

}

/// Blanket implementation that does nothing.
impl Handler for () { }

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
    /// Quick play has been fixed 
    FixLegacyQuickPlay {  },
    ///  proxy has been defined to fix legacy versions.
    FixLegacyProxy {
        host: &'a str,
        port: u16,
    },
    /// Legacy merge sort has been fixed.
    FixLegacyMergeSort {  },
    /// Legacy resolution arguments have been added.
    FixLegacyResolution {  },
    /// Notification of a fix of authlib:2.1.28 has happened.
    FixBrokenAuthlib {  },
    /// A quick play mode is requested by is not supported by this version, or the fix
    /// has been disabled. This is just a warning.
    QuickPlayNotSupported {  },
    /// A specific initial window resolution has been requested but it's not supported
    /// by the current version and the fix is disabled. This is just a warning.
    ResolutionNotSupported {  },
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Standard(#[from] standard::Error),
    /// A root alias version, `Release` or `Snapshot` has not been found because fetching
    /// is disabled, or if the alias is missing from the Mojang's version manifest.
    #[error("root version not found: {root:?}")]
    AliasVersionNotFound {
        root: Root,
    },
    /// The LWJGL fix is enabled with a version that is not supported, maybe because
    /// it is too old (< 3.2.3) or because of your system not being supported.
    #[error("lwjgl fix not found: {version}")]
    LwjglFixNotFound {
        version: String,
    },
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Specify the root version to start with Mojang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    /// Resolve the latest release version.
    Release,
    /// Resolve the latest snapshot version.
    Snapshot,
    /// Resolve a specific root version from its id.
    Id(String),
}

/// An impl so that we can give string-like objects to the builder.
impl<T: Into<String>> From<T> for Root {
    fn from(value: T) -> Self {
        Root::Id(value.into())
    }
}

/// This represent the optional Quick Play mode when launching the game. This is usually 
/// not supported on versions older than 1.20 (23w14a), however a fix exists for 
/// supporting multiplayer Quick Play on older versions, other modes are unsupported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickPlay {
    /// Launch the game and follow instruction for Quick Play in the given path, relative
    /// to the working directory.
    Path {
        path: PathBuf,
    },
    /// Launch the game and directly join the world given its name.
    Singleplayer {
        name: String,
    },
    /// Launch the game and directly join the specified server address.
    Multiplayer {
        host: String,
        port: u16,
    },
    /// Launch the game and directly join the realm given its id.
    Realms {
        id: String,
    },
}

/// Request the Mojang versions' manifest with the currently configured cache file.
pub fn request_manifest(handler: impl download::Handler) -> standard::Result<serde::MojangManifest> {
    
    let entry = Entry::new_cached(VERSION_MANIFEST_URL);
    let file = entry.file.to_path_buf();
    entry.download(handler)?;

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

// ========================== //
// Following code is internal //
// ========================== //

/// Internal handler given to the standard installer.
struct InternalHandler<'a, H: Handler> {
    /// Inner handler.
    inner: &'a mut H,
    /// Back-reference to the installer to know its configuration.
    installer: &'a InstallerInner,
    /// If fetching is enabled, then this contains the manifest to use.
    manifest: &'a mut Option<serde::MojangManifest>,
    /// Download informations for versions that should be downloaded.
    downloads: HashMap<String, standard::serde::Download>,
    /// Id of the "leaf" version, the last version without inherited version.
    leaf_id: &'a mut String,
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

    fn handle_standard_event_inner(&mut self, mut event: standard::Event) -> Result<()> {

        match event {
            standard::Event::HierarchyFilter { 
                ref hierarchy,
            } => {
                // Unwrap because hierarchy can't be empty.
                *self.leaf_id = hierarchy.last().unwrap().id.clone();
                self.inner.handle_standard_event(event);
            }
            standard::Event::FeaturesFilter { 
                ref mut features,
            } => {
                self.modify_features(&mut **features);
                self.inner.handle_standard_event(event);
            }
            // In this case we check the version hash just before loading it, if the hash
            // is wrong we delete the version and so the next event will be that version
            // is not found as handled below.
            standard::Event::VersionLoading { 
                id, 
                file
            } => {

                self.inner.handle_standard_event(event);

                // Ignore the version if excluded.
                if !self.installer.fetch || self.installer.fetch_exclude.iter().any(|id| id == id) {
                    return Ok(());
                }

                // Only ensure that the manifest is loaded after checking fetch exclude.
                let manifest = match self.manifest {
                    Some(manifest) => manifest,
                    None => self.manifest.insert(request_manifest(self.inner.as_download_dyn())?)
                };

                // Unwrap because we checked the manifest in the condition.
                let Some(dl) = manifest.versions.iter()
                    .find(|v| &v.id == id)
                    .map(|v| &v.download) else {
                        return Ok(());
                    };

                // Save the download information for events "VersionNotFound".
                self.downloads.insert(id.to_string(), dl.clone());

                if !check_file(file, dl.size, dl.sha1.as_deref())? {
                    
                    fs::remove_file(file)
                        .map_err(|e| standard::Error::new_io_file(e, file.to_path_buf()))?;
                    
                    self.inner.handle_mojang_event(Event::MojangVersionInvalidated { id });
                
                }
                
            }
            // In this case we handle a missing version, by finding it in the manifest.
            standard::Event::VersionNotFound { 
                id, 
                file, 
                error: _, 
                ref mut retry 
            } => {
                
                let Some(dl) = self.downloads.get(id) else {
                    self.inner.handle_standard_event(event);
                    return Ok(());
                };
                
                self.inner.handle_mojang_event(Event::MojangVersionFetching { id });
                
                EntrySource::from(dl)
                    .with_file(file.to_path_buf())
                    .download(&mut self.inner)
                    .map_err(standard::Error::Download)?;

                self.inner.handle_mojang_event(Event::MojangVersionFetched { id });

                // Retry only if no preceding error.
                **retry = true;

            }
            // Apply the various libs fixes we can apply.
            standard::Event::LibrariesFilter { 
                ref mut libraries
            } => {
                self.modify_libraries(&mut **libraries)?;                
                self.inner.handle_standard_event(event);
            },
            _ => self.inner.handle_standard_event(event),
        }

        Ok(())

    }

    /// Called from the handler to modify features.
    fn modify_features(&self, features: &mut HashSet<String>) {

        if self.installer.demo {
            features.insert("is_demo_user".to_string());
        }

        if self.installer.resolution.is_some() {
            features.insert("has_custom_resolution".to_string());
        }

        if let Some(quick_play) = &self.installer.quick_play {
            features.insert(match quick_play {
                QuickPlay::Path { .. } => "has_quick_plays_support",
                QuickPlay::Singleplayer { .. } => "is_quick_play_singleplayer",
                QuickPlay::Multiplayer { .. } => "is_quick_play_multiplayer",
                QuickPlay::Realms { .. } => "is_quick_play_realms",
            }.to_string());
        }

    }

    /// Called from the handler to modify libs.
    fn modify_libraries(&mut self, libraries: &mut Vec<Library>) -> Result<()> {

        if self.installer.fix_broken_authlib {
            self.apply_fix_broken_authlib(&mut *libraries);
        }

        if let Some(lwjgl_version) = self.installer.fix_lwjgl.as_deref() {
            self.apply_fix_lwjgl(&mut *libraries, lwjgl_version)?;
        }

        Ok(())

    }

    fn apply_fix_broken_authlib(&mut self, libraries: &mut Vec<Library>) {

        let target_gav = Gav::new("com.mojang", "authlib", "2.1.28", None, None);
        let pos = libraries.iter().position(|lib| lib.gav == target_gav);
    
        if let Some(pos) = pos {

            libraries[pos].path = None;  // Ensure that the path is recomputed.
            libraries[pos].gav.set_version("2.2.30");
            libraries[pos].source = Some(EntrySource {
                url: format!("{LIBRARIES_URL}com/mojang/authlib/2.2.30/authlib-2.2.30.jar").into_boxed_str(),
                size: Some(87497),
                sha1: Some([0xd6, 0xe6, 0x77, 0x19, 0x9a, 0xa6, 0xb1, 0x9c, 0x4a, 0x9a, 0x2e, 0x72, 0x50, 0x34, 0x14, 0x9e, 0xb3, 0xe7, 0x46, 0xf8]),
            });

            self.inner.handle_mojang_event(Event::FixBrokenAuthlib {  });

        }
    
    }
    
    fn apply_fix_lwjgl(&mut self, libraries: &mut Vec<Library>, version: &str) -> Result<()> {
    
        if version != "3.2.3" && !version.starts_with("3.3.") {
            return Err(Error::LwjglFixNotFound { 
                version: version.to_string(),
            });
        }
    
        let classifier = match (env::consts::OS, env::consts::ARCH) {
            ("windows", "x86") => "natives-windows-x86",
            ("windows", "x86_64") => "natives-windows",
            ("windows", "aarch64") if version != "3.2.3" => "natives-windows-arm64",
            ("linux", "x86" | "x86_64") => "natives-linux",
            ("linux", "arm") => "natives-linux-arm32",
            ("linux", "aarch64") => "natives-linux-arm64",
            ("macos", "x86_64") => "natives-macos",
            ("macos", "aarch64") if version != "3.2.3" => "natives-macos-arm64",
            _ => return Err(Error::LwjglFixNotFound { 
                version: version.to_string(),
            })
        };
    
        // Contains to-be-expected unique LWJGL libraries, without classifier.
        let mut lwjgl_libs = Vec::new();
    
        // Start by not retaining libraries with classifiers (natives).
        libraries.retain_mut(|lib| {
            if let ("org.lwjgl", "jar") = (lib.gav.group(), lib.gav.extension()) {
                if lib.gav.classifier().is_empty() {
                    lib.path = None;
                    lib.source = None;  // Will be updated afterward.
                    lib.gav.set_version(version);
                    lwjgl_libs.push(lib.gav.clone());
                    true
                } else {
                    // Libraries with classifiers are not retained.
                    false
                }
            } else {
                true
            }
        });
    
        // Now we add the classifiers for each LWJGL lib.
        libraries.extend(lwjgl_libs.into_iter().map(|mut gav| {
            gav.set_classifier(Some(classifier));
            Library {
                gav,
                path: None,
                source: None, // Will be set in the loop just after.
                natives: false,
            }
        }));
    
        // Finally we update the download source.
        for lib in libraries {
            if let ("org.lwjgl", "jar") = (lib.gav.group(), lib.gav.extension()) {
                let mut url = "https://repo1.maven.org/maven2".to_string();
                for component in lib.gav.file_components() {
                    url.push('/');
                    url.push_str(&component);
                }
                lib.source = Some(EntrySource::new(url));
            }
        }

        Ok(())
    
    }

}
