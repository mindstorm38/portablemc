//! The base installation procedure.

pub(crate) mod serde;

use std::io::{self, BufReader, BufWriter, Seek, SeekFrom};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::fmt::{self, Debug, Write as _};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fs::{self, File};
use std::sync::LazyLock;
use std::time::Duration;
use std::{env, thread};
use std::ffi::OsStr;

use indexmap::IndexSet;

use zip::ZipArchive;

use sha1::{Digest, Sha1};
use uuid::{uuid, Uuid};

use crate::path::{PathExt, PathBufExt};
use crate::download::{self, Batch};
use crate::maven::Gav;


/// Base URL for downloading game's assets.
pub(crate) const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";

/// The URL to meta manifest for Mojang-provided JVMs. 
pub(crate) const JVM_META_MANIFEST_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Base URL for libraries.
pub(crate) const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

/// The UUID namespace of PMC, used in various places.
pub(crate) const UUID_NAMESPACE: Uuid = uuid!("8df5a464-38de-11ec-aa66-3fd636ee2ed7");

/// The default JVM arguments used if no one are presents, such as for old versions.
pub(crate) const LEGACY_JVM_ARGS: &[&str] = &[
    "-Djava.library.path=${natives_directory}",
    "-Dminecraft.launcher.brand=${launcher_name}",
    "-Dminecraft.launcher.version=${launcher_version}",
    "-cp",
    "${classpath}",
];

/// The installer that supports the minimal basic format for version metadata with
/// support for libraries, assets and loggers automatic installation. By defaults, it 
/// also supports finding a suitable JVM for running the game and installs one provided
/// by Mojang as a fallback.
/// 
/// Note that this installer doesn't provide any fetching of missing versions, enables
/// no feature by default and provides no fixes for legacy things. This installer just
/// implements the basics of how Minecraft versions are specified, this is mostly from
/// reverse engineering. **Most of the time, you don't want to use this directly, instead
/// you can use the [`mojang::Installer`](crate::mojang::Installer), that provides support
/// for fetching missing Mojang versions, various fixes and authentication support.**
#[derive(Debug, Clone)]
pub struct Installer {
    version: String,
    versions_dir: PathBuf,
    libraries_dir: PathBuf,
    assets_dir: PathBuf,
    jvm_dir: PathBuf,
    bin_dir: PathBuf,
    mc_dir: PathBuf,
    strict_assets_check: bool,
    strict_libraries_check: bool,
    strict_jvm_check: bool,
    jvm_policy: JvmPolicy,
    launcher_name: Option<String>,
    launcher_version: Option<String>,
}

impl Installer {

    /// Create a new installer with default configuration and the given root version.
    /// 
    /// If the various directories to be configured are not configured then they will be
    /// derived from the default main directory.
    pub fn new(version: impl Into<String>) -> Self {
        
        let mc_dir = default_main_dir().unwrap_or_else(|| Path::new(""));

        Self {
            version: version.into(),
            versions_dir: mc_dir.join("versions"),
            libraries_dir: mc_dir.join("libraries"),
            assets_dir: mc_dir.join("assets"),
            jvm_dir: mc_dir.join("jvm"),
            bin_dir: mc_dir.join("bin"),
            mc_dir: mc_dir.to_path_buf(),
            strict_assets_check: false,
            strict_libraries_check: false,
            strict_jvm_check: false,
            jvm_policy: JvmPolicy::SystemThenMojang,
            launcher_name: None,
            launcher_version: None,
        }

    }

    /// Get the root version to load with its hierarchy and install.
    #[inline]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Set the root version to load with its hierarchy and install.
    #[inline]
    pub fn set_version(&mut self, version: impl Into<String>) -> &mut Self {
        self.version = version.into();
        self
    }

    /// The directory where versions are stored.
    #[inline]
    pub fn versions_dir(&self) -> &Path {
        &self.versions_dir
    }

    /// See [`Self::versions_dir`].
    #[inline]
    pub fn set_versions_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.versions_dir = dir.into();
        self
    }

    /// The directory where libraries are stored, organized like a maven repository.
    #[inline]
    pub fn libraries_dir(&self) -> &Path {
        &self.libraries_dir
    }

    /// See [`Self::libraries_dir`].
    #[inline]
    pub fn set_libraries_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.libraries_dir = dir.into();
        self
    }

    /// The directory where assets, assets index, cached skins and logs config are stored.
    /// Note that this directory stores caches player skins, so this is the only 
    /// directory where the client will need to write, and so it needs the permission
    /// to do so.
    #[inline]
    pub fn assets_dir(&self) -> &Path {
        &self.assets_dir
    }

    /// See [`Self::assets_dir`].
    #[inline]
    pub fn set_assets_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.assets_dir = dir.into();
        self
    }

    /// The directory where Mojang-provided JVM has been installed.
    #[inline]
    pub fn jvm_dir(&self) -> &Path {
        &self.jvm_dir
    }

    /// See [`Self::jvm_dir`].
    #[inline]
    pub fn set_jvm_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.jvm_dir = dir.into();
        self
    }

    /// The directory used to extract natives into (.dll, .so) before startup, in modern
    /// versions the launcher no longer extract natives itself, instead LWJGL is auto
    /// extracting its own needed natives into that directory. The user launching the
    /// game should have read/write permissions to this directory.
    /// 
    /// Note that a sub-directory will be created with a name that is kind of a hash of
    /// class files and natives files paths. This directory is considered temporary, not
    /// really heavy and so can be removed after all instances of the game have been 
    /// terminated, it can also be set to something like `/tmp/pmc` on Linux for example.
    #[inline]
    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// See [`Self::bin_dir`].
    #[inline]
    pub fn set_bin_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.bin_dir = dir.into();
        self
    }

    /// The directory where the process' working directory is set and all user stuff is
    /// saved (saves, resource packs, options and more). The user launching the
    /// game should have read/write permissions to this directory.
    #[inline]
    pub fn mc_dir(&self) -> &Path {
        &self.mc_dir
    }

    /// See [`Self::mc_dir`].
    #[inline]
    pub fn set_mc_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.mc_dir = dir.into();
        self
    }

    /// Shortcut for defining the various main directories of the game, by deriving
    /// the given path, the directories `versions`, `assets`, `libraries` and `jvm`
    /// are defined.
    /// 
    /// **Note that on Windows**, long NT UNC paths are very likely to be unsupported and
    /// you'll get unsound errors with the JVM or the game itself.
    #[inline]
    pub fn set_main_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        let mc_dir = dir.into();
        self.versions_dir = mc_dir.join("versions");
        self.assets_dir = mc_dir.join("assets");
        self.libraries_dir = mc_dir.join("libraries");
        self.jvm_dir = mc_dir.join("jvm");
        self.bin_dir = mc_dir.join("bin");
        self.mc_dir = mc_dir;
        self
    }

    /// When enabled, all assets are strictly checked against their expected SHA-1,
    /// this is disabled by default because it's heavy on CPU.
    #[inline]
    pub fn strict_assets_check(&self) -> bool {
        self.strict_assets_check
    }

    /// See [`Self::strict_assets_check`].
    #[inline]
    pub fn set_strict_assets_check(&mut self, strict: bool) -> &mut Self {
        self.strict_assets_check = strict;
        self
    }

    /// When enabled, all libraries are strictly checked against their expected SHA-1,
    /// this is disabled by default because it's heavy on CPU.
    #[inline]
    pub fn strict_libraries_check(&self) -> bool {
        self.strict_libraries_check
    }

    /// See [`Self::strict_libraries_check`].
    #[inline]
    pub fn set_strict_libraries_check(&mut self, strict: bool) -> &mut Self {
        self.strict_libraries_check = strict;
        self
    }

    /// When enabled, all files from Mojang-provided JVMs are strictly checked against
    /// their expected SHA-1, this is disabled by default because it's heavy on CPU.
    #[inline]
    pub fn strict_jvm_check(&self) -> bool {
        self.strict_jvm_check
    }

    /// See [`Self::set_strict_jvm_check`].
    #[inline]
    pub fn set_strict_jvm_check(&mut self, strict: bool) -> &mut Self {
        self.strict_jvm_check = strict;
        self
    }

    /// The policy for finding a JVM to run the game on.
    #[inline]
    pub fn jvm_policy(&self) -> &JvmPolicy {
        &self.jvm_policy
    }

    /// See [`Self::jvm_policy`].
    #[inline]
    pub fn set_jvm_policy(&mut self, policy: JvmPolicy) -> &mut Self {
        self.jvm_policy = policy;
        self
    }

    /// A specific launcher name to put on the command line, defaults to "portablemc".
    pub fn launcher_name(&self) -> &str {
        self.launcher_name.as_deref().unwrap_or(env!("CARGO_PKG_NAME"))
    }

    /// See [`Self::launcher_name`].
    #[inline]
    pub fn set_launcher_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.launcher_name = Some(name.into());
        self
    }

    /// A specific launcher version to put on the command line, defaults to PMC version.
    pub fn launcher_version(&self) -> &str {
        self.launcher_version.as_deref().unwrap_or(env!("CARGO_PKG_VERSION"))
    }

    /// See [`Self::launcher_version`].
    #[inline]
    pub fn set_launcher_version(&mut self, version: impl Into<String>) -> &mut Self {
        self.launcher_version = Some(version.into());
        self
    }

    /// Ensure that a the given version, from its id, is fully installed and return
    /// a game instance that can be used to run launch it.
    #[inline]
    pub fn install(&mut self, mut handler: impl Handler) -> Result<Game> {
        self.install_dyn(&mut handler)
    }

    /// Inner install function to force dyn dispatch.
    #[inline(never)]
    fn install_dyn(&mut self, handler: &mut dyn Handler) -> Result<Game> {
        
        // Start by setting up features.
        let mut features = HashSet::new();
        handler.on_event(Event::FilterFeatures { features: &mut features });
        handler.on_event(Event::LoadedFeatures { features: &features });
        
        // Then we have a sequence of steps that may add entries to the download batch.
        let mut batch = Batch::new();
        let hierarchy = self.load_hierarchy(&mut *handler, &self.version)?;
        let mut lib_files = self.load_libraries(&mut *handler, &hierarchy, &features, &mut batch)?;
        let logger_config = self.load_logger(&mut *handler, &hierarchy, &mut batch)?;
        let assets = self.load_assets(&mut *handler, &hierarchy, &mut batch)?;
        let jvm = self.load_jvm(&mut *handler, &hierarchy, &mut batch)?;

        // If we don't find the main class it is impossible to launch.
        let main_class = hierarchy.iter()
            .find_map(|v| v.metadata.main_class.as_ref())
            .cloned()
            .ok_or(Error::MainClassNotFound {  })?;

        // Only trigger download events if the batch is not empty. Note that in this
        // module and generally in this crate we transform handlers to a dynamic download
        // handler '&mut dyn download::Handler' to avoid large polymorphism duplications.
        if !batch.is_empty() {
            
            let mut cancel = false;
            handler.on_event(Event::DownloadResources { cancel: &mut cancel });

            if cancel {
                return Err(Error::DownloadResourcesCancelled {  });
            }

            batch.download((&mut *handler).into_download())
                .map_err(|e| Error::new_reqwest(e, "download resources"))?
                .into_result()?;

            handler.on_event(Event::DownloadedResources);

        }

        // Finalization of libraries to create a unique bin dir and extract them into.
        let bin_dir = self.finalize_libraries(&mut *handler, &mut lib_files)?;

        // Final installation step is to finalize assets if virtual or resource mapping.
        if let Some(assets) = &assets {
            self.finalize_assets(assets)?;
        }

        // Finalization of JVM is needed to ensure executable and linked files.
        self.finalize_jvm(&jvm)?;

        // Resolve arguments from the hierarchy of versions.
        let mut jvm_args = Vec::new();
        let mut game_args = Vec::new();

        for version in &hierarchy {
            if let Some(version_args) = &version.metadata.arguments {
                self.check_args(&mut jvm_args, &version_args.jvm, &features, None);
                self.check_args(&mut game_args, &version_args.game, &features, None);
            } else if let Some(version_legacy_args) = &version.metadata.legacy_arguments {
                // Legacy args are overwriting everything and abort child version.
                jvm_args = LEGACY_JVM_ARGS.iter().copied().map(str::to_string).collect::<Vec<_>>();
                game_args = version_legacy_args.split_whitespace().map(str::to_string).collect::<Vec<_>>();
                break;
            }
        }

        // The logger configuration is an additional JVM argument.
        if let Some(logger_config) = &logger_config {
            let logger_file = canonicalize_file(&logger_config.file)?;
            jvm_args.push(logger_config.argument.replace("${path}", &logger_file.to_string_lossy()));
        }

        // We also canonicalize paths that will probably be used by args replacements...
        let bin_dir = canonicalize_file(&bin_dir)?;
        let mc_dir = canonicalize_file(&self.mc_dir)?;
        let libraries_dir = canonicalize_file(&self.libraries_dir)?;
        let assets_dir = canonicalize_file(&self.assets_dir)?;
        let jvm_file = canonicalize_file(&jvm.file)?;
        let assets_virtual_dir = match &assets {
            Some(Assets { mapping: Some(mapping), .. }) => Some(canonicalize_file(&mapping.virtual_dir)?),
            _ => None,
        };

        // Closure to replace arguments in both JVM and game args.
        let repl_arg = |arg: &str| {
            Some(match arg {
                // This is used by some mod loaders...
                #[cfg(windows)]      "classpath_separator" => ";".to_string(),
                #[cfg(not(windows))] "classpath_separator" => ":".to_string(),
                "classpath" => env::join_paths(lib_files.class_files.iter())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                "natives_directory" => bin_dir.display().to_string(),
                // Information about launcher anv launched version.
                "launcher_name" => self.launcher_name().to_string(),
                "launcher_version" => self.launcher_version().to_string(),
                "version_name" => hierarchy[0].name.clone(),
                "version_type" => return hierarchy.iter() // First occurrence of 'type'.
                    .filter_map(|v| v.metadata.r#type.as_ref())
                    .map(|t| t.as_str().to_string())
                    .next(),
                // Same as the mc dir for simplification of the abstraction.
                "game_directory" => mc_dir.display().to_string(),
                // Has been observed in some custom versions...
                "library_directory" => libraries_dir.display().to_string(),
                // Modern objects-based assets...
                "assets_root" => assets_dir.display().to_string(),
                "assets_index_name" => return assets.as_ref()
                    .map(|assets| assets.id.clone()),
                // Legacy assets...
                "game_assets" => return assets_virtual_dir.as_ref()
                    .map(|dir| dir.display().to_string()),
                _ => return None
            })
        };

        replace_strings_args(&mut jvm_args, repl_arg);
        replace_strings_args(&mut game_args, repl_arg);

        Ok(Game {
            jvm_file, 
            mc_dir,
            main_class, 
            jvm_args, 
            game_args,
        })

    }

    /// Internal function that loads the version hierarchy from their JSON metadata files.
    fn load_hierarchy(&self, 
        handler: &mut dyn Handler, 
        root_version: &str
    ) -> Result<Vec<LoadedVersion>> {

        // This happen if a temporary empty root id has been used.
        if root_version.is_empty() {
            return Err(Error::VersionNotFound { version: String::new() });
        }

        handler.on_event(Event::LoadHierarchy { root_version });

        let mut hierarchy = Vec::new();
        let mut current_name = Some(root_version.to_string());
        let mut unique_names = HashSet::new();

        while let Some(version_name) = current_name.take() {
            
            if !unique_names.insert(version_name.clone()) {
                return Err(Error::HierarchyLoop { version: version_name });
            }

            let version = self.load_version(handler, version_name)?;
            if let Some(next_name) = &version.metadata.inherits_from {
                current_name = Some(next_name.clone());
            }
            hierarchy.push(version);

        }

        handler.on_event(Event::LoadedHierarchy { hierarchy: &hierarchy });

        Ok(hierarchy)

    }

    /// Internal function that loads a version from its JSON metadata file.
    fn load_version(&self, 
        handler: &mut dyn Handler, 
        version: String,
    ) -> Result<LoadedVersion> {

        if version.is_empty() {
            return Err(Error::VersionNotFound { version: String::new() });
        }

        let dir = self.versions_dir.join(&version);
        let file = dir.join_with_extension(&version, "json");

        handler.on_event(Event::LoadVersion { version: &version, file: &file });

        // Try a second time if retry is requested...
        for i in 0..2 {

            let reader = match File::open(&file) {
                Ok(reader) => BufReader::new(reader),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    let mut retry = false;
                    if i == 0 {
                        handler.on_event(Event::NeedVersion { version: &version, file: &file, retry: &mut retry });
                    }
                    if retry {
                        continue;
                    } else {
                        break;
                    }
                }
                Err(e) => return Err(Error::new_io_file(e, &file))
            };

            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            let metadata = serde_path_to_error::deserialize::<_, Box<serde::VersionMetadata>>(&mut deserializer)
                .map_err(|e| Error::new_json_file(e, &file))?;

            handler.on_event(Event::LoadedVersion { version: &version, file: &file });

            return Ok(LoadedVersion { name: version, dir, metadata });

        }

        // If not retried, we return a version not found error.
        Err(Error::VersionNotFound { version })

    }

    /// Load the entry point version JAR file.
    fn load_client(&self, 
        handler: &mut dyn Handler, 
        hierarchy: &[LoadedVersion], 
        batch: &mut Batch,
    ) -> Result<PathBuf> {
        
        let root_version = &hierarchy[0];
        let file = root_version.dir.join_with_extension(&root_version.name, "jar");

        handler.on_event(Event::LoadClient);

        let dl = hierarchy.iter()
            .filter_map(|version| version.metadata.downloads.get("client"))
            .next();

        if let Some(dl) = dl {
            let check_client_sha1 = dl.sha1.as_deref().filter(|_| self.strict_libraries_check);
            if !check_file(&file, dl.size, check_client_sha1)? {
                batch.push(dl.url.clone(), file.clone())
                    .set_expected_size(dl.size)
                    .set_expected_sha1(dl.sha1.as_deref().copied());
            }
        } else if !file.is_file() {
            return Err(Error::ClientNotFound {  });
        }

        handler.on_event(Event::LoadedClient { file: &file });
        
        Ok(file)

    }

    /// Load libraries required to run the game.
    fn load_libraries(&self,
        handler: &mut dyn Handler,
        hierarchy: &[LoadedVersion], 
        features: &HashSet<String>,
        batch: &mut Batch,
    ) -> Result<LibrariesFiles> {

        let client_file = self.load_client(&mut *handler, &hierarchy, &mut *batch)?;

        handler.on_event(Event::LoadLibraries);

        // Tracking libraries that are already defined and should not be overridden.
        let mut libraries_set = HashSet::new();
        let mut libraries = Vec::new();

        for version in hierarchy {

            for lib in &version.metadata.libraries {

                let mut lib_gav = lib.name.clone();

                if let Some(lib_natives) = &lib.natives {
                    
                    // Same reason as below.
                    let (Some(os_name), Some(os_bits)) = (os_name(), os_bits()) else {
                        continue;
                    };

                    // If natives object is present, the classifier associated to the
                    // OS overrides the library specifier classifier. If not existing,
                    // we just skip this library because natives are missing.
                    let Some(classifier) = lib_natives.get(os_name) else {
                        continue;
                    };

                    // If we find a arch replacement pattern, we must replace it with
                    // the target architecture bit-ness (32, 64).
                    const ARCH_REPLACEMENT_PATTERN: &str = "${arch}";
                    if let Some(pattern_idx) = classifier.find(ARCH_REPLACEMENT_PATTERN) {
                        let mut classifier = classifier.clone();
                        classifier.replace_range(pattern_idx..pattern_idx + ARCH_REPLACEMENT_PATTERN.len(), os_bits);
                        lib_gav.set_classifier(Some(&classifier));
                    } else {
                        lib_gav.set_classifier(Some(&classifier));
                    }

                }

                // Start by applying rules before the actual parsing. Important, we do
                // that after checking natives, so this will override the lib state if
                // rejected, and we still benefit from classifier resolution.
                if let Some(lib_rules) = &lib.rules {
                    if !self.check_rules(lib_rules, features, None) {
                        continue;
                    }
                }

                // Clone the spec with wildcard for version because we shouldn't override
                // if any of the group/artifact/classifier/extension are matching.
                let mut lib_gav_wildcard = lib_gav.clone();
                lib_gav_wildcard.set_version("*");
                if !libraries_set.insert(lib_gav_wildcard) {
                    continue;
                }

                libraries.push(LoadedLibrary {
                    name: lib_gav,
                    path: None,
                    download: None,
                    natives: lib.natives.is_some(),
                });

                let lib_obj = libraries.last_mut().unwrap();

                let lib_dl;
                if lib_obj.natives {
                    // Unwrap because as seen above, if there are native with define a
                    // classifier on the GAV.
                    lib_dl = lib.downloads.classifiers.get(lib_obj.name.classifier().unwrap());
                } else {
                    lib_dl = lib.downloads.artifact.as_ref();
                }

                if let Some(lib_dl) = lib_dl {
                    lib_obj.path = lib_dl.path.as_ref().map(PathBuf::from);
                    lib_obj.download = Some(LibraryDownload {
                        url: lib_dl.download.url.to_string(),
                        size: lib_dl.download.size,
                        sha1: lib_dl.download.sha1.as_deref().copied(),
                    });
                } else if let Some(repo_url) = &lib.url {
                    
                    // If we don't have any download information, it's possible to use
                    // the 'url', which is the base URL of a maven repository, that we
                    // can derive with the library name to find a URL.

                    let mut url = repo_url.clone();
                    if !url.ends_with('/') {
                        url.push('/');
                    }
                    write!(url, "{}", lib_obj.name.url()).unwrap();

                    lib_obj.download = Some(LibraryDownload {
                        url,
                        size: None,
                        sha1: None,
                    });

                }

                // Additional check because libraries with empty URLs have been seen in
                // the wild, so we remove the source if its URL is empty.
                if let Some(lib_source) = &lib_obj.download {
                    if lib_source.url.is_empty() {
                        lib_obj.download = None;
                    }
                }

            }

        }

        handler.on_event(Event::FilterLibraries { libraries: &mut libraries });
        handler.on_event(Event::LoadedLibraries { libraries: &libraries });

        // Old versions seems to prefer having the main class first in class path, so by
        // default here we put it first, but it may be modified by later versions.
        let mut lib_files = LibrariesFiles::default();
        lib_files.class_files.push(client_file);

        // After possible filtering by event handler, verify libraries and download 
        // missing ones.
        for lib in libraries {

            // Construct the library path depending on its presence.
            let lib_file = if let Some(lib_rel_path) = lib.path.as_deref() {
                // FIXME: Insecure path joining.
                self.libraries_dir.join(lib_rel_path)
            } else {
                lib.name.file(&self.libraries_dir)
            };

            // If no repository URL is given, no more download method is available,
            // so if the JAR file isn't installed, the game cannot be launched.
            // 
            // Note: In the past, we used to default the url to Mojang's maven 
            // repository, but this was a bad habit because most libraries could
            // not be downloaded from their repository, and this was confusing to
            // get a download error for such libraries.
            if let Some(download) = lib.download {
                // Only check SHA-1 if strict checking is enabled.
                let check_source_sha1 = download.sha1.as_ref().filter(|_| self.strict_libraries_check);
                if !check_file(&lib_file, download.size, check_source_sha1)? {
                    batch.push(download.url, lib_file.clone())
                        .set_expected_size(download.size)
                        .set_expected_sha1(download.sha1);
                }
            } else if !lib_file.is_file() {
                return Err(Error::LibraryNotFound { name: lib.name })
            }

            (if lib.natives { 
                &mut lib_files.natives_files 
            } else { 
                &mut lib_files.class_files 
            }).push(lib_file);

        }

        handler.on_event(Event::FilterLibrariesFiles { 
            class_files: &mut lib_files.class_files, 
            natives_files: &mut lib_files.natives_files });
        handler.on_event(Event::LoadedLibrariesFiles { 
            class_files: &lib_files.class_files, 
            natives_files: &lib_files.natives_files });

        Ok(lib_files)

    }

    /// Finalize libraries after download by making every path canonicalized, then 
    /// computing the unique UUID of all the lib files (just by hashing their file 
    /// names) in order to construct a bin (natives) directory unique to these files.
    /// All natives files are then extracted or copied into this binary directory
    /// and it is returned by this function.
    fn finalize_libraries(&self,
        handler: &mut dyn Handler,
        lib_files: &mut LibrariesFiles
    ) -> Result<PathBuf> {

        let mut hash_buf = Vec::new();

        // We know that everything has been downloaded and so we canonicalize in place.
        for file in &mut lib_files.class_files {
            *file = canonicalize_file(file)?;
            hash_buf.extend_from_slice(file.as_os_str().as_encoded_bytes());
        }
        
        for file in &mut lib_files.natives_files {
            *file = canonicalize_file(file)?;
            hash_buf.extend_from_slice(file.as_os_str().as_encoded_bytes());
        }

        // We place the root id as prefix for clarity, even if we can theoretically
        // have multiple bin dir for the same version, if libraries change.
        let bin_uuid = Uuid::new_v5(&UUID_NAMESPACE, &hash_buf);
        let bin_dir = self.bin_dir.join(&self.version)
            .appended(format!("-{}", bin_uuid.hyphenated()));

        // Create the directory and then canonicalize it.
        fs::create_dir_all(&bin_dir)
            .map_err(|e| Error::new_io(e, format!("create dir: {}", bin_dir.display())))?;

        // Now we extract all binaries.
        for src_file in &lib_files.natives_files {
            
            let ext = src_file.extension()
                .map(OsStr::as_encoded_bytes)
                .unwrap_or_default();

            match ext {
                b"zip" | b"jar" => {

                    let src_reader = File::open(src_file)
                        .map_err(|e| Error::new_io_file(e, src_file))
                        .map(BufReader::new)?;

                    let mut archive = ZipArchive::new(src_reader)
                        .map_err(|e| Error::new_zip_file(e, src_file))?;
                    
                    for i in 0..archive.len() {
                        
                        let mut file = archive.by_index(i).unwrap();
                        let Some(file_path) = file.enclosed_name() else {
                            continue;
                        };
                        let Some(file_ext) = file_path.extension() else {
                            continue;
                        };

                        if !matches!(file_ext.as_encoded_bytes(), b"so" | b"dll" | b"dylib") {
                            continue;
                        }

                        // Unwrapping because file should have a name if it has extension.
                        let file_name = file_path.file_name().unwrap();
                        let dst_file = bin_dir.join(file_name);

                        let mut dst_writer = File::create(&dst_file)
                            .map_err(|e| Error::new_io_file(e, &dst_file))?;

                        io::copy(&mut file, &mut dst_writer)
                            .map_err(|e| Error::new_io(e, format!("extract: {}, from: {}, to: {}", 
                                file.name(),
                                src_file.display(),
                                dst_file.display())))?;

                    }

                }
                _ => {

                    // Here we just copy the file, if it happens to be a .so file we 
                    // elide the version number (.so.1.2.3).

                    let Some(mut file_name) = src_file.file_name() else {
                        continue;
                    };

                    // Right find a 'so' extension...
                    let file_name_bytes = file_name.as_encoded_bytes();
                    let mut file_name_new_len = file_name_bytes.len();
                    for part in file_name_bytes.rsplit(|&n| n == b'.') {
                        
                        // The remaining length can't be zero initially.
                        debug_assert_ne!(file_name_new_len, 0);
                        file_name_new_len -= part.len();
                        if file_name_new_len == 0 {
                            continue;  // This is equivalent to break.
                        }

                        if part == b"so" {
                            // SAFETY: We matched an ASCII extension 'so' after the dot, 
                            // so it's a valid bound where we can cut off the OS string.
                            file_name = unsafe { 
                                OsStr::from_encoded_bytes_unchecked(&file_name_bytes[..file_name_new_len + 2])
                            };
                            break;
                        }

                        file_name_new_len -= 1;  // For the dot.

                    }

                    // Note that 'src_file' has been canonicalized and therefore we have
                    // no issue of relative linking.
                    let dst_file = bin_dir.join(file_name);
                    symlink_or_copy_file(&src_file, &dst_file)?;

                }
            }
            
        }

        handler.on_event(Event::ExtractedBinaries { dir: &bin_dir });

        Ok(bin_dir)

    }

    /// Load libraries required to run the game.
    fn load_logger(&self,
        handler: &mut dyn Handler,
        hierarchy: &[LoadedVersion], 
        batch: &mut Batch,
    ) -> Result<Option<LoggerConfig>> {

        let config = hierarchy.iter()
            .filter_map(|version| version.metadata.logging.get("client"))
            .next();

        let Some(config) = config else {
            handler.on_event(Event::NoLogger);
            return Ok(None);
        };

        handler.on_event(Event::LoadLogger { id: &config.file.id });

        let file = self.assets_dir
            .join("log_configs")
            .joined(config.file.id.as_str());

        if !check_file(&file, config.file.download.size, config.file.download.sha1.as_deref())? {
            batch.push(config.file.download.url.clone(), file.clone())
                .set_expected_size(config.file.download.size)
                .set_expected_sha1(config.file.download.sha1.as_deref().copied());
        }

        handler.on_event(Event::LoadedLogger { id: &config.file.id });

        Ok(Some(LoggerConfig {
            kind: config.r#type,
            argument: config.argument.clone(),
            file,
        }))

    }

    /// Load and verify all assets of the game.
    fn load_assets(&self, 
        handler: &mut dyn Handler, 
        hierarchy: &[LoadedVersion], 
        batch: &mut Batch,
    ) -> Result<Option<Assets>> {

        /// Internal description of asset information first found in hierarchy.
        #[derive(Debug)]
        struct IndexInfo<'a> {
            download: Option<&'a serde::Download>,
            id: &'a str,
        }

        // We search the first version that provides asset informations, we also support
        // the legacy 'assets' that doesn't have download information.
        let index_info = hierarchy.iter()
            .find_map(|version| {
                if let Some(asset_index) = &version.metadata.asset_index {
                    Some(IndexInfo {
                        download: Some(&asset_index.download),
                        id: &asset_index.id,
                    })
                } else if let Some(asset_id) = &version.metadata.assets {
                    Some(IndexInfo {
                        download: None,
                        id: &asset_id,
                    })
                } else {
                    None
                }
            });

        let Some(index_info) = index_info else {
            handler.on_event(Event::NoAssets);
            return Ok(None);
        };

        handler.on_event(Event::LoadAssets { id: index_info.id });

        // Resolve all used directories and files...
        let indexes_dir = self.assets_dir.join("indexes");
        let index_file = indexes_dir.join_with_extension(index_info.id, "json");

        // All modern version metadata have download information attached to the assets
        // index identifier, we check the file against the download information and then
        // download this single file. If the file has no download info
        let mut index_downloaded = false;
        if let Some(dl) = index_info.download {
            if !check_file(&index_file, dl.size, dl.sha1.as_deref())? {
                download::single(dl.url.clone(), index_file.clone())
                    .set_expected_size(dl.size)
                    .set_expected_sha1(dl.sha1.as_deref().copied())
                    .download((&mut *handler).into_download())?;
                index_downloaded = true;
            }
        }

        // Scoped to release the reader.
        let asset_index = {

            let reader = match File::open(&index_file) {
                Ok(reader) => BufReader::new(reader),
                Err(e) if !index_downloaded && e.kind() == io::ErrorKind::NotFound =>
                    return Err(Error::AssetsNotFound { id: index_info.id.to_owned() }),
                Err(e) => 
                    return Err(Error::new_io_file(e, &index_file))
            };
    
            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            serde_path_to_error::deserialize::<_, serde::AssetIndex>(&mut deserializer)
                .map_err(|e| Error::new_json_file(e, &index_file))?

        };
        
        handler.on_event(Event::LoadedAssets { 
            id: index_info.id, 
            count: asset_index.objects.len(),
        });

        // Now we check assets that needs to be downloaded...
        let objects_dir = self.assets_dir.join("objects");
        let mut asset_file_name = String::new();
        let mut unique_hashes = HashSet::new();

        let mut assets = Assets {
            id: index_info.id.to_string(),
            mapping: None,
        };

        // If any mapping is needed we compute the virtual directory.
        if asset_index.r#virtual || asset_index.map_to_resources {
            assets.mapping = Some(AssetsMapping {
                objects: Vec::new(),
                virtual_dir: self.assets_dir
                    .join("virtual")
                    .joined(assets.id.as_str())
                    .into_boxed_path(),
                resources: asset_index.map_to_resources,
            });
        }

        for (asset_rel_file, asset) in &asset_index.objects {

            asset_file_name.clear();
            for byte in *asset.hash {
                write!(asset_file_name, "{byte:02x}").unwrap();
            }
            
            let asset_hash_prefix = &asset_file_name[0..2];
            let asset_hash_file = objects_dir
                .join(asset_hash_prefix)
                .joined(asset_file_name.as_str());

            // Save the association of asset path to the actual hash file, only do
            // that if we need it because of virtual or resource mapped assets.
            if let Some(mapping) = &mut assets.mapping {
                mapping.objects.push(AssetObject {
                    rel_file: PathBuf::from(asset_rel_file).into_boxed_path(),
                    object_file: asset_hash_file.clone().into_boxed_path(),
                    size: asset.size,
                });
            }

            // Some assets are represented with multiple files, but we don't 
            // want to download a file multiple time so we abort here.
            if !unique_hashes.insert(&*asset.hash) {
                continue;
            }

            // Only check SHA-1 if strict checking.
            let check_asset_sha1 = self.strict_assets_check.then_some(&*asset.hash);
            if !check_file(&asset_hash_file, Some(asset.size), check_asset_sha1)? {
                batch.push(format!("{RESOURCES_URL}{asset_hash_prefix}/{asset_file_name}"), asset_hash_file)
                    .set_expected_size(Some(asset.size))
                    .set_expected_sha1(Some(*asset.hash));
            }

        }

        handler.on_event(Event::VerifiedAssets { 
            id: index_info.id, 
            count: asset_index.objects.len(),
        });

        Ok(Some(assets))

    }

    /// Finalize assets linking in case of virtual or resources mapping.
    fn finalize_assets(&self, assets: &Assets) -> Result<()> {

        // If the mapping is resource or virtual then we start by copying assets to
        // their virtual directory. We are using hard link because it's way cheaper
        // than copying and save storage.
        let Some(mapping) = &assets.mapping else {
            return Ok(());
        };

        // Important note: pre-1.6 versions (more exactly 13w23b and before) are altering
        // the resources directory on their own, downloading resources that don't match
        // the metadata returned by http://s3.amazonaws.com/MinecraftResources/ (this
        // URL no longer works, but can be fixed using proxies). This means that:
        //
        // - We should copy the resources again and again before each launch and let the
        //   game modify them if needed, therefore no hard/sym link to the virtual dir.
        //
        // - Running the installer for a pre-1.6 version in the same work dir as another
        //   running pre-1.6 version will overwrite the modified resources and therefore
        //   the running version may read the wrong assets for a short time (until the
        //   installed version is run), and if the two versions are different then both
        //   versions will download different things. There is also a potential issue if 
        //   the installer wants to overwrite a resource while it is also being modified
        //   at the same time by the running instance.
        let resources_dir = mapping.resources
            .then(|| self.mc_dir.join("resources"));

        // Hard link each asset into its virtual directory.
        for object in &mapping.objects {
            
            let virtual_file = mapping.virtual_dir.join(&object.rel_file);
            if let Some(parent_dir) = virtual_file.parent() {
                fs::create_dir_all(parent_dir)
                    .map_err(|e| Error::new_io(e, format!("create dir: {}", parent_dir.display())))?;
            }
            hard_link_file(&object.object_file, &virtual_file)?;

            // We copy each resource, if not matching (size only).
            if let Some(resources_dir) = &resources_dir {

                let resource_file = resources_dir.join(&object.rel_file);
                if !check_file(&resource_file, Some(object.size), None)? {
                    
                    if let Some(parent_dir) = resource_file.parent() {
                        fs::create_dir_all(parent_dir)
                            .map_err(|e| Error::new_io(e, format!("create dir: {}", parent_dir.display())))?;
                    }

                    fs::copy(&object.object_file, &resource_file)
                        .map_err(|e| Error::new_io(e, format!("copy: {}, to: {}",
                            object.object_file.display(),
                            resource_file.display())))?;

                }

            }

        }

        Ok(())

    }
    
    /// The goal of this step is to find a valid JVM to run the game on.
    fn load_jvm(&self, 
        handler: &mut dyn Handler, 
        hierarchy: &[LoadedVersion], 
        batch: &mut Batch,
    ) -> Result<Jvm> {

        let version = hierarchy.iter()
            .find_map(|version| version.metadata.java_version.as_ref());

        let major_version = version
            .map(|v| v.major_version)
            .unwrap_or(8);  // Default to Java 8 if not specified.

        // If there is not distribution we try to use a well-known one.
        let distribution = version
            .and_then(|v| v.component.as_deref())
            .or_else(|| Some(match major_version {
                8 => "jre-legacy",
                16 => "java-runtime-alpha",
                17 => "java-runtime-gamma",
                21 => "java-runtime-delta",
                _ => return None
            }));
        
        handler.on_event(Event::LoadJvm { major_version });

        // We simplify the code with this condition and duplicated match, because in the
        // 'else' case we can simplify any policy that contains Mojang and System to
        // System, because we don't have instructions for finding Mojang version.
        let jvm = if let Some(distribution) = distribution {
            match self.jvm_policy {
                JvmPolicy::Static(ref file) => 
                    Some(self.load_static_jvm(handler, &file, major_version)?),
                JvmPolicy::System => 
                    self.load_system_jvm(handler, major_version)?,
                JvmPolicy::Mojang => 
                    self.load_mojang_jvm(handler, distribution, batch)?,
                JvmPolicy::SystemThenMojang => {
                    let mut jvm = self.load_system_jvm(handler, major_version)?;
                    if jvm.is_none() {
                        jvm = self.load_mojang_jvm(handler, distribution, batch)?;
                    }
                    jvm
                }
                JvmPolicy::MojangThenSystem => {
                    let mut jvm = self.load_mojang_jvm(handler, distribution, batch)?;
                    if jvm.is_none() {
                        jvm = self.load_system_jvm(handler, major_version)?;
                    }
                    jvm
                }
            }
        } else {
            match self.jvm_policy {
                JvmPolicy::Static(ref file) => 
                    Some(self.load_static_jvm(handler, &file, major_version)?),
                JvmPolicy::System | 
                JvmPolicy::SystemThenMojang | 
                JvmPolicy::MojangThenSystem => 
                    self.load_system_jvm(handler, major_version)?,
                JvmPolicy::Mojang => None,
            }
        };

        let Some(jvm) = jvm else {
            return Err(Error::JvmNotFound { major_version });
        };

        let version = jvm.version.as_ref()
            .map(|v| v.full.as_str());
        
        let compatible = jvm.version.as_ref()
            .map(|v| v.major_compatibility.is_some())
            .unwrap_or(false);

        handler.on_event(Event::LoadedJvm { 
            file: &jvm.file, 
            version, 
            compatible,
        });

        Ok(jvm)

    }

    /// Load the JVM by checking its version,
    fn load_static_jvm(&self,
        _handler: &mut dyn Handler,
        file: &Path,
        major_version: u32,
    ) -> Result<Jvm> {

        let mut jvm = Jvm {
            file: file.to_path_buf(),
            version: None,
            mojang: None,
        };

        self.find_jvm_versions(std::slice::from_mut(&mut jvm), major_version);
        Ok(jvm)

    }

    /// Try to find a JVM executable installed on the system in standard paths, depending
    /// on the OS.
    fn load_system_jvm(&self,
        handler: &mut dyn Handler,
        major_version: u32,
    ) -> Result<Option<Jvm>> {

        let mut candidates = IndexSet::new();
        let exec_name = jvm_exec_name();

        // Check every JVM available in PATH.
        if let Some(path) = env::var_os("PATH") {
            for mut path in env::split_paths(&path) {
                path.push(exec_name);
                if path.is_file() {
                    candidates.insert(path);
                }
            }
        }

        // On Linux distributions the different JVMs are in '/usr/lib/jvm/'.
        #[cfg(target_os = "linux")] {
            if let Ok(read_dir) = fs::read_dir("/usr/lib/jvm/") {
                for entry in read_dir {
                    let Ok(entry) = entry else { continue };
                    let path = entry.path()
                        .joined("bin")
                        .joined(exec_name);
                    if path.is_file() {
                        candidates.insert(path);
                    }
                }
            }
        }

        // On windows we can search in registry.
        #[cfg(windows)] {

            const REG_PATHS: [&str; 4] = [
                "SOFTWARE\\JavaSoft\\Java Development Kit",
                "SOFTWARE\\JavaSoft\\Java Runtime Environment",
                "SOFTWARE\\JavaSoft\\JDK",
                "SOFTWARE\\JavaSoft\\JRE",
            ];

            // Here we silently ignore any error.
            for path in REG_PATHS {
                let Ok(key) = windows_registry::LOCAL_MACHINE.open(path) else { continue };
                let Ok(keys) = key.keys() else { continue };
                for sub_key in keys {
                    let Ok(sub_key) = key.open(&sub_key) else { continue };
                    let Ok(java_home) = sub_key.get_string("JavaHome") else { continue };
                    let path = PathBuf::from(java_home)
                        .joined("bin")
                        .joined(exec_name);
                    if path.is_file() {
                        candidates.insert(path);
                    }
                }
            }

        }

        // Convert unique file paths to JVM, to be fed to JVM versions.
        let mut jvms = candidates.into_iter().map(|file| Jvm {
            file,
            version: None,
            mojang: None,
        }).collect::<Vec<_>>();

        self.find_jvm_versions(&mut jvms, major_version);

        let mut min_score_jvm = None;
        for jvm in jvms {

            let Some(version) = &jvm.version else { continue };

            let Some(score) = version.major_compatibility else {
                handler.on_event(Event::FoundJvmSystemVersion { 
                    file: &jvm.file, 
                    version: &version.full, 
                    compatible: false,
                });
                continue;
            };

            handler.on_event(Event::FoundJvmSystemVersion { 
                file: &jvm.file, 
                version: &version.full, 
                compatible: true,
            });

            // Don't replace the min score JVM if we are greater or equal.
            if let Some((_, min_score)) = min_score_jvm {
                if min_score <= score {
                    continue;
                }
            }

            min_score_jvm = Some((jvm, score));

        }

        Ok(min_score_jvm.map(|(jvm, _score)| jvm))

    }

    fn load_mojang_jvm(&self,
        handler: &mut dyn Handler,
        distribution: &str,
        batch: &mut Batch,
    ) -> Result<Option<Jvm>> {

        // On Linux, only glibc dynamic linkage is supported by Mojang-provided JVMs.
        if cfg!(target_os = "linux") && cfg!(target_feature = "crt-static") {
            handler.on_event(Event::WarnJvmUnsupportedDynamicCrt);
            return Ok(None);
        }

        // If we don't have JVM platform this means that we can't load Mojang JVM.
        let Some(jvm_platform) = mojang_jvm_platform() else {
            handler.on_event(Event::WarnJvmUnsupportedPlatform);
            return Ok(None);
        };

        // Start by ensuring that we have a cached version of the JVM meta-manifest.
        let meta_manifest = {

            let mut entry = download::single_cached(JVM_META_MANIFEST_URL)
                .set_keep_open()
                .download((&mut *handler).into_download())?;

            let reader = BufReader::new(entry.take_handle().unwrap());
            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            serde_path_to_error::deserialize::<_, serde::JvmMetaManifest>(&mut deserializer)
                .map_err(|e| Error::new_json_file(e, entry.file()))?

        };

        let Some(meta_platform) = meta_manifest.platforms.get(jvm_platform) else {
            handler.on_event(Event::WarnJvmUnsupportedPlatform);
            return Ok(None);
        };

        let Some(meta_distribution) = meta_platform.distributions.get(distribution) else {
            handler.on_event(Event::WarnJvmMissingDistribution);
            return Ok(None);
        };

        // We take the first variant for now.
        let Some(meta_variant) = meta_distribution.variants.get(0) else {
            handler.on_event(Event::WarnJvmMissingDistribution);
            return Ok(None);
        };

        let dir = self.jvm_dir.join(distribution);
        let manifest_file = self.jvm_dir.join_with_extension(distribution, "json");

        // On macOS the JVM bundle structure is a bit different so different bin path.
        let bin_file = if cfg!(target_os = "macos") {
            dir.join("jre.bundle/Contents/Home/bin/java")
        } else {
            dir.join("bin").joined(jvm_exec_name())
        };

        // Check the manifest, download it, read and parse it...
        let manifest = {
            
            if !check_file(&manifest_file, meta_variant.manifest.size, meta_variant.manifest.sha1.as_deref())? {
                download::single(meta_variant.manifest.url.clone(), manifest_file.clone())
                    .set_expected_size(meta_variant.manifest.size)
                    .set_expected_sha1(meta_variant.manifest.sha1.as_deref().copied())
                    .set_keep_open()
                    .download((&mut *handler).into_download())?;
            }
            
            let reader = File::open(&manifest_file)
                .map_err(|e| Error::new_io_file(e, &manifest_file))
                .map(BufReader::new)?;

            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            serde_path_to_error::deserialize::<_, serde::JvmManifest>(&mut deserializer)
                .map_err(|e| Error::new_json_file(e, &manifest_file))?

        };

        let mut mojang_jvm = MojangJvm::default();
        
        // Here we only check files because it's too early to assert symlinks.
        for (rel_file, manifest_file) in &manifest.files {

            // NOTE: We could optimize this repeated allocation, maybe.
            let rel_file = Path::new(rel_file);
            let file = dir.join(rel_file);

            match manifest_file {
                serde::JvmManifestFile::Directory => {
                    fs::create_dir_all(&file)
                        .map_err(|e| Error::new_io(e, format!("create dir: {}", file.display())))?;
                }
                serde::JvmManifestFile::File { 
                    executable, 
                    downloads 
                } => {

                    if *executable {
                        mojang_jvm.executables.push(file.clone().into_boxed_path());
                    }
                    
                    let dl = &downloads.raw;
                    
                    // Only check SHA-1 if strict checking is enabled.
                    let check_dl_sha1 = dl.sha1.as_deref().filter(|_| self.strict_jvm_check);
                    if !check_file(&file, dl.size, check_dl_sha1)? {
                        batch.push(dl.url.clone(), file)
                            .set_expected_size(dl.size)
                            .set_expected_sha1(dl.sha1.as_deref().copied());
                    }

                }
                serde::JvmManifestFile::Link { 
                    target
                } => {
                    mojang_jvm.links.push(MojangJvmLink {
                        file: file.into_boxed_path(),
                        target_file: PathBuf::from(target).into_boxed_path(),
                    });
                }
            }

        }

        Ok(Some(Jvm {
            file: bin_file,
            version: Some(JvmVersion {
                full: meta_variant.version.name.clone(),
                major_compatibility: Some(0),  // Likely perfect compact
            }),
            mojang: Some(mojang_jvm),
        }))

    }

    /// Given a slice of multiple JVMs, update their detected version when possible, by
    /// executing '-version' flag command.
    fn find_jvm_versions(&self, jvms: &mut [Jvm], major_version: u32) {

        // We put the resulting JVM inside this vector so that we have the same
        // ordering as the given exec files.
        let mut children = Vec::new();
        let mut remaining = 0usize;

        // The standard doc says that -version outputs version on stderr. This
        // argument -version is also practical because the version is given between
        // double quotes.
        for jvm in jvms.iter_mut() {
            
            let child = Command::new(&jvm.file)
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .ok();
            
            if child.is_some() {
                remaining += 1;
            }

            children.push(child);

        }

        const TRIES_COUNT: usize = 30;  // 3 second maximum.
        const TRIES_SLEEP: Duration = Duration::from_millis(100);
        
        for _ in 0..TRIES_COUNT {

            for (child_idx, child_opt) in children.iter_mut().enumerate() {

                let Some(child) = child_opt else { continue };
                let Ok(status) = child.try_wait() else {
                    // If an error happens we just forget the child: don't check it again.
                    let _ = child.kill();
                    *child_opt = None;
                    remaining -= 1;
                    continue;
                };

                // If child has terminated, we take child to not check it again.
                let Some(status) = status else { continue };
                let child = child_opt.take().unwrap();
                remaining -= 1;
                
                // Not a success, just forget this child.
                if !status.success() {
                    continue;
                }

                // If successful, get the output (it should not error nor block)...
                let output = child.wait_with_output().unwrap();
                let Ok(output) = String::from_utf8(output.stderr) else { 
                    continue; // Ignore if stderr is not UTF-8.
                };

                jvms[child_idx].version = output.lines()
                    .filter_map(|line| line.split_once('"'))
                    .filter_map(|(_, line)| line.split_once('"'))
                    .map(|(version, _)| version)
                    .next()
                    .and_then(|version| {
                        
                        let actual_major_version = parse_jvm_major_version(version)?;

                        Some(JvmVersion {
                            full: version.to_string(),
                            major_compatibility: calc_jvm_major_compatibility(major_version, actual_major_version),
                        })

                    });
                
            }

            if remaining == 0 {
                break;
            }

            thread::sleep(TRIES_SLEEP);

        }

    }

    /// Finalize the setup of any Mojang-provided JVM, doing nothing if not Mojang.
    fn finalize_jvm(&self, jvm: &Jvm) -> Result<()> {

        let Some(mojang_jvm) = &jvm.mojang else {
            return Ok(());
        };

        // This is only relevant on unix where we can set executable mode
        #[cfg(unix)]
        for exec_file in &mojang_jvm.executables {

            use std::os::unix::fs::PermissionsExt;

            let mut perm = exec_file.metadata()
                .map_err(|e| Error::new_io_file(e, &exec_file))?
                .permissions();

            // Set executable permission for every owner/group/other with read access.
            let mode = perm.mode();
            let new_mode = mode | ((mode & 0o444) >> 2);
            if new_mode != mode {
                
                perm.set_mode(new_mode);
                fs::set_permissions(exec_file, perm)
                    .map_err(|e| Error::new_io(e, format!("set permissions: {}", exec_file.display())))?;

            }
            
        }

        // On Unix we simply use a symlink, on other systems (Windows) we hard link,
        // this act like a copy but is way cheaper.
        for link in &mojang_jvm.links {
            link_file(&link.target_file, &link.file)?;
        }

        Ok(())

    }

    /// Resolve metadata game arguments, checking for rules when needed.
    fn check_args(&self,
        dest: &mut Vec<String>,
        args: &[serde::VersionArgument],
        features: &HashSet<String>,
        mut all_features: Option<&mut HashSet<String>>,
    ) {

        for arg in args {
                    
            // If the argument is conditional then we check rule.
            if let serde::VersionArgument::Conditional(cond) = arg {
                if let Some(rules) = &cond.rules {
                    if !self.check_rules(rules, features, all_features.as_deref_mut()) {
                        continue;
                    }
                }
            }

            match arg {
                serde::VersionArgument::Raw(val) => dest.push(val.clone()),
                serde::VersionArgument::Conditional(cond) => 
                    match &cond.value {
                        serde::SingleOrVec::Single(val) => dest.push(val.clone()),
                        serde::SingleOrVec::Vec(vals) => dest.extend_from_slice(&vals),
                    },
            }

        }

    }

    /// Resolve the given JSON array as rules and return true if allowed.
    fn check_rules(&self,
        rules: &[serde::Rule],
        features: &HashSet<String>,
        mut all_features: Option<&mut HashSet<String>>,
    ) -> bool {

        // Initially disallowed...
        let mut allowed = false;

        for rule in rules {
            // NOTE: Diverge from what have been done in the Python module for long, we
            // no longer early return on disallow.
            match self.check_rule(rule, features, all_features.as_deref_mut()) {
                Some(serde::RuleAction::Allow) => allowed = true,
                Some(serde::RuleAction::Disallow) => allowed = false,
                None => (),
            }
        }

        allowed

    }

    /// Resolve a single rule JSON object and return action if the rule passes. This 
    /// function accepts a set of all features that will be filled with all features
    /// that are checked, accepted or not.
    /// 
    /// This function may return unexpected schema error.
    fn check_rule(&self, 
        rule: &serde::Rule, 
        features: &HashSet<String>, 
        mut all_features: Option<&mut HashSet<String>>
    ) -> Option<serde::RuleAction> {

        if !self.check_rule_os(&rule.os) {
            return None;
        }

        for (feature, feature_expected) in &rule.features {

            // Only check if still valid...
            if features.contains(feature) != *feature_expected {
                return None;
            }
            
            if let Some(all_features) = all_features.as_deref_mut() {
                all_features.insert(feature.clone());
            }

        }

        Some(rule.action)

    }

    /// Resolve OS rules JSON object and return true if the OS is matching the rule.
    /// 
    /// This function may return an unexpected schema error.
    fn check_rule_os(&self, rule_os: &serde::RuleOs) -> bool {

        if let (Some(name), Some(os_name)) = (&rule_os.name, os_name()) {
            if name != os_name {
                return false;
            }
        }

        if let (Some(arch), Some(os_arch)) = (&rule_os.arch, os_arch()) {
            if arch != os_arch {
                return false;
            }
        }

        if let (Some(version), Some(os_version)) = (&rule_os.version, os_version()) {
            if !version.is_match(os_version) {
                return false;
            }
        }

        true

    }

}

/// Events happening when installing.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// Filter the features that will be later used to filter rules using them.
    FilterFeatures { features: &'a mut HashSet<String> },
    /// Notification of all features that have been selected after filtering.
    LoadedFeatures { features: &'a HashSet<String> },
    /// The version hierarchy will be loaded, starting from the given root version.
    LoadHierarchy { root_version: &'a str },
    /// The given version hierarchy has been successfully loaded.
    LoadedHierarchy { hierarchy: &'a [LoadedVersion] },
    /// A version will be loaded, at this point you can check the file for its 
    /// validity, and delete it if relevant, in this case [`Self::need_version`]
    /// is called just after to possibly install the version metadata.
    LoadVersion { version: &'a str, file: &'a Path },
    /// This event is called if the given version is missing a metadata file, in this
    /// case its path is given and this handler has the possibility of installing it
    /// before retrying. If the handler actually wants the loading to be retried after
    /// it as handled it, it should return true.
    NeedVersion { version: &'a str, file: &'a Path, retry: &'a mut bool },
    /// The given version in the hierarchy has been successfully loaded, the metadata
    /// file path is also given.
    LoadedVersion { version: &'a str, file: &'a Path },
    /// The client JAR file will be loaded.
    LoadClient,
    /// The client JAR file has been loaded successfully at the given path.
    LoadedClient { file: &'a Path },
    /// The game required libraries are going to be loaded.
    LoadLibraries,
    /// Filter versions before their verification.
    FilterLibraries { libraries: &'a mut Vec<LoadedLibrary> },
    /// Libraries have been loaded. After that, the libraries will be verified and 
    /// added to the downloads list if missing.
    LoadedLibraries { libraries: &'a [LoadedLibrary] },
    /// Libraries have been verified, the class files includes the client JAR file as 
    /// first path in the vector. Note that all paths will be canonicalized, 
    /// relatively to the current process' working dir, before being added to the 
    /// command line, so the files must exists.
    FilterLibrariesFiles { class_files: &'a mut Vec<PathBuf>, natives_files: &'a mut Vec<PathBuf> },
    /// The final version of class and natives files has been loaded.
    LoadedLibrariesFiles { class_files: &'a [PathBuf], natives_files: &'a [PathBuf] },
    /// No logger configuration will be loaded because version doesn't specify any.
    NoLogger,
    /// The logger configuration will be loaded.
    LoadLogger { id: &'a str },
    /// Logger configuration has been loaded successfully.
    LoadedLogger { id: &'a str },
    /// Assets will not be loaded because version doesn't specify any.
    NoAssets,
    /// Assets will be loaded.
    LoadAssets { id: &'a str },
    /// Assets have been loaded, and are going to be verified in order to add missing 
    /// ones to the download batch.
    LoadedAssets { id: &'a str, count: usize },
    /// Assets have been verified and missing assets have been added to the download
    /// batch.
    VerifiedAssets { id: &'a str, count: usize },
    /// The JVM will be loaded, depending on the policy configured in the installer. 
    /// The major version that is required is given, when not specified by any
    /// version metadata it defaults to Java 8, because most older versions didn't
    /// specify it.
    LoadJvm { major_version: u32 },
    /// When searching for JVMs in the system standard paths, this event trigger for
    /// each detected JVM executable, and indicates if this version is compatible and
    /// therefore is a potential candidate for being used as the JVM. 
    FoundJvmSystemVersion { file: &'a Path, version: &'a str, compatible: bool },
    /// The system runs on Linux and has C runtime not dynamically linked (static, 
    /// musl for example), suggesting that your system doesn't provide dynamic C 
    /// runtime (glibc), and such JVM are not provided by Mojang. 
    WarnJvmUnsupportedDynamicCrt,
    /// When trying to find a Mojang JVM to install, your operating system and 
    /// architecture are not supported.
    WarnJvmUnsupportedPlatform,
    /// When trying to find a Mojang JVM to install, your operating system and 
    /// architecture are supported but the distribution (the java version packaged and
    /// distributed by Mojang) is not found.
    WarnJvmMissingDistribution,
    /// The JVM has been loaded, if the version is known. The compatible flag 
    /// indicates if this JVM is **likely** compatible with the game version, 
    /// when false it indicates that it will likely be incompatible.
    LoadedJvm { file: &'a Path, version: Option<&'a str>, compatible: bool },
    /// Resources will be downloaded. This function returns a boolean that indicates
    /// if the download should proceed, this can be used to abort 
    DownloadResources { cancel: &'a mut bool },
    /// Resources have been successfully downloaded.
    DownloadedResources,
    /// A download progress forwarded from a download handler.
    DownloadProgress { count: u32, total_count: u32, size: u32, total_size: u32 },
    /// All binaries has been successfully extracted to the given binary directory.
    ExtractedBinaries { dir: &'a Path },
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
pub(crate) trait HandlerInto: Handler + Sized {
    
    #[inline]
    fn into_download(self) -> impl download::Handler {
        pub(crate) struct Adapter<H: Handler>(pub H);
        impl<H: Handler> download::Handler for Adapter<H> {
            fn on_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
                self.0.on_event(Event::DownloadProgress { count, total_count, size, total_size });
            }
        }
        Adapter(self)
    }

}

impl<H: Handler> HandlerInto for H {}


/// The base installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The given version appears twice in the hierarchy, implying an infinite recursion.
    #[error("hierarchy loop: {version}")]
    HierarchyLoop {
        version: String,
    },
    /// The given version is not found when trying to fetch it.
    #[error("version not found: {version}")]
    VersionNotFound {
        version: String,
    },
    /// The given version is not found and no download information is provided.
    #[error("assets not found: {id}")]
    AssetsNotFound {
        id: String,
    },
    /// The version JAR file that is required has no download information and is not 
    /// already existing, is is mandatory to build the class path.
    #[error("client not found")]
    ClientNotFound {  },
    /// A library has no download information and is missing the libraries directory.
    #[error("library not found: {name}")]
    LibraryNotFound {
        name: Gav,
    },
    /// No JVM was found when installing the version, this depends on installer policy.
    #[error("jvm not found")]
    JvmNotFound {
        major_version: u32,
    },
    #[error("main class not found")]
    MainClassNotFound {  },
    /// Returned if the [`Handler::download_resources`] returned false, the installation
    /// procedure can't continue because it needs resources to be downloaded.
    #[error("download resources cancelled")]
    DownloadResourcesCancelled {  },
    /// There are some errors in the given download batch.
    #[error("download: {} errors over {} entries", batch.errors_count(), batch.len())]
    Download {
        batch: download::BatchResult,
    },
    /// A generic error that originates from internal or third-party dependencies. The
    /// goal of this is to provide a backward-compatible error variant that can be 
    /// dynamically checked and downcast if needed, the actual error types are not
    /// guaranteed to be present in future versions. It's associated to an origin 
    /// string that helps knowing the location of the issue.
    /// 
    /// Currently these are the error types that can be produced by PortableMC:
    /// 
    /// - [`std::io::Error`] for any unexpected I/O error type.
    /// 
    /// - [`serde_json::Error`] (or inside a [`serde_path_to_error::Error`]) for any 
    ///   unexpected parsing error.
    /// 
    /// - [`zip::result::ZipError`] for errors related to ZIP extractions.
    /// 
    /// - [`reqwest::Error`] for errors related to HTTP requests.
    #[error("internal: {error} @ {origin}")]
    Internal {
        #[source]
        error: Box<dyn std::error::Error + Send + Sync>,
        origin: Box<str>,
    },
}

impl From<download::BatchResult> for Error {
    fn from(batch: download::BatchResult) -> Self {
        Self::Download { batch }
    }
}

impl From<download::EntryError> for Error {
    fn from(value: download::EntryError) -> Self {
        Self::Download { batch: download::BatchResult::from(value) }
    }
}

/// Type alias for a result with the base error type.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {

    #[inline]
    pub(crate) fn new_io(error: io::Error, origin: impl Into<Box<str>>) -> Self {
        Self::Internal { error: Box::new(error), origin: origin.into() }
    }
    
    #[inline]
    pub(crate) fn new_json(error: serde_path_to_error::Error<serde_json::Error>, origin: impl Into<Box<str>>) -> Self {
        Self::Internal { error: Box::new(error), origin: origin.into() }
    }
    
    #[inline]
    pub(crate) fn new_zip(error: zip::result::ZipError, origin: impl Into<Box<str>>) -> Self {
        Self::Internal { error: Box::new(error), origin: origin.into() }
    }

    #[inline]
    pub(crate) fn new_reqwest(error: reqwest::Error, origin: impl Into<Box<str>>) -> Self {
        Self::Internal { error: Box::new(error), origin: origin.into() }
    }

    #[inline]
    pub(crate) fn new_io_file(error: io::Error, file: impl AsRef<Path>) -> Self {
        Self::new_io(error, file.as_ref().display().to_string())
    }
    
    #[inline]
    pub(crate) fn new_json_file(error: serde_path_to_error::Error<serde_json::Error>, file: impl AsRef<Path>) -> Self {
        Self::new_json(error, file.as_ref().display().to_string())
    }

    #[inline]
    pub(crate) fn new_zip_file(error: zip::result::ZipError, file: impl AsRef<Path>) -> Self {
        Self::new_zip(error, file.as_ref().display().to_string())
    }

}

/// The policy for finding or installing the JVM executable to be used for launching
/// the game.
#[derive(Debug, Clone)]
pub enum JvmPolicy {
    /// The path to the JVM executable is given and will be used.
    Static(PathBuf),
    /// The installer will try to find a suitable JVM executable in the path, searching
    /// a `java` (or `javaw.exe` on Windows) executable. On operating systems where it's
    /// supported, this will also check for known directories (on Arch for example).
    /// If the version needs a specific JVM major version, each candidate executable is 
    /// checked and a warning is triggered to notify that the version is not suited.
    /// Invalid versions are not kept, and if no valid version is found at the end then
    /// a [`Error::JvmNotFound`] error is returned.
    System,
    /// The installer will try to find a suitable JVM to install from Mojang-provided
    /// distributions, if no JVM is available for the platform (`jvm_platform` on the
    /// installer) and for the required distribution then a [`Error::JvmNotFound`] error
    /// is returned.
    Mojang,
    /// The installer search system and then mojang as a fallback.
    SystemThenMojang,
    /// The installer search Mojang and then system as a fallback.
    MojangThenSystem,
}

/// Represent a loaded version.
#[derive(Clone)]
pub struct LoadedVersion {
    /// Name of this version.
    name: String,
    /// Directory of that version, where metadata is stored with the JAR file.
    dir: PathBuf,
    /// The loaded metadata of the version.
    metadata: Box<serde::VersionMetadata>,
}

impl LoadedVersion {

    /// Get the version name.
    /// 
    /// Most game resources call this the "version id", but for consistency and clarity
    /// we decided to go with `name` everywhere in the public interface of the library. 
    /// When it's clear, we just call it "version".
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the version directory, where the metadata and client JAR is stored, this 
    /// directory is named after this version's name.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Return the release channel for this version, if specified in the metadata.
    pub fn channel(&self) -> Option<VersionChannel> {
        self.metadata.r#type.map(VersionChannel::from)
    }

}

impl fmt::Debug for LoadedVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedVersion")
            .field("name", &self.name)
            .field("dir", &self.dir)
            .finish()
    }
}

/// The different release channels for versions. Most of the game versions calls this 
/// the "version type", but for keyword reservation issues with `type` we call this 
/// channel on the public interface, and this is also a good indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionChannel {
    Release,
    Snapshot,
    Beta,
    Alpha,
}

/// Internal conversion from the serde equivalent of this to the public interface enum.
/// Both have the same discriminant values so this should be optimized to just a copy.
impl From<serde::VersionType> for VersionChannel {
    #[inline]
    fn from(value: serde::VersionType) -> Self {
        match value {
            serde::VersionType::Release => Self::Release,
            serde::VersionType::Snapshot => Self::Snapshot,
            serde::VersionType::OldBeta => Self::Beta,
            serde::VersionType::OldAlpha => Self::Alpha,
        }
    }
}

/// Represent a loaded library.
#[derive(Debug, Clone)]
pub struct LoadedLibrary {
    /// GAV for this library.
    pub name: Gav,
    /// The path to install the library at, relative to the libraries directory, if not
    /// specified, it will be derived from the library specifier.
    pub path: Option<PathBuf>,
    /// An optional download information for this library if it is missing.
    pub download: Option<LibraryDownload>,
    /// True if this contains natives that should be extracted into the binaries 
    /// directory before launching the game, instead of being added to the class path.
    pub natives: bool,
}

/// Represent how a library will be downloaded if needed.
#[derive(Debug, Clone)]
pub struct LibraryDownload {
    pub url: String,
    pub size: Option<u32>,
    pub sha1: Option<[u8; 20]>,
}

/// An abstract filter for libraries and their resolved files.
pub trait LibraryFilter {

    /// Filter versions before their verification.
    fn filter_libraries(&self, libraries: &mut Vec<LoadedLibrary>);

    /// Libraries have been verified, the class files includes the client JAR file as 
    /// first path in the vector. Note that all paths will be canonicalized, 
    /// relatively to the current process' working dir, before being added to the 
    /// command line, so the files must exists.
    fn filter_libraries_files(&self, classes_files: &mut Vec<PathBuf>, natives_files: &mut Vec<PathBuf>);

}

impl Debug for dyn LibraryFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dyn LibraryFilter")
    }
}

/// Description of all installed resources needed for running an installed game version.
/// The arguments may contain replacement patterns that will be used when starting the 
/// game.
/// 
/// **Important note:** paths in this structure are all relative to the directories
/// configured in the installer, they are all made absolute before launching the game. 
#[derive(Debug, Clone)]
pub struct Game {
    /// Path to the JVM executable file.
    pub jvm_file: PathBuf,
    /// Working directory where the JVM process should be running.
    pub mc_dir: PathBuf,
    /// The main class that contains the JVM entrypoint.
    pub main_class: String,
    /// List of JVM arguments (before the main class in the command line).
    pub jvm_args: Vec<String>,
    /// List of game arguments (after the main class in the command line).
    pub game_args: Vec<String>,
}

impl Game {

    /// Modify the arguments and check for unresolved variables.
    /// 
    /// Currently internal to the crate, mostly unused.
    pub(crate) fn replace_args<F>(&mut self, mut func: F)
    where
        F: FnMut(&str) -> Option<String>,
    {
        replace_strings_args(&mut self.jvm_args, &mut func);
        replace_strings_args(&mut self.game_args, &mut func);
    }

    /// Create a command to launch the process, this command can be modified if you wish.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.jvm_file);
        command
            .current_dir(&self.mc_dir)
            .args(&self.jvm_args)
            .arg(&self.main_class)
            .args(&self.game_args);
        command
    }

    /// Create a command to launch the process and directly spawn the process.
    pub fn spawn(&self) -> io::Result<Child> {
        self.command().spawn()
    }

    /// Spawn the process and wait for it to finish.
    pub fn spawn_and_wait(&self) -> io::Result<ExitStatus> {
        self.spawn()?.wait()
    }

}

// ========================== //
// Following code is internal //
// ========================== //

/// Internal resolved libraries file paths.
#[derive(Debug, Default)]
struct LibrariesFiles {
    class_files: Vec<PathBuf>,
    natives_files: Vec<PathBuf>,
}

/// Internal resolved logger configuration.
#[derive(Debug)]
struct LoggerConfig {
    #[allow(unused)]
    kind: serde::VersionLoggingType,
    argument: String,
    file: PathBuf,
}

/// Internal resolved assets associating the virtual file path to its hash file path.
#[derive(Debug)]
struct Assets {
    id: String,
    mapping: Option<AssetsMapping>,
}

/// In case of virtual or resources mapped assets, the launcher needs to hard link all
/// asset object files to their virtual relative path inside the assets index's virtual
/// directory. 
/// 
/// - Virtual assets has been used between 13w23b (pre 1.6, excluded) and 13w48b (1.7.2).
/// - Resource mapped assets has been used for versions 13w23b (pre 1.6) and before.
#[derive(Debug)]
struct AssetsMapping {
    /// List of objects to link to virtual dir.
    objects: Vec<AssetObject>,
    /// Path to the virtual directory for the assets id.
    virtual_dir: Box<Path>,
    /// True if a resources directory should link game's working directory to the
    /// assets index' virtual directory.
    resources: bool,
}

/// A single asset object mapping from its relative (virtual) path to the object path.
#[derive(Debug)]
struct AssetObject {
    rel_file: Box<Path>,
    object_file: Box<Path>,
    size: u32,
}

/// Internal resolved JVM.
#[derive(Debug)]
struct Jvm {
    /// The 'java' or 'javaw' executable file.
    file: PathBuf,
    /// The JVM version, if known.
    version: Option<JvmVersion>,
    /// If this JVM originate from a mojang JVM, it contains the post-installation infos.
    mojang: Option<MojangJvm>,
}

/// When a JVM version is known, this contains the information and compatibility score of
/// that JVM.
#[derive(Debug)]
struct JvmVersion {
    /// The full version string.
    full: String,
    /// A compatibility score for the major version of that JVM with the required major
    /// version, none when the version is **likely** incompatible. The lower the score is,
    /// the better the JVM is compatible, zero being the **most likely** compatible JVM.
    major_compatibility: Option<u32>,
}

/// Internal optional to the resolve JVM in case of Mojang-provided JVM where files
/// needs to be made executable and links added.
#[derive(Debug, Default)]
struct MojangJvm {
    /// List of full paths to files that should be executable (relevant under Linux).
    executables: Vec<Box<Path>>,
    /// List of links to add given `(link_file, target_file)`.
    links: Vec<MojangJvmLink>,
}

#[derive(Debug)]
struct MojangJvmLink {
    file: Box<Path>,
    target_file: Box<Path>,
}

/// Check if a file at a given path has the corresponding properties (size and/or SHA-1), 
/// returning true if it is valid, so false is returned anyway if the file doesn't exists.
pub(crate) fn check_file(file: &Path, size: Option<u32>, sha1: Option<&[u8; 20]>) -> Result<bool> {
    check_file_advanced(file, size, sha1, false)
}

/// Check if a file at a given path has the corresponding properties (size and/or SHA-1), 
/// returning true if it is valid, you can choose if a file not found is considered valid
/// or not.
pub(crate) fn check_file_advanced(file: &Path, size: Option<u32>, sha1: Option<&[u8; 20]>, not_found_valid: bool) -> Result<bool> {

    fn inner(file: &Path, size: Option<u32>, sha1: Option<&[u8; 20]>, not_found_valid: bool) -> io::Result<bool> {
    
        if let Some(sha1) = sha1 {
            // If we want to check SHA-1 we need to open the file and compute it...
            match File::open(file) {
                Ok(mut reader) => {
    
                    // If relevant, start by checking the actual size of the file.
                    if let Some(size) = size {
                        let actual_size = reader.seek(SeekFrom::End(0))?;
                        if size as u64 != actual_size {
                            return Ok(false);
                        }
                        reader.seek(SeekFrom::Start(0))?;
                    }
                    
                    // Only after we compute hash...
                    let mut digest = Sha1::new();
                    io::copy(&mut reader, &mut digest)?;
                    if digest.finalize().as_slice() != sha1 {
                        return Ok(false);
                    }
                    
                    Ok(true)
    
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(not_found_valid),
                Err(e) => return Err(e),
            }
        } else {
            match (file.metadata(), size) {
                // File is existing and we want to check size...
                (Ok(metadata), Some(size)) => Ok(metadata.len() == size as u64),
                // File is existing but we don't have size to check, no need to download.
                (Ok(_metadata), None) => Ok(true),
                (Err(e), _) if e.kind() == io::ErrorKind::NotFound => Ok(not_found_valid),
                (Err(e), _) => return Err(e),
            }
        }
    
    }

    inner(file, size, sha1, not_found_valid)
        .map_err(|e| Error::new_io(e, format!("check file: {}", file.display())))

}

/// Apply arguments replacement for each string, explained in [`replace_string_args`].
fn replace_strings_args<'input, F>(ss: &mut [String], mut func: F)
where 
    F: FnMut(&str) -> Option<String>,
{
    for s in ss {
        replace_string_args(s, &mut func);
    }
}

/// Given a string buffer, search for each argument of the form `${arg}`, give its name
/// to the given closure and if some value is returned, replace it by this value.
fn replace_string_args<F>(s: &mut String, mut func: F)
where 
    F: FnMut(&str) -> Option<String>,
{

    // Our cursor means that everything before this index has been already checked.
    let mut cursor = 0;

    while let Some(open_idx) = s[cursor..].find("${") {
        
        let open_idx = cursor + open_idx;
        let Some(close_idx) = s[open_idx + 2..].find('}') else { break };
        let close_idx = open_idx + 2 + close_idx + 1;
        cursor = close_idx;

        if let Some(value) = func(&s[open_idx + 2..close_idx - 1]) {
            
            s.replace_range(open_idx..close_idx, &value);
            
            let repl_len = close_idx - open_idx;
            let repl_diff = value.len() as isize - repl_len as isize;
            cursor = cursor.checked_add_signed(repl_diff).unwrap();

        }

    }

}

/// Parse a JVM major version, this supports pre-v9 versions.
fn parse_jvm_major_version(version: &str) -> Option<u32> {
    
    // Special case for parsing versions such as '8u51'.
    if !version.contains('.') {
        if let Some((major, _patch)) = version.split_once('u') {
            return major.parse::<u32>().ok();
        }
    }

    let mut comp = version.split('.');
    let mut major = comp.next()?.parse::<u32>().ok()?;
    if major == 1 {
        major = comp.next()?.parse::<u32>().ok()?;
    }
    Some(major)

}

/// This function compute the compatibility between a given JVM version and the expected
/// one, returning None if the versions are fully incompatible. If versions are compatible
/// then a score is returned, the less this score is, the higher if the compatibility,
/// a score of zero means that the versions are fully compatible.
fn calc_jvm_major_compatibility(expected_version: u32, version: u32) -> Option<u32> {
    if expected_version <= 8 {
        // Because of huge breakings in the internal APIs between Java 8 and 9 (and onward),
        // we require strict equality for Java 8 and before.
        (expected_version == version).then_some(0)
    } else {
        // After Java 8, we allow any greater JVM version to run, the score is computed
        // to privilege versions that are close to another, thus reducing potential 
        // breakings between version, even if it shouldn't happen.
        if version >= expected_version {
            Some(version - expected_version)
        } else {
            None
        }
    }
}

/// Internal shortcut to canonicalize a file or directory and map error into an 
/// installer error.
#[inline]
pub(crate) fn canonicalize_file(file: &Path) -> Result<PathBuf> {
    dunce::canonicalize(file)
        .map_err(|e| Error::new_io(e, format!("canonicalize: {}", file.display())))
}

/// Internal shortcut to creating a link file that points to another one, this function
/// tries to create a symlink on unix systems and make a hard link on other systems.
/// **Not made for directories linking!**
/// 
/// This function accepts relative path, in case of relative path is refers to the 
/// directory the link resides in, no security check is performed.
/// 
/// This function ignores if the links already exists.
#[inline]
pub(crate) fn link_file(original: &Path, link: &Path) -> Result<()> {

    let err;
    let action;

    #[cfg(unix)] {
        // We just give the relative link with '..' which will be resolved 
        // relative to the link's location by the filesystem.
        err = std::os::unix::fs::symlink(original, link);
        action = "symlink";
    }

    #[cfg(not(unix))] {
        let parent_dir = link.parent().unwrap();
        let file = parent_dir.join(&original);
        err = fs::hard_link(original, &file);
        action = "hard link";
    }

    match err {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(Error::new_io(e, format!("{action}: {}, to: {}", original.display(), link.display()))),
    }

}

#[inline]
pub(crate) fn symlink_or_copy_file(original: &Path, link: &Path) -> Result<()> {

    let err;
    let action;

    #[cfg(unix)] {
        // We just give the relative link with '..' which will be resolved 
        // relative to the link's location by the filesystem.
        err = match std::os::unix::fs::symlink(original, link) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e),
        };
        action = "symlink";
    }

    #[cfg(not(unix))] {
        err = fs::copy(original, link).map(|_| ());
        action = "copy";
    }

    err.map_err(|e| Error::new_io(e, format!("{action}: {}, to: {}", original.display(), link.display())))

}

/// Internal shortcut to hard link files, this can also be used for hard linking
/// directories, if the link already exists the error is ignored.
#[inline]
pub(crate) fn hard_link_file(original: &Path, link: &Path) -> Result<()> {
    match fs::hard_link(original, link) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(Error::new_io(e, format!("hard link: {}, to: {}", original.display(), link.display()))),
    }
}

/// A utility function not used in this module, but used for Fabric and Forge mod loader
/// installers, which needs to manually write the metadata. This function creates any
/// parent directory if missing.
pub(crate) fn write_version_metadata(file: &Path, metadata: &serde::VersionMetadata) -> Result<()> {

    // We unwrap because any version metadata file should be located insane version dir.
    let dir = file.parent().unwrap();
    fs::create_dir_all(dir)
        .map_err(|e| Error::new_io_file(e, dir))?;

    let writer = File::create(file)
        .map_err(|e| Error::new_io_file(e, file))
        .map(BufWriter::new)?;

    let mut serializer = serde_json::Serializer::new(writer);
    serde_path_to_error::serialize(&metadata, &mut serializer)
        .map_err(|e| Error::new_json_file(e, file))?;

    Ok(())

}

/// Return the default main directory for Minecraft, so called ".minecraft".
pub fn default_main_dir() -> Option<&'static Path> {

    static MAIN_DIR: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
        // TODO: Maybe change the main dir to something more standard under Linux.
        if cfg!(target_os = "windows") {
            dirs::data_dir().map(|dir| dir.joined(".minecraft"))
        } else if cfg!(target_os = "macos") {
            dirs::data_dir().map(|dir| dir.joined("minecraft"))
        } else {
            dirs::home_dir().map(|dir| dir.joined(".minecraft"))
        }
    });

    MAIN_DIR.as_deref()
    
}

/// Return the default OS name for rules.
/// Returning none if the OS is not known.
/// 
/// This is currently not dynamic, so this will return the OS name the binary 
/// has been compiled for.
#[inline]
fn os_name() -> Option<&'static str> {
    Some(match env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "osx",
        "freebsd" => "freebsd",
        "openbsd" => "openbsd",
        "netbsd" => "netbsd",
        _ => return None
    })
}

/// Return the default OS system architecture name for rules.
/// 
/// This is currently not dynamic, so this will return the OS architecture the binary
/// has been compiled for.
#[inline]
fn os_arch() -> Option<&'static str> {
    Some(match env::consts::ARCH {
        "x86" => "x86",
        "x86_64" => "x86_64",
        "arm" => "arm32",
        "aarch64" => "arm64",
        _ => return None
    })
}

/// Return the default OS version name for rules.
#[inline]
fn os_bits() -> Option<&'static str> {
    Some(match env::consts::ARCH {
        "x86" | "arm" => "32",
        "x86_64" | "aarch64" => "64",
        _ => return None
    })
}

/// Return the default OS version name for rules.
#[inline]
fn os_version() -> Option<&'static str> {

    static VERSION: LazyLock<Option<String>> = LazyLock::new(|| {
        use os_info::Version;
        match os_info::get().version() {
            Version::Unknown => None,
            version => Some(version.to_string())
        }
    });

    VERSION.as_deref()

}

/// Return the JVM exec file name. 
#[inline]
fn jvm_exec_name() -> &'static str {
    if cfg!(windows) { "javaw.exe" } else { "java" }
}

#[inline]
fn mojang_jvm_platform() -> Option<&'static str> {
    Some(match (env::consts::OS, env::consts::ARCH) {
        ("macos", "x86_64") => "mac-os",
        ("macos", "aarch64") => "mac-os-arm64",
        ("linux", "x86") => "linux-i386",
        ("linux", "x86_64") => "linux",
        ("windows", "x86") => "windows-x86",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        _ => return None
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn replace_string_args() {
        
        use super::replace_string_args;

        let mut buf = "${begin}foo${middle}bar${end}".to_string();
        replace_string_args(&mut buf, |_arg| None);
        assert_eq!(buf, "${begin}foo${middle}bar${end}");
        replace_string_args(&mut buf, |arg| if arg == "middle" { Some(".:.".to_string()) } else { None });
        assert_eq!(buf, "${begin}foo.:.bar${end}");
        replace_string_args(&mut buf, |arg| Some(format!("[  {arg}  ]")));
        assert_eq!(buf, "[  begin  ]foo.:.bar[  end  ]");

    }

    #[test]
    fn parse_jvm_major_version() {

        use super::parse_jvm_major_version;

        assert_eq!(parse_jvm_major_version("7u80"), Some(7));
        assert_eq!(parse_jvm_major_version("8u51"), Some(8));
        assert_eq!(parse_jvm_major_version("17"), Some(17));
        assert_eq!(parse_jvm_major_version("17.0"), Some(17));
        assert_eq!(parse_jvm_major_version("17.0.2"), Some(17));
        assert_eq!(parse_jvm_major_version("1.8.0_111"), Some(8));
        assert_eq!(parse_jvm_major_version("10.0.2"), Some(10));

        // Corner cases
        assert_eq!(parse_jvm_major_version("10.foo"), Some(10));
        assert_eq!(parse_jvm_major_version("1.foo"), None);
        assert_eq!(parse_jvm_major_version("foou51"), None);
        assert_eq!(parse_jvm_major_version("8ufoo"), Some(8));

    }

    #[test]
    fn calc_jvm_major_compatibility() {

        use super::calc_jvm_major_compatibility;
        
        assert_eq!(calc_jvm_major_compatibility(7, 7), Some(0));
        assert_eq!(calc_jvm_major_compatibility(8, 8), Some(0));
        assert_eq!(calc_jvm_major_compatibility(8, 7), None);

        assert_eq!(calc_jvm_major_compatibility(9, 8), None);
        assert_eq!(calc_jvm_major_compatibility(9, 9), Some(0));
        assert_eq!(calc_jvm_major_compatibility(9, 11), Some(2));
        assert_eq!(calc_jvm_major_compatibility(9, 17), Some(8));
        assert_eq!(calc_jvm_major_compatibility(17, 17), Some(0));
        assert_eq!(calc_jvm_major_compatibility(17, 11), None);

    }

}
