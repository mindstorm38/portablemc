//! Standard installation procedure.

pub mod serde;

use std::io::{self, BufReader, Seek, SeekFrom};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fmt::Write as _;
use std::fs::File;

use sha1::{Digest, Sha1};

use crate::path::PathExt;
use crate::gav::Gav;


/// Base URL for downloading game's assets.
const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";

// /// Base URL for downloading game's libraries.
// const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";


/// Standard installer handle to install versions, this object is just the configuration
/// of the installer when a version will be installed, such as directories to install 
/// into, the installation will not mutate this object.
#[derive(Debug)]
pub struct Installer {
    /// The directory where versions are stored.
    pub versions_dir: PathBuf,
    /// The directory where assets, assets index, cached skins and logs config are stored.
    /// TODO: Note on permissions for skins directory...
    pub assets_dir: PathBuf,
    /// The directory where libraries are stored, organized like a maven repository.
    pub libraries_dir: PathBuf,
    /// The working directory from where the game is run, the game stores thing like 
    /// saves, resource packs, options and mods if relevant.
    pub work_dir: PathBuf,
    /// The binary directory contains temporary directories that are used only during the
    /// game's runtime, modern versions no longer use it but it.
    pub bin_dir: PathBuf,
    /// The OS name used when applying rules for the version metadata.
    pub meta_os_name: String,
    /// The OS system architecture name used when applying rules for version metadata.
    pub meta_os_arch: String,
    /// The OS version name used when applying rules for version metadata.
    pub meta_os_version: String,
    /// The OS bits replacement for "${arch}" replacement of library natives.
    pub meta_os_bits: String,
    /// When enabled, all assets are strictly checked against their expected SHA-1,
    /// this is disabled by default because it's heavy on CPU.
    pub strict_assets_checking: bool,
    /// When enabled, all libraries are strictly checked against their expected SHA-1,
    /// this is disabled by default because it's heavy on CPU.
    pub strict_libraries_checking: bool,
}

impl Installer {

    /// Create a new installer with default configuration and pointing to defaults
    /// directories.
    pub fn new() -> Self {
        let dir = default_main_dir().unwrap();
        Self::with_dirs(dir.clone(), dir)
    }

    /// Create a new installer with default configuration and pointing to given 
    /// directories.
    pub fn with_dirs(main_dir: PathBuf, work_dir: PathBuf) -> Self {
        Self {
            versions_dir: main_dir.join("versions"),
            assets_dir: main_dir.join("assets"),
            libraries_dir: main_dir.join("libraries"),
            bin_dir: main_dir.join("bin"), // FIXME:
            work_dir,
            meta_os_name: default_meta_os_name().unwrap(),
            meta_os_arch: default_meta_os_arch().unwrap(),
            meta_os_version: default_meta_os_version().unwrap(),
            meta_os_bits: default_meta_os_bits().unwrap(),
            strict_assets_checking: false,
            strict_libraries_checking: false,
        }
    }

    /// Ensure that a the given version, from its id, is properly installed.
    pub fn install(&self, handler: &mut dyn Handler, id: &str) -> Result<()> {
        
        let mut downloads = Vec::new();
        let features = HashSet::new();

        let hierarchy = self.load_hierarchy(handler, id)?;
        let _client_file = self.load_client(handler, &hierarchy, &mut downloads)?;
        let _lib_files = self.load_libraries(handler, &hierarchy, &features, &mut downloads)?;
        let _logger_config = self.load_logger(handler, &hierarchy, &mut downloads)?;
        let _assets = self.load_assets(handler, &hierarchy, &mut downloads)?;

        self.download_many(handler, downloads)?;

        Ok(())

    }

    /// Internal function that loads the version hierarchy from their JSON metadata files.
    fn load_hierarchy(&self, 
        handler: &mut dyn Handler, 
        root_id: &str
    ) -> Result<Vec<Version>> {

        handler.handle(self, Event::HierarchyLoading { root_id })?;

        let mut hierarchy = Vec::new();
        let mut current_id = Some(root_id.to_string());

        while let Some(load_id) = current_id.take() {
            let version = self.load_version(handler, load_id)?;
            if let Some(next_id) = &version.metadata.inherits_from {
                current_id = Some(next_id.clone());
            }
            hierarchy.push(version);
        }

        handler.handle(self, Event::HierarchyLoaded { hierarchy: &mut hierarchy })?;

        Ok(hierarchy)

    }

    /// Internal function that loads a version from its JSON metadata file.
    fn load_version(&self, 
        handler: &mut dyn Handler, 
        id: String
    ) -> Result<Version> {

        if id.is_empty() {
            return Err(Error::VersionNotFound { id });
        }

        let dir = self.versions_dir.join(&id);
        let file = dir.join_with_extension(&id, "json");

        handler.handle(self, Event::VersionLoading { id: &id, file: &file })?;

        loop {

            let reader = match File::open(&file) {
                Ok(reader) => BufReader::new(reader),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {

                    // If not retried, we return a version not found error.
                    match handler.handle(self, Event::VersionNotFound { id: &id, file: &file, error: None }) {
                        Ok(()) => return Err(Error::VersionNotFound { id }),
                        Err(Error::Retry) => continue,
                        Err(e) => return Err(e),
                    }

                }
                Err(e) => return Err(Error::new_file_io(file, e))
            };

            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            let mut metadata: serde::VersionMetadata = match serde_path_to_error::deserialize(&mut deserializer) {
                Ok(obj) => obj,
                Err(e) => return Err(Error::new_file_json(file, e)),
            };

            handler.handle(self, Event::VersionLoaded { id: &id, file: &file, metadata: &mut metadata })?;

            break Ok(Version {
                id,
                dir,
                metadata,
            });

        }

    }

    /// Load the entry point version JAR file.
    fn load_client(&self, 
        handler: &mut dyn Handler, 
        hierarchy: &[Version], 
        downloads: &mut Vec<Download>
    ) -> Result<PathBuf> {
        
        let root_version = &hierarchy[0];
        let client_file = root_version.dir.join_with_extension(&root_version.id, "jar");

        handler.handle(self, Event::ClientLoading {  })?;

        let dl = hierarchy.iter()
            .filter_map(|version| version.metadata.downloads.get("client"))
            .next();

        if let Some(dl) = dl {
            let check_client_sha1 = dl.sha1.as_deref().filter(|_| self.strict_libraries_checking);
            if !check_file(&client_file, dl.size, check_client_sha1).map_err(Error::new_io)? {
                downloads.push(Download {
                    source: dl.into(),
                    file: client_file.clone().into_boxed_path(),
                    executable: false,
                });
            }
        } else if !client_file.is_file() {
            return Err(Error::ClientNotFound);
        }

        handler.handle(self, Event::ClientLoaded {  })?;
        Ok(client_file)

    }

    /// Load libraries required to run the game.
    fn load_libraries(&self,
        handler: &mut dyn Handler,
        hierarchy: &[Version], 
        features: &HashSet<String>,
        downloads: &mut Vec<Download>
    ) -> Result<LibraryFiles> {

        handler.handle(self, Event::LibrariesLoading {})?;

        // Tracking libraries that are already defined and should not be overridden.
        let mut libraries_set = HashSet::new();
        let mut libraries = Vec::new();

        for version in hierarchy {

            for lib in &version.metadata.libraries {

                let mut lib_gav = lib.name.clone();

                if let Some(lib_natives) = &lib.natives {

                    // If natives object is present, the classifier associated to the
                    // OS overrides the library specifier classifier. If not existing,
                    // we just skip this library because natives are missing.
                    let Some(classifier) = lib_natives.get(&self.meta_os_name) else {
                        continue;
                    };

                    // If we find a arch replacement pattern, we must replace it with
                    // the target architecture bit-ness (32, 64).
                    const ARCH_REPLACEMENT_PATTERN: &str = "${arch}";
                    if let Some(pattern_idx) = lib_gav.classifier().find(ARCH_REPLACEMENT_PATTERN) {
                        let mut classifier = classifier.clone();
                        classifier.replace_range(pattern_idx..pattern_idx + ARCH_REPLACEMENT_PATTERN.len(), &self.meta_os_bits);
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

                libraries.push(Library {
                    gav: lib_gav,
                    path: None,
                    source: None,
                    natives: lib.natives.is_some(),
                });

                let lib_obj = libraries.last_mut().unwrap();

                let lib_dl;
                if lib_obj.natives {
                    lib_dl = lib.downloads.classifiers.get(lib_obj.gav.classifier());
                } else {
                    lib_dl = lib.downloads.artifact.as_ref();
                }

                if let Some(lib_dl) = lib_dl {
                    lib_obj.path = lib_dl.path.as_ref().map(PathBuf::from);
                    lib_obj.source = Some(DownloadSource::from(&lib_dl.download));
                } else if let Some(repo_url) = &lib.url {
                    
                    // If we don't have any download information, it's possible to use
                    // the 'url', which is the base URL of a maven repository, that we
                    // can derive with the library name to find a URL.

                    let mut url = repo_url.clone();

                    if url.ends_with('/') {
                        url.truncate(url.len() - 1);
                    }
                    
                    for component in lib_obj.gav.file_components() {
                        url.push('/');
                        url.push_str(&component);
                    }
                    
                    lib_obj.source = Some(DownloadSource {
                        url: url.into_boxed_str(),
                        size: None,
                        sha1: None,
                    });

                }

                // Additional check because libraries with empty URLs have been seen in
                // the wild, so we remove the source if its URL is empty.
                if let Some(lib_source) = &lib_obj.source {
                    if lib_source.url.is_empty() {
                        lib_obj.source = None;
                    }
                }

            }

        }

        handler.handle(self, Event::LibrariesLoaded { libraries: &mut libraries })?;

        let mut lib_files = LibraryFiles::default();

        // After possible filtering by event handler, verify libraries and download 
        // missing ones.
        for lib in libraries {

            // Construct the library path depending on its presence.
            let lib_file = {
                let mut buf = self.libraries_dir.clone();
                if let Some(lib_rel_path) = lib.path.as_deref() {
                    buf.push(lib_rel_path);
                } else {
                    for comp in lib.gav.file_components() {
                        buf.push(&*comp);
                    }
                }
                buf
            };

            // If no repository URL is given, no more download method is available,
            // so if the JAR file isn't installed, the game cannot be launched.
            // 
            // Note: In the past, we used to default the url to Mojang's maven 
            // repository, but this was a bad habit because most libraries could
            // not be downloaded from their repository, and this was confusing to
            // get a download error for such libraries.
            if let Some(source) = lib.source {
                // Only check SHA-1 if strict checking is enabled.
                let check_source_sha1 = source.sha1.as_ref().filter(|_| self.strict_libraries_checking);
                if !check_file(&lib_file, source.size, check_source_sha1).map_err(Error::new_io)? {
                    downloads.push(Download {
                        source,
                        file: lib_file.clone().into_boxed_path(),
                        executable: false,
                    });
                }
            } else if !lib_file.is_file() {
                return Err(Error::LibraryNotFound { gav: lib.gav })
            }

            (if lib.natives { 
                &mut lib_files.natives_files 
            } else { 
                &mut lib_files.class_files 
            }).push(lib_file);

        }

        handler.handle(self, Event::LibrariesVerified {
            class_files: &lib_files.class_files,
            natives_files: &lib_files.natives_files,
        })?;

        Ok(lib_files)

    }

    /// Load libraries required to run the game.
    fn load_logger(&self,
        handler: &mut dyn Handler,
        hierarchy: &[Version], 
        downloads: &mut Vec<Download>,
    ) -> Result<Option<LoggerConfig>> {

        let config = hierarchy.iter()
            .filter_map(|version| version.metadata.logging.get("client"))
            .next();

        let Some(config) = config else {
            handler.handle(self, Event::LoggerAbsent {  })?;
            return Ok(None);
        };

        handler.handle(self, Event::LoggerLoading { id: &config.file.id })?;

        let file = {
            let mut buf = self.assets_dir.join("log_configs");
            buf.push(&config.file.id);
            buf
        };

        if !check_file(&file, config.file.download.size, config.file.download.sha1.as_deref()).map_err(Error::new_io)? {
            downloads.push(Download {
                source: DownloadSource::from(&config.file.download),
                file: file.clone().into_boxed_path(),
                executable: false,
            });
        }

        handler.handle(self, Event::LoggerLoaded { id: &config.file.id })?;

        Ok(Some(LoggerConfig {
            kind: config.r#type,
            argument: config.argument.clone(),
            file,
        }))

    }

    /// Load and verify all assets of the game.
    fn load_assets(&self, 
        handler: &mut dyn Handler, 
        hierarchy: &[Version], 
        downloads: &mut Vec<Download>
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
            handler.handle(self, Event::AssetsAbsent {  })?;
            return Ok(None);
        };

        handler.handle(self, Event::AssetsLoading { id: index_info.id })?;

        // Resolve all used directories and files...
        let indexes_dir = self.assets_dir.join("indexes");
        let index_file = indexes_dir.join_with_extension(index_info.id, "json");

        // All modern version metadata have download information attached to the assets
        // index identifier, we check the file against the download information and then
        // download this single file. If the file has no download info
        if let Some(dl) = index_info.download {
            if !check_file(&index_file, dl.size, dl.sha1.as_deref()).map_err(Error::new_io)? {
                self.download_many(handler, vec![Download {
                    source: dl.into(),
                    file: index_file.clone().into_boxed_path(),
                    executable: false,
                }])?;
            }
        }

        let reader = match File::open(&index_file) {
            Ok(reader) => BufReader::new(reader),
            Err(e) if e.kind() == io::ErrorKind::NotFound =>
                return Err(Error::AssetsNotFound { id: index_info.id.to_owned() }),
            Err(e) => 
                return Err(Error::new_file_io(index_file, e))
        };

        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let asset_index: serde::AssetIndex = match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(obj) => obj,
            Err(e) => return Err(Error::new_file_json(index_file, e))
        };
        
        handler.handle(self, Event::AssetsLoaded { id: index_info.id, index: &asset_index })?;

        // Now we check assets that needs to be downloaded...
        let objects_dir = self.assets_dir.join("objects");
        let mut asset_file_name = String::new();
        let mut unique_hashes = HashSet::new();
        let mut assets = Assets::default();

        for (asset_path, asset) in &asset_index.objects {

            asset_file_name.clear();
            for byte in *asset.hash {
                write!(asset_file_name, "{byte:02x}").unwrap();
            }
            
            let asset_hash_prefix = &asset_file_name[0..2];
            let asset_hash_file = {
                let mut buf = objects_dir.clone();
                buf.push(asset_hash_prefix);
                buf.push(&asset_file_name);
                buf
            };

            // Save the association of asset path to the actual hash file.
            assets.objects.insert(PathBuf::from(asset_path).into_boxed_path(), asset_hash_file.clone().into_boxed_path());

            // Some assets are represented with multiple files, but we don't 
            // want to download a file multiple time so we abort here.
            if !unique_hashes.insert(&*asset.hash) {
                continue;
            }

            // Only check SHA-1 if strict checking.
            let check_asset_sha1 = self.strict_assets_checking.then_some(&*asset.hash);
            if !check_file(&asset_hash_file, Some(asset.size), check_asset_sha1).map_err(Error::new_io)? {
                downloads.push(Download {
                    source: DownloadSource {
                        url: format!("{RESOURCES_URL}{asset_hash_prefix}/{asset_file_name}").into_boxed_str(),
                        size: Some(asset.size),
                        sha1: Some(*asset.hash),
                    },
                    file: asset_hash_file.into_boxed_path(),
                    executable: false,
                });
            }

        }

        handler.handle(self, Event::AssetsVerified { id: index_info.id, index: &asset_index })?;

        Ok(Some(assets))

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

        if let Some(name) = &rule_os.name {
            if name != &self.meta_os_name {
                return false;
            }
        }

        if let Some(arch) = &rule_os.arch {
            if arch != &self.meta_os_arch {
                return false;
            }
        }

        if let Some(version) = &rule_os.version {
            if !version.is_match(&self.meta_os_version) {
                return false;
            }
        }

        true

    }

    /// Bulk download a sequence of entries, events will be sent to the handler. After
    /// this method returns successfully, all downloaded files are guaranteed to have
    /// been downloaded at their location.
    pub fn download_many(&self, handler: &mut dyn Handler, downloads: Vec<Download>) -> Result<()> {
        download::download_many_blocking(self, handler, downloads)
    }

    /// Shortcut for calling [`Self::download_many`] with a single download, check it
    /// for more information.
    #[inline]
    pub fn download(&self, handler: &mut dyn Handler, download: Download) -> Result<()> {
        self.download_many(handler, vec![download])
    }

}

/// Handler for events happening when installing.
pub trait Handler {

    /// Handle an even from the installer.
    fn handle(&mut self, installer: &Installer, event: Event) -> Result<()>;

}

/// Blanket implementation that does nothing.
impl Handler for () {
    
    fn handle(&mut self, installer: &Installer, event: Event) -> Result<()> {
        let _ = (installer, event);
        Ok(())
    }

}

/// An event produced by the installer that can be handled by the install handler.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// The version hierarchy will be loaded.
    HierarchyLoading {
        root_id: &'a str,
    },
    /// The version hierarchy has been loaded successfully.
    HierarchyLoaded {
        /// All versions of the hierarchy, in order, starting at the root version.
        hierarchy: &'a mut Vec<Version>,
    },
    /// A version will be loaded.
    VersionLoading {
        id: &'a str,
        file: &'a Path,
    },
    /// A version file has not been found but is needed. An optional parsing error can
    /// be attached if the file exists but is invalid.
    /// 
    /// **Retry**: this will retry to open and load the version file. If not retried, the
    /// installation halts with [`Error::VersionNotFound`] error.
    VersionNotFound {
        id: &'a str,
        file: &'a Path,
        error: Option<serde_path_to_error::Error<serde_json::Error>>,
    },
    /// A version file has been loaded successfully.
    VersionLoaded {
        id: &'a str,
        file: &'a Path,
        metadata: &'a mut serde::VersionMetadata,
    },
    /// The client JAR file will be loaded.
    ClientLoading {},
    /// The client JAR file has been loaded successfully.
    ClientLoaded {},
    /// Libraries will be loaded.
    LibrariesLoading {},
    /// Libraries have been loaded, this can be altered by the event handler. After that,
    /// the libraries will be verified and added to the downloads list.
    LibrariesLoaded {
        libraries: &'a mut Vec<Library>,
    },
    /// Libraries have been verified.
    LibrariesVerified {
        class_files: &'a [PathBuf],
        natives_files: &'a [PathBuf],
    },
    /// No logger configuration will be loaded because version doesn't specify any.
    LoggerAbsent {},
    /// The logger configuration will be loaded.
    LoggerLoading {
        id: &'a str,
    },
    /// Logger configuration has been loaded successfully.
    LoggerLoaded {
        id: &'a str,
    },
    /// Assets will not be loaded because version doesn't specify any.
    AssetsAbsent {},
    /// Assets will be loaded.
    AssetsLoading {
        id: &'a str,
    },
    /// Assets have been loaded, and are going to be verified in order to att missing 
    /// ones to the download list.
    AssetsLoaded {
        id: &'a str,
        index: &'a serde::AssetIndex,
    },
    /// Assets have been verified.
    AssetsVerified {
        id: &'a str,
        index: &'a serde::AssetIndex,
    },
    /// Notification of a download progress. This event isn't required to be produced
    /// for each entry, however it should be produced at the beginning with a count and 
    /// size of 0 and a total count with the total number of downloads. A download is 
    /// considered finished when the count reaches the total count.
    DownloadProgress {
        /// Number of entries successfully downloaded.
        count: u32,
        /// Total number of entries that will be downloaded.
        total_count: u32,
        /// The current downloaded size.
        size: u32,
        /// Total size of all entries that will be downloaded, this can increase while 
        /// downloading if file sizes were unknown are are now known, or if some 
        /// downloads are retried.
        total_size: u32,
    },
}

/// The standard installer could not proceed to the installation of a version.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The given version is not found when trying to fetch it.
    #[error("version not found: {id}")]
    VersionNotFound {
        id: String,
    },
    /// The given version is not found and no download information is provided.
    #[error("assets not found: {id}")]
    AssetsNotFound {
        id: String,
    },
    /// The version JAR file that is required has no download information and is not 
    /// already existing, is is mandatory to build the class path.
    #[error("client not found")]
    ClientNotFound,
    /// A library has no download information and is missing the libraries directory.
    #[error("library not found: {gav}")]
    LibraryNotFound {
        gav: Gav,
    },
    /// A special error that is returned by the handler to request a retry for a specific
    /// phase, if this error is returned by the global installation process, it means that
    /// the retry was not possible. The retry-ability of a phase is described on events.
    #[error("retry")]
    Retry,
    /// A special error than can be used by the handler to halt the installation process
    /// when an event is produced.
    #[error("halt")]
    Halt,
    /// Download error, associating its failed download entry to the download error.
    #[error("download: {errors:?}")]
    Download {
        errors: Vec<(Download, DownloadError)>,
    },
    /// A developer-oriented error that cannot be handled with other errors, it has an
    /// origin that could be a file or any other raw string, attached to the actual error.
    /// This includes filesystem, network, JSON parsing and schema errors.
    #[error("other: {kind:?} @ {origin:?}")]
    Other {
        /// The origin of the error, can be a file path.
        origin: ErrorOrigin,
        /// The error error kind from the origin.
        kind: ErrorKind,
    },
}

/// Origin of an uncategorized error.
#[derive(Debug, Default)]
pub enum ErrorOrigin {
    /// Unknown origin for the error.
    #[default]
    Unknown,
    /// The error is related to a specific file.
    File(Box<Path>),
    /// The origin of the error is explained in this raw message.
    Raw(Box<str>),
}

/// Kind of an uncategorized error.
#[derive(Debug)]
pub enum ErrorKind {
    Io(io::Error),
    Json(serde_path_to_error::Error<serde_json::Error>),
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    
    #[inline]
    pub fn new_io(e: io::Error) -> Self {
        Self::Other { origin: ErrorOrigin::Unknown, kind: ErrorKind::Io(e) }
    }
    
    #[inline]
    pub fn new_file_io(file: impl Into<Box<Path>>, e: io::Error) -> Self {
        Self::Other { origin: ErrorOrigin::File(file.into()), kind: ErrorKind::Io(e) }
    }
    
    #[inline]
    pub fn new_raw_io(raw: impl Into<Box<str>>, e: io::Error) -> Self {
        Self::Other { origin: ErrorOrigin::Raw(raw.into()), kind: ErrorKind::Io(e) }
    }
    
    #[inline]
    pub fn new_file_json(file: impl Into<Box<Path>>, e: serde_path_to_error::Error<serde_json::Error>) -> Self {
        Self::Other { origin: ErrorOrigin::File(file.into()), kind: ErrorKind::Json(e) }
    }

}

/// Represent a loaded version.
#[derive(Debug)]
pub struct Version {
    /// Identifier of this version.
    pub id: String,
    /// Directory of that version, where metadata is stored with the JAR file.
    pub dir: PathBuf,
    /// The loaded metadata of the version.
    pub metadata: serde::VersionMetadata,
}

/// Represent a loaded library.
#[derive(Debug)]
pub struct Library {
    /// GAV for this library.
    pub gav: Gav,
    /// The path to install the library at, relative to the libraries directory, by 
    /// default it will be derived from the library specifier.
    pub path: Option<PathBuf>,
    /// An optional download source for this library if it is missing.
    pub source: Option<DownloadSource>,
    /// True if this contains natives that should be extracted into the binaries 
    /// directory before launching the game, instead of being in the class path.
    pub natives: bool,
}

/// Internal resolved assets associating the virtual file path to its hash file path.
#[derive(Debug, Default)]
struct Assets {
    objects: HashMap<Box<Path>, Box<Path>>,
}

/// Internal resolved libraries file paths.
#[derive(Debug, Default)]
struct LibraryFiles {
    class_files: Vec<PathBuf>,
    natives_files: Vec<PathBuf>,
}

/// Internal resolved logger configuration.
#[derive(Debug)]
#[allow(unused)]  // FIXME:
struct LoggerConfig {
    kind: serde::VersionLoggingType,
    argument: String,
    file: PathBuf,
}

/// Check if a file at a given path has the corresponding properties (size and/or SHA-1), 
/// returning true if it is valid, so false is returned anyway if the file doesn't exists.
fn check_file(
    file: &Path,
    size: Option<u32>,
    sha1: Option<&[u8; 20]>,
) -> io::Result<bool> {

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
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => return Err(e),
        }
    } else {
        match (file.metadata(), size) {
            // File is existing and we want to check size...
            (Ok(metadata), Some(size)) => Ok(metadata.len() == size as u64),
            // File is existing but we don't have size to check, no need to download.
            (Ok(_metadata), None) => Ok(true),
            (Err(e), _) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            (Err(e), _) => return Err(e),
        }
    }

}

/// Return the default main directory for Minecraft, so called ".minecraft".
fn default_main_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        dirs::data_dir().map(|dir| dir.join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir().map(|dir| dir.join("minecraft"))
    } else {
        dirs::home_dir().map(|dir| dir.join(".minecraft"))
    }
}

/// Return the default OS name for rules.
/// Returning none if the OS is not supported.
/// 
/// This is currently not dynamic, so this will return the OS name the binary 
/// has been compiled for.
fn default_meta_os_name() -> Option<String> {
    Some(match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "osx",
        "freebsd" => "freebsd",
        "openbsd" => "openbsd",
        "netbsd" => "netbsd",
        _ => return None
    }.to_string())
}

/// Return the default OS system architecture name for rules.
/// 
/// This is currently not dynamic, so this will return the OS architecture the binary
/// has been compiled for.
fn default_meta_os_arch() -> Option<String> {
    Some(match std::env::consts::ARCH {
        "x86" => "x86",
        "x86_64" => "x86_64",
        "arm" => "arm32",
        "aarch64" => "arm64",
        _ => return None
    }.to_string())
}

/// Return the default OS version name for rules.
fn default_meta_os_version() -> Option<String> {
    use os_info::Version;
    match os_info::get().version() {
        Version::Unknown => None,
        version => Some(version.to_string())
    }
}

/// Return the default OS version name for rules.
fn default_meta_os_bits() -> Option<String> {
    match std::env::consts::ARCH {
        "x86" | "arm" => Some("32".to_string()),
        "x86_64" | "aarch64" => Some("64".to_string()),
        _ => return None
    }
}
