//! Extension to the standard installer with verification and installation of missing
//! Mojang versions, it also provides support for common arguments and fixes on legacy
//! versions.

pub mod serde;

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

use crate::standard::{self, LIBRARIES_URL, check_file, replace_strings_args, Handler as _, Library};
use crate::download::{self, Entry, EntrySource};
use crate::gav::Gav;

pub use standard::Game;


/// Static URL to the version manifest provided by Mojang.
const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";


/// An installer for supporting Mojang-provided versions. It provides support for various
/// standard arguments such as demo mode, window resolution and quick play, it also 
/// provides various fixes for known issues of old versions.
#[derive(Debug)]
pub struct Installer {
    /// The underlying standard installer logic.
    pub inner: standard::Installer,
    /// Set to true if this installer should use the online versions manifest to check
    /// already installed version and fetch them if outdated or not installed. This is
    /// enabled by default.
    pub fetch: bool,
    /// Set to true to enable the demo mode of the game.
    pub demo: bool,
    /// Optionally set an initial resolution for the game's window.
    pub resolution: Option<(u16, u16)>,
    /// Optionally enables Quick Play when launching the game, from 1.20 (23w14a).
    pub quick_play: Option<QuickPlay>,
    /// When starting versions older than 1.20 (23w14a) where Quick Play was not supported
    /// by the client, this fix tries to use legacy arguments instead, such as --server
    /// and --port, this s enabled by default.
    pub fix_legacy_quick_play: bool,
    /// When starting older alpha, beta and release up to 1.5, this allows legacy online
    /// resources such as skins to be properly requested. The implementation is currently 
    /// using `betacraft.uk` proxies.
    pub fix_legacy_proxy: bool,
    /// When starting older alpha and beta versions, this adds a JVM argument to use the
    /// legacy merge sort `java.util.Arrays.useLegacyMergeSort=true`, this is required on
    /// some old versions to avoid crashes.
    pub fix_legacy_merge_sort: bool,
    /// Versions 1.16.4 and 1.16.5 uses authlib:2.1.28 which cause multiplayer button
    /// (and probably in-game chat) to be disabled, this can be fixed by switching to
    /// version 2.2.30 of authlib.
    pub fix_authlib_2_1_28: bool,
    /// Changing the version of LWJGL, this support versions greater or equal to 3.2.3,
    /// and also provides ARM support when the LWJGL version supports it. It's not 
    /// guaranteed to work with every version of Minecraft, and downgrading LWJGL version
    /// is not recommended.
    /// 
    /// If the given version is less than 3.2.3 this will do nothing.
    pub fix_lwjgl: Option<String>,
}

impl Installer {

    pub fn with_inner(inner: standard::Installer) -> Self {
        Self {
            inner,
            fetch: true,
            demo: false,
            resolution: None,
            quick_play: None,
            fix_legacy_quick_play: true,
            fix_legacy_proxy: true,
            fix_legacy_merge_sort: true,
            fix_authlib_2_1_28: true,
            fix_lwjgl: None,
        }
    }

    pub fn into_inner(self) -> standard::Installer {
        self.inner
    }

    /// Request the Mojang versions' manifest with the currently configured cache file.
    pub fn request_manifest(&self, mut handler: impl Handler) -> standard::Result<serde::MojangManifest> {
        
        let entry = Entry::new_cached(VERSION_MANIFEST_URL);
        let file = entry.file.to_path_buf();
        entry.download(handler.as_download_dyn())?;

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

    /// Install the given Mojang version from its identifier. This also supports alias
    /// identifiers such as "release" and "snapshot" that will be resolved, note that
    /// these identifiers are just those presents in the "latest" mapping of the
    /// Mojang versions manifest. 
    /// 
    /// If the given version is not found in the manifest then it's silently ignored and
    /// the version metadata must already exists.
    pub fn install(&self, mut handler: impl Handler, id: &str) -> Result<Game> {
        
        // We only load the manifest if checking and fetching is enabled.
        let manifest = if self.fetch {
            Some(self.request_manifest(&mut handler)?)
        } else {
            None
        };

        // Resolve aliases such as "release" or "snapshot" if fetch is enabled.
        let id = manifest.as_ref()
            .and_then(|manifest| manifest.latest.get(id))
            .map(String::as_str)
            .unwrap_or(id);

        let download = manifest.as_ref()
            .and_then(|manifest| manifest.versions.iter()
                .find(|v| v.id == id)
                .map(|v| &v.download));
        
        let mut handler = InternalHandler {
            inner: handler,
            installer: self,
            id,
            download,
            error: Ok(()),
        };

        // Same as above, we are giving a &mut dyn ref to avoid huge monomorphization.
        let res = self.inner.install(handler.as_standard_dyn(), id);
        handler.error?;
        let mut game = res?;

        // If Quick Play is enabled, we know that the feature has been enabled by the
        // handler, and if the feature is actually present (1.20 and after), if not
        // present we can try to use legacy arguments for supported quick play types.
        if let Some(quick_play) = &self.quick_play {

            let quick_play_arg = match quick_play {
                QuickPlay::Path { .. } => "quickPlayPath",
                QuickPlay::Singleplayer { .. } => "quickPlaySingleplayer",
                QuickPlay::Multiplayer { .. } => "quickPlayMultiplayer",
                QuickPlay::Realms { .. } => "quickPlayRealms",
            };

            let mut quick_play_present = false;

            replace_strings_args(&mut game.game_args, |arg| {
                if arg == quick_play_arg {
                    quick_play_present = true;
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

            if !quick_play_present && self.fix_legacy_quick_play {
                if let QuickPlay::Multiplayer { host, port } = quick_play {
                    game.game_args.extend([
                        "--server".to_string(), host.clone(),
                        "--port".to_string(), port.to_string(),
                    ]);
                }
            }

            // TODO: If not supported, return an error?

        }

        if self.fix_legacy_merge_sort && (id.starts_with("a1.") || id.starts_with("b1.")) {
            game.jvm_args.push("-Djava.util.Arrays.useLegacyMergeSort=true".to_string());
        }

        Ok(game)

    }

    /// Called from the handler to modify features.
    fn modify_features(&self, features: &mut HashSet<String>) {

        if self.demo {
            features.insert("is_demo_user".to_string());
        }

        if self.resolution.is_some() {
            features.insert("has_custom_resolution".to_string());
        }

        if let Some(quick_play) = &self.quick_play {
            features.insert(match quick_play {
                QuickPlay::Path { .. } => "has_quick_plays_support",
                QuickPlay::Singleplayer { .. } => "is_quick_play_singleplayer",
                QuickPlay::Multiplayer { .. } => "is_quick_play_multiplayer",
                QuickPlay::Realms { .. } => "is_quick_play_realms",
            }.to_string());
        }

    }

    /// Called from the handler to modify libs.
    fn modify_libraries(&self, libraries: &mut Vec<Library>) -> Result<()> {

        if self.fix_authlib_2_1_28 {
            self.apply_fix_authlib_2_1_28(&mut *libraries);
        }

        if let Some(lwjgl_version) = self.fix_lwjgl.as_deref() {
            self.apply_fix_lwjgl(&mut *libraries, lwjgl_version)?;
        }

        Ok(())

    }

    fn apply_fix_authlib_2_1_28(&self, libraries: &mut Vec<Library>) {

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
        }
    
    }
    
    fn apply_fix_lwjgl(&self, libraries: &mut Vec<Library>, version: &str) -> Result<()> {
    
        if version != "3.2.3" && !version.starts_with("3.3.") {
            return Err(Error::LwjglFixNotFound { 
                version: version.to_string(),
            });
        }
    
        let classifier = match (&*self.inner.os_name, &*self.inner.os_arch) {
            ("windows", "x86") => "natives-windows-x86",
            ("windows", "x86_64") => "natives-windows",
            ("windows", "arm64") if version != "3.2.3" => "natives-windows-arm64",
            ("linux", "x86" | "x86_64") => "natives-linux",
            ("linux", "arm32") => "natives-linux-arm32",
            ("linux", "arm64") => "natives-linux-arm64",
            ("osx", "x86_64") => "natives-macos",
            ("osx", "arm64") if version != "3.2.3" => "natives-macos-arm64",
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
            let mut url = "https://repo1.maven.org/maven2".to_string();
            for component in lib.gav.file_components() {
                url.push('/');
                url.push_str(&component);
            }
            lib.source = Some(EntrySource::new(url));
        }

        Ok(())
    
    }

}

impl From<standard::Installer> for Installer {
    fn from(value: standard::Installer) -> Self {
        Self::with_inner(value)
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
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error from the standard installer.
    #[error("standard: {0}")]
    Standard(#[from] standard::Error),
    /// The LWJGL fix is enabled with a version that is not supported, maybe because
    /// it is too old (< 3.2.3) or because of your system not being supported.
    #[error("lwjgl fix not found: {version}")]
    LwjglFixNotFound {
        version: String,
    },
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

// ========================== //
// Following code is internal //
// ========================== //

/// Internal handler given to the standard installer.
struct InternalHandler<'a, H: Handler> {
    /// Inner handler.
    inner: H,
    /// Back-reference to the installer to know its configuration.
    installer: &'a Installer,
    /// The identifier of the Mojang version to launch.
    id: &'a str,
    /// The manifest version of the Mojang version to launch, only set if online check
    /// and fetch is enabled, and the version is a known Mojang one.
    download: Option<&'a standard::serde::Download>,
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
            standard::Event::FeaturesLoaded { 
                ref mut features,
            } => {
                self.installer.modify_features(&mut **features);
                self.inner.handle_standard_event(event);
            }
            // In this case we check the version hash just before loading it, if the hash
            // is wrong we delete the version and so the next event will be that version
            // is not found as handled below.
            standard::Event::VersionLoading { 
                id, 
                file
            } if id == self.id && self.download.is_some() => {

                self.inner.handle_standard_event(event);

                let dl = self.download.unwrap();
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
                retry 
            } if id == self.id && self.download.is_some() => {

                self.inner.handle_mojang_event(Event::MojangVersionFetching { id });
                
                EntrySource::from(self.download.unwrap())
                    .with_file(file.to_path_buf())
                    .download(&mut self.inner)
                    .map_err(standard::Error::Download)?;

                self.inner.handle_mojang_event(Event::MojangVersionFetched { id });

                // Retry only if no preceding error.
                *retry = true;

            }
            // Apply the various libs fixes we can apply.
            standard::Event::LibrariesLoaded { 
                ref mut libraries
            } => {
                self.installer.modify_libraries(&mut **libraries)?;                
                self.inner.handle_standard_event(event);
            },
            _ => self.inner.handle_standard_event(event),
        }

        Ok(())

    }

}
