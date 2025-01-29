//! Extension to the standard installer with verification and installation of missing
//! Mojang versions, it also provides support for common arguments and fixes on legacy
//! versions.

pub(crate) mod serde;

use std::io::{Write as _, BufReader};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::env;
use std::fs;

use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::standard::{self, 
    LIBRARIES_URL,
    check_file_advanced, 
    LibraryDownload, LoadedLibrary, VersionChannel
};
use crate::maven::Gav;
use crate::download;
use crate::msa;

pub use standard::Game;


/// Static URL to the version manifest provided by Mojang.
pub(crate) const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// An installer for supporting Mojang-provided versions. It provides support for various
/// standard arguments such as demo mode, window resolution and quick play, it also 
/// provides various fixes for known issues of old versions.
/// 
/// Notes about various versions:
/// - 1.19.3 metadata adds no parameter to specify extract directory for LWJGL (version
///   3.3.1-build-7), therefore natives are extracted to 
///   '/tmp/lwjgl&lt;username&gt;/&lt;version&gt;'.
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
    version: Version,
    fetch_exclude: Option<Vec<String>>,  // None when fetch is disabled.
    demo: bool,
    quick_play: Option<QuickPlay>,
    resolution: Option<(u16, u16)>,
    disable_multiplayer: bool,
    disable_chat: bool,
    auth_type: String,  // Empty to trigger default auth.
    auth_uuid: Uuid,
    auth_username: String,
    auth_token: String,
    auth_xuid: String,  // Apparently used for Minecraft Telemetry
    client_id: String,  // Apparently used for Minecraft Telemetry
    fix_legacy_quick_play: bool,
    fix_legacy_proxy: bool,
    fix_legacy_merge_sort: bool,
    fix_legacy_resolution: bool,
    fix_broken_authlib: bool,
    fix_lwjgl: Option<String>,
}

impl Installer {

    /// Create a new installer with default configuration, using defaults directories. 
    /// This Mojang installer has all fixes enabled except LWJGL and missing version 
    /// fetching is enabled.
    pub fn new(version: impl Into<Version>, main_dir: impl Into<PathBuf>) -> Self {
        Self {
            standard: standard::Installer::new(String::new(), main_dir),
            inner: InstallerInner {
                version: version.into(),
                fetch_exclude: Some(Vec::new()),  // Enabled by default
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
                client_id: String::new(),
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
    pub fn new_with_default(version: impl Into<Version>) -> Option<Self> {
        Some(Self::new(version, standard::default_main_dir()?))
    }

    /// Get the underlying standard installer.
    #[inline]
    pub fn standard(&self) -> &standard::Installer {
        &self.standard
    }

    /// Get the underlying standard installer through mutable reference.
    /// 
    /// *Note that the `root` property will be overwritten when installing.*
    #[inline]
    pub fn standard_mut(&mut self) -> &mut standard::Installer {
        &mut self.standard
    }

    /// Execute some callback to alter the standard installer.
    /// 
    /// *Note that the `root` property will be overwritten when installing.*
    #[inline]
    pub fn with_standard<F>(&mut self, func: F) -> &mut Self
    where
        F: FnOnce(&mut standard::Installer) -> &mut standard::Installer,
    {
        func(&mut self.standard);
        self
    }

    /// Change the version to install and start specified at construction.
    #[inline]
    pub fn set_version(&mut self, version: impl Into<Version>) -> &mut Self {
        self.inner.version = version.into();
        self
    }

    /// See [`Self::set_version`].
    #[inline]
    pub fn version(&self) -> &Version {
        &self.inner.version
    }

    /// Clear all versions from being fetch excluded.See [`Self::fetch_exclude`] and 
    /// [`Self::set_fetch_exclude_all`]. **This is the default state when constructed.**
    #[inline]
    pub fn clear_fetch_exclude(&mut self) -> &mut Self {
        self.inner.fetch_exclude = Some(Vec::new());
        self
    }

    /// Exclude all version from being fetched from the online versions manifest. This
    /// online version manifest is used to verify and install if needed each version in
    /// the hierarchy, by default, so this argument can be used to disable that.
    /// 
    /// **To don't use online manifest at all, you must also ensure that the root version
    /// is not an alias (`Release` or `Snapshot`).**
    #[inline]
    pub fn set_fetch_exclude_all(&mut self) -> &mut Self {
        self.inner.fetch_exclude = None;
        self
    }

    /// Exclude the given version id from versions that should be fetched, this has no
    /// effect if [`Self::set_fetch_exclude_all`] has already been called and not cancelled
    /// by a [`Self::clear_fetch_exclude`].
    #[inline]
    pub fn add_fetch_exclude(&mut self, id: impl Into<String>) -> &mut Self {
        if let Some(v) = &mut self.inner.fetch_exclude {
            v.push(id.into());
        }
        self
    }

    /// Return the list of version ids to be excluded from being fetched from the Mojang 
    /// manifest, this returns None if all versions are excluded.
    pub fn fetch_exclude(&self) -> Option<&[String]> {
        self.inner.fetch_exclude.as_deref()
    }

    /// Set to true to enable the demo mode of the game.
    #[inline]
    pub fn set_demo(&mut self, demo: bool) -> &mut Self {
        self.inner.demo = demo;
        self
    }

    /// See [`Self::set_demo`].
    #[inline]
    pub fn demo(&self) -> bool {
        self.inner.demo
    }

    /// Enables Quick Play when launching the game, from 1.20 (23w14a).
    #[inline]
    pub fn set_quick_play(&mut self, quick_play: QuickPlay) -> &mut Self {
        self.inner.quick_play = Some(quick_play);
        self
    }

    /// Remove Quick Play when launching, this is the default.
    #[inline]
    pub fn remove_quick_play(&mut self) -> &mut Self {
        self.inner.quick_play = None;
        self
    }

    /// See [`Self::set_quick_play`].
    #[inline]
    pub fn quick_play(&self) -> Option<&QuickPlay> {
        self.inner.quick_play.as_ref()
    }

    /// Set an initial resolution for the game's window.
    #[inline]
    pub fn set_resolution(&mut self, width: u16, height: u16) -> &mut Self {
        self.inner.resolution = Some((width, height));
        self
    }

    /// Remove initial resolution for the game's window, this is the default.
    #[inline]
    pub fn remove_resolution(&mut self) -> &mut Self {
        self.inner.resolution = None;
        self
    }

    /// See [`Self::set_resolution`].
    #[inline]
    pub fn resolution(&self) -> Option<(u16, u16)> {
        self.inner.resolution
    }

    /// Disable or not the multiplayer when launching the game.
    #[inline]
    pub fn set_disable_multiplayer(&mut self, disable_multiplayer: bool) -> &mut Self {
        self.inner.disable_multiplayer = disable_multiplayer;
        self
    }

    /// See [`Self::set_disable_multiplayer`].
    #[inline]
    pub fn disable_multiplayer(&self) -> bool {
        self.inner.disable_multiplayer
    }

    /// Disable or not the chat when launching the game.
    #[inline]
    pub fn set_disable_chat(&mut self, disable_chat: bool) -> &mut Self {
        self.inner.disable_chat = disable_chat;
        self
    }

    /// See [`Self::set_disable_chat`].
    #[inline]
    pub fn disable_chat(&self) -> bool {
        self.inner.disable_chat
    }

    /// Manually set the authentication UUID, not touching any other parameter.
    pub fn set_auth_raw_uuid(&mut self, uuid: Uuid) -> &mut Self {
        self.inner.auth_uuid = uuid;  // TODO: add missing other methods
        self
    }

    /// Internal function to reset to zero-length all online-related auth variables.
    #[inline(always)]
    fn reset_auth_online(&mut self) -> &mut Self {
        self.inner.auth_type = String::new();
        self.inner.auth_token = String::new();
        self.inner.auth_xuid = String::new();
        self
    }

    /// Use offline session with the given UUID and username, note that the username will
    /// be truncated 16 bytes at most (this function will panic if the truncation is not 
    /// on a valid UTF-8 character boundary).
    pub fn set_auth_offline(&mut self, uuid: Uuid, username: impl Into<String>) -> &mut Self {
        self.inner.auth_uuid = uuid;
        self.inner.auth_username = username.into();
        self.inner.auth_username.truncate(16);
        self.reset_auth_online()
    }

    /// Use offline session with the given UUID, the username is derived from the first
    /// 8 characters of the rendered UUID.
    pub fn set_auth_offline_uuid(&mut self, uuid: Uuid) -> &mut Self {
        self.inner.auth_uuid = uuid;
        self.inner.auth_username = uuid.to_string();
        self.inner.auth_username.truncate(8);
        self.reset_auth_online()
    }

    /// Use offline session with a deterministic UUID, derived from this machine's 
    /// hostname, the username is then derived from the UUID following the same logic
    /// as for [`Self::set_auth_offline_uuid`].
    /// 
    /// **This is the default UUID/username used if no auth is specified, so you don't
    /// need to call this function, except if you want to override previous auth.**
    pub fn set_auth_offline_hostname(&mut self) -> &mut Self {
        self.set_auth_offline_uuid(Uuid::new_v5(&standard::UUID_NAMESPACE, gethostname::gethostname().as_encoded_bytes()))
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
    /// [`Self::set_auth_offline_username`] instead.
    pub fn set_auth_offline_username_legacy(&mut self, username: impl Into<String>) -> &mut Self {
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
    pub fn set_auth_offline_username(&mut self, username: impl Into<String>) -> &mut Self {
        
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
    pub fn set_auth_msa(&mut self, account: &msa::Account) -> &mut Self {
        self.inner.auth_uuid = account.uuid().clone();
        self.inner.auth_username = account.username().to_string();
        self.inner.auth_token = account.access_token().to_string();
        self.inner.auth_type = "msa".to_string();
        self.inner.auth_xuid = account.xuid().to_string();
        self
    }

    /// See [`Self::set_client_id`].
    #[inline]
    pub fn client_id(&self) -> &str {
        &self.inner.client_id
    }

    /// Set the client ID used for telemetry of the game. The default client id is empty
    /// and so the telemetry can't use it.
    #[inline]
    pub fn set_client_id(&mut self, client_id: impl Into<String>) -> &mut Self {
        self.inner.client_id = client_id.into();
        self
    }

    /// When starting versions older than 1.20 (23w14a) where Quick Play was not supported
    /// by the client, this fix tries to use legacy arguments instead, such as --server
    /// and --port, this is enabled by default.
    #[inline]
    pub fn set_fix_legacy_quick_play(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_quick_play = fix;
        self
    }

    /// See [`Self::set_fix_legacy_quick_play`].
    #[inline]
    pub fn fix_legacy_quick_play(&self) -> bool {
        self.inner.fix_legacy_quick_play
    }

    /// When starting older alpha, beta and release up to 1.5, this allows legacy online
    /// resources such as skins to be properly requested. The implementation is currently 
    /// using `betacraft.uk` proxies.
    #[inline]
    pub fn set_fix_legacy_proxy(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_proxy = fix;
        self
    }

    /// See [`Self::set_fix_legacy_proxy`].
    #[inline]
    pub fn fix_legacy_proxy(&self) -> bool {
        self.inner.fix_legacy_proxy
    }

    /// When starting older alpha and beta versions, this adds a JVM argument to use the
    /// legacy merge sort `java.util.Arrays.useLegacyMergeSort=true`, this is required on
    /// some old versions to avoid crashes.
    #[inline]
    pub fn set_fix_legacy_merge_sort(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_merge_sort = fix;
        self
    }

    /// See [`Self::set_fix_legacy_merge_sort`].
    #[inline]
    pub fn fix_legacy_merge_sort(&self) -> bool {
        self.inner.fix_legacy_merge_sort
    }

    /// When starting older versions that don't support modern resolution arguments, this
    /// fix will add arguments to force resolution of the initial window.
    #[inline]
    pub fn set_fix_legacy_resolution(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_legacy_resolution = fix;
        self
    }

    /// See [`Self::set_fix_legacy_resolution`].
    #[inline]
    pub fn fix_legacy_resolution(&self) -> bool {
        self.inner.fix_legacy_resolution
    }

    /// Versions 1.16.4 and 1.16.5 uses authlib:2.1.28 which cause multiplayer button
    /// (and probably in-game chat) to be disabled, this can be fixed by switching to
    /// version 2.2.30 of authlib.
    #[inline]
    pub fn set_fix_broken_authlib(&mut self, fix: bool) -> &mut Self {
        self.inner.fix_broken_authlib = fix;
        self
    }

    /// See [`Self::set_fix_broken_authlib`].
    #[inline]
    pub fn fix_broken_authlib(&self) -> bool {
        self.inner.fix_broken_authlib
    }

    /// Changing the version of LWJGL, this support versions greater or equal to 3.2.3,
    /// and also provides ARM support when the LWJGL version supports it. It's not 
    /// guaranteed to work with every version of Minecraft, and downgrading LWJGL version
    /// is not recommended.
    /// 
    /// If the given version is less than 3.2.3 this will do nothing.
    #[inline]
    pub fn set_fix_lwjgl(&mut self, lwjgl_version: impl Into<String>) -> &mut Self {
        self.inner.fix_lwjgl = Some(lwjgl_version.into());
        self
    }
    
    /// Don't fix LWJGL version (see [`Self::fix_lwjgl`]).
    #[inline]
    pub fn remove_fix_lwjgl(&mut self) -> &mut Self {
        self.inner.fix_lwjgl = None;
        self
    }

    /// See [`Self::set_fix_lwjgl`].
    #[inline]
    pub fn fix_lwjgl(&self) -> Option<&str> {
        self.inner.fix_lwjgl.as_deref()
    }

    /// Install the given Mojang version from its identifier. This also supports alias
    /// identifiers such as "release" and "snapshot" that will be resolved, note that
    /// these identifiers are just those presents in the "latest" mapping of the
    /// Mojang versions manifest. 
    /// 
    /// If the given version is not found in the manifest then it's silently ignored and
    /// the version metadata must already exists.
    #[inline]
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {
        self.install_dyn(&mut handler)
    }

    #[inline(never)]
    pub fn install_dyn(&mut self, handler: &mut dyn Handler) -> Result<Game> {
        
        // Apply default offline auth, derived from hostname.
        if self.inner.auth_username.is_empty() {
            self.set_auth_offline_hostname();
        }

        let Self {
            ref mut standard,
            ref inner,
        } = self;

        let manifest = match self.inner.version {
            Version::Release | 
            Version::Snapshot => Some(Manifest::request(&mut *handler)?),
            _ => None
        };

        let version = match &self.inner.version {
            Version::Release => manifest.as_ref().unwrap().latest_release_name(),
            Version::Snapshot => manifest.as_ref().unwrap().latest_snapshot_name(),
            Version::Name(name) => name.as_str(),
        };

        standard.set_version(version);
        
        // Let the handler find the "leaf" version.
        let mut leaf_version = String::new();

        // Scoping the temporary internal handler.
        let mut game = {

            let mut handler = InternalHandler {
                inner: &mut *handler,
                installer: &inner,
                error: Ok(()),
                manifest,
                leaf_version: &mut leaf_version,
            };
    
            // Same as above, we are giving a &mut dyn ref to avoid huge monomorphization.
            let res = standard.install(&mut handler);
            handler.error?;
            res?

        };

        // Apply auth parameters.
        game.replace_args(|arg| {
            Some(match arg {
                "auth_player_name" => inner.auth_username.clone(),
                "auth_uuid" => inner.auth_uuid.as_simple().to_string(),
                "auth_access_token" => inner.auth_token.clone(),
                "auth_xuid" => inner.auth_xuid.clone(),
                // Legacy parameter
                "auth_session" if !inner.auth_token.is_empty() => 
                    format!("token:{}:{}", inner.auth_token, inner.auth_uuid.as_simple()),
                "auth_session" => String::new(),
                "user_type" => inner.auth_type.clone(),
                "user_properties" => format!("{{}}"),
                "clientid" => inner.client_id.clone(),
                _ => return None
            })
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
            game.replace_args(|arg| {
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
                    handler.fixed_legacy_quick_play();

                }
            }

            if !quick_play_supported {
                handler.warn_unsupported_quick_play();
            }

        }

        if inner.fix_legacy_proxy {

            // Checking as bytes because it's ASCII and we simply matching.
            let proxy_port = match leaf_version.as_bytes() {
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
                handler.fixed_legacy_proxy("betacraft.uk", proxy_port);
            }

        }

        if inner.fix_legacy_merge_sort && (leaf_version.starts_with("a1.") || leaf_version.starts_with("b1.")) {
            game.jvm_args.push("-Djava.util.Arrays.useLegacyMergeSort=true".to_string());
            handler.fixed_legacy_merge_sort();
        }

        if let Some((width, height)) = inner.resolution {

            let mut resolution_supported = false;
            game.replace_args(|arg| {
                let repl = match arg {
                    "resolution_width" => width.to_string(),
                    "resolution_height" => height.to_string(),
                    _ => return None
                };
                resolution_supported = true;
                Some(repl)
            });

            if !resolution_supported && inner.fix_legacy_resolution {

                game.game_args.extend([
                    "--width".to_string(), width.to_string(),
                    "--height".to_string(), height.to_string(),
                ]);

                resolution_supported = true;
                handler.fixed_legacy_resolution();

            }

            if !resolution_supported {
                handler.warn_unsupported_resolution();
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

crate::trait_event_handler! {
    /// Handler for events happening when installing.
    pub trait Handler: standard::Handler {
        
        /// When the given version is being loaded but the file has an invalid size,
        /// SHA-1, or any other invalidating reason, it has been removed in order to 
        /// download an up-to-date version.
        fn invalidated_version(version: &str);
        /// The required version metadata is missing and so will be fetched.
        fn fetch_version(version: &str);
        /// The version has been fetched.
        fn fetched_version(version: &str);

        /// Quick play has been fixed.
        fn fixed_legacy_quick_play();
        /// Legacy proxy has been defined to fix legacy versions.
        fn fixed_legacy_proxy(host: &str, port: u16);
        /// Legacy merge sort has been fixed.
        fn fixed_legacy_merge_sort();
        /// Legacy resolution arguments have been added.
        fn fixed_legacy_resolution();
        /// Notification of a fix of authlib:2.1.28 has happened.
        fn fixed_broken_authlib();

        /// A quick play mode is requested by is not supported by this version, or the fix
        /// has been disabled. This is just a warning.
        fn warn_unsupported_quick_play();
        /// A specific initial window resolution has been requested but it's not supported
        /// by the current version and the fix is disabled. This is just a warning.
        fn warn_unsupported_resolution();

    }
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Standard(#[source] standard::Error),
    /// The LWJGL fix is enabled with a version that is not supported, maybe because
    /// it is too old (< 3.2.3) or because of your system not being supported.
    #[error("lwjgl fix not found: {version}")]
    LwjglFixNotFound {
        version: String,
    },
}

impl<T: Into<standard::Error>> From<T> for Error {
    fn from(value: T) -> Self {
        Error::Standard(value.into())
    }
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

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

/// The version to install.
#[derive(Debug, Clone)]
pub enum Version {
    /// Install the latest Mojang release.
    Release,
    /// Install the latest Mojang snapshot.
    Snapshot,
    /// Install this specific game version, if not a Mojang-provided version, it should
    /// be already installed in the versions directory.
    Name(String),
}

/// An impl so that we can give string-like objects to the builder.
impl<T: Into<String>> From<T> for Version {
    fn from(value: T) -> Self {
        Self::Name(value.into())
    }
}

/// A handle to the Mojang versions manifest.
#[derive(Debug)]
pub struct Manifest {
    inner: Box<serde::MojangManifest>,
}

impl Manifest {

    /// Request the Mojang versions' manifest. It takes a download handler because this 
    /// it will download it in cache and reuse any previous one that is still valid.
    pub fn request(handler: impl download::Handler) -> Result<Self> {

        let mut entry = download::single_cached(VERSION_MANIFEST_URL)
            .set_keep_open()
            .download(handler)??;

        let reader = BufReader::new(entry.take_handle().unwrap());
        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let manifest = serde_path_to_error::deserialize::<_, Box<serde::MojangManifest>>(&mut deserializer)
            .map_err(|e| standard::Error::new_json_file(e, entry.file()))?;

        Ok(Self { inner: manifest })

    }

    /// Iterator over all versions in the manifest.
    /// 
    /// This method currently returns an abstract iterator because the API is not 
    /// stabilized yet.
    pub fn iter(&self) -> impl Iterator<Item = ManifestVersion<'_>> + use<'_> {
        self.inner.versions.iter()
            .map(ManifestVersion)
    }

    /// Return the latest release version name.
    #[inline]
    pub fn latest_release_name(&self) -> &str {
        &self.inner.latest.release
    }

    /// Return the latest snapshot version name.
    #[inline]
    pub fn latest_snapshot_name(&self) -> &str {
        &self.inner.latest.release
    }

    /// Find the index of a version given its name.
    pub fn find_index_of_name(&self, name: &str) -> Option<usize> {
        self.inner.versions.iter().position(|v| v.id == name)
    }

    /// Get a version from its index within the manifest.
    pub fn find_by_index(&self, index: usize) -> Option<ManifestVersion<'_>> {
        self.inner.versions.get(index).map(ManifestVersion)
    }

    /// Get a handle to a version information from its name.
    pub fn find_by_name(&self, name: &str) -> Option<ManifestVersion<'_>> {
        self.inner.versions.iter()
            .find(|v| v.id == name)
            .map(ManifestVersion)
    }

}

/// A handle to a version in the Mojang versions manifest.
#[derive(Debug)]
pub struct ManifestVersion<'a>(&'a serde::MojangManifestVersion);

impl<'a> ManifestVersion<'a> {

    /// The name of this version.
    /// 
    /// See [`standard::LoadedVersion::name`] for more information on the naming.
    pub fn name(&self) -> &'a str {
        &self.0.id
    }

    /// The release channel of this version.
    pub fn channel(&self) -> VersionChannel {
        VersionChannel::from(self.0.r#type)
    }

    /// The last update time for this version.
    pub fn time(&self) -> &'a DateTime<FixedOffset> {
        &self.0.time
    }

    /// The release time for this version. 
    pub fn release_time(&self) -> &'a DateTime<FixedOffset> {
        &self.0.release_time
    }

    /// Return the download URL to this version metadata.
    pub fn url(&self) -> &'a str {
        &self.0.download.url
    }

    /// Return the expected size of this version metadata, if any.
    pub fn size(&self) -> Option<u32> {
        self.0.download.size
    }

    /// Return the expected SHA-1 of this version metadata, if any.
    pub fn sha1(&self) -> Option<&'a [u8; 20]> {
        self.0.download.sha1.as_deref()
    }

}

// ========================== //
// Following code is internal //
// ========================== //

/// Internal handler given to the standard installer.
struct InternalHandler<'a> {
    /// Inner handler.
    inner: &'a mut dyn Handler,
    /// Back-reference to the installer to know its configuration.
    installer: &'a InstallerInner,
    /// If there is an error in the handler.
    error: Result<()>,
    /// If fetching is enabled, then this contains the manifest to use.
    manifest: Option<Manifest>,
    /// Id of the "leaf" version, the last version without inherited version.
    leaf_version: &'a mut String,
}

impl download::Handler for InternalHandler<'_> {
    
    fn fallback(&mut self, _token: crate::sealed::Token) -> Option<&mut dyn download::Handler> {
        Some(&mut self.inner)
    }

}

impl standard::Handler for InternalHandler<'_> {

    fn fallback(&mut self, _token: crate::sealed::Token) -> Option<&mut dyn standard::Handler> {
        Some(&mut self.inner)
    }

    fn filter_features(&mut self, features: &mut HashSet<String>) {
        
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

    fn loaded_hierarchy(&mut self, hierarchy: &[standard::LoadedVersion]) {
        *self.leaf_version = hierarchy.last().unwrap().name().to_string();
        self.inner.loaded_hierarchy(hierarchy);
    }

    fn load_version(&mut self, version: &str, file: &Path) {
        self.inner.load_version(version, file);
        match self.inner_load_version(version, file) {
            Ok(()) => (),
            Err(e) => self.error = Err(e),
        }
    }

    fn need_version(&mut self, version: &str, file: &Path) -> bool {
        match self.inner_need_version(version, file) {
            Ok(true) => return true,
            Ok(false) => (),
            Err(e) => self.error = Err(e),
        }
        self.inner.need_version(version, file)
    }

    fn filter_libraries(&mut self, libraries: &mut Vec<LoadedLibrary>) {
        
        if self.installer.fix_broken_authlib {
            self.apply_fix_broken_authlib(&mut *libraries);
        }

        if let Some(lwjgl_version) = self.installer.fix_lwjgl.as_deref() {
            match self.apply_fix_lwjgl(&mut *libraries, lwjgl_version) {
                Ok(()) => (),
                Err(e) => self.error = Err(e),
            }
        }

    }

}

impl InternalHandler<'_> {

    fn inner_load_version(&mut self, version: &str, file: &Path) -> Result<()> {

        // Ignore the version if excluded.
        let Some(exclude) = &self.installer.fetch_exclude else {
            return Ok(());
        };

        if exclude.iter().any(|excluded_id| excluded_id == version) {
            return Ok(());
        }

        // Only ensure that the manifest is loaded after checking fetch exclude.
        let manifest = match self.manifest {
            Some(ref manifest) => manifest,
            None => self.manifest.insert(Manifest::request(&mut *self.inner)?)
        };

        // Unwrap because we checked the manifest in the condition.
        let Some(version) = manifest.find_by_name(version) else {
            return Ok(());
        };

        if !check_file_advanced(file, version.size(), version.sha1(), true)? {
            
            fs::remove_file(file)
                .map_err(|e| standard::Error::new_io_file(e, file))?;
            
            self.inner.invalidated_version(version.name());
        
        }

        Ok(())

    }

    fn inner_need_version(&mut self, version: &str, file: &Path) -> Result<bool> {

        let Some(manifest) = self.manifest.as_ref() else {
            return Ok(false);
        };
        
        let Some(version) = manifest.find_by_name(version) else {
            return Ok(false);
        };
        
        self.inner.fetch_version(version.name());
        
        download::single(version.url(), file)
            .set_expected_size(version.size())
            .set_expected_sha1(version.sha1().copied())
            .download(&mut *self.inner)??;

        self.inner.fetched_version(version.name());

        Ok(true)

    }

    fn apply_fix_broken_authlib(&mut self, libraries: &mut Vec<LoadedLibrary>) {

        let target_gav = Gav::new("com.mojang", "authlib", "2.1.28", None, None);
        let pos = libraries.iter().position(|lib| lib.gav == target_gav);
    
        if let Some(pos) = pos {

            libraries[pos].path = None;  // Ensure that the path is recomputed.
            libraries[pos].gav.set_version("2.2.30");
            libraries[pos].download = Some(LibraryDownload {
                url: format!("{LIBRARIES_URL}com/mojang/authlib/2.2.30/authlib-2.2.30.jar"),
                size: Some(87497),
                sha1: Some([0xd6, 0xe6, 0x77, 0x19, 0x9a, 0xa6, 0xb1, 0x9c, 0x4a, 0x9a, 0x2e, 0x72, 0x50, 0x34, 0x14, 0x9e, 0xb3, 0xe7, 0x46, 0xf8]),
            });

            self.inner.fixed_broken_authlib();

        }
    
    }
    
    fn apply_fix_lwjgl(&mut self, libraries: &mut Vec<LoadedLibrary>, version: &str) -> Result<()> {
    
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
            if let ("org.lwjgl", "jar") = (lib.gav.group(), lib.gav.extension_or_default()) {
                if lib.gav.classifier().is_none() {
                    lib.path = None;
                    lib.download = None;  // Will be updated afterward.
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
            LoadedLibrary {
                gav,
                path: None,
                download: None, // Will be set in the loop just after.
                natives: false,
            }
        }));
    
        // Finally we update the download source.
        for lib in libraries {
            if let ("org.lwjgl", "jar") = (lib.gav.group(), lib.gav.extension_or_default()) {
                let url = format!("https://repo1.maven.org/maven2/{}", lib.gav.url());
                lib.download = Some(LibraryDownload { url, size: None, sha1: None });
            }
        }

        Ok(())
    
    }

}
