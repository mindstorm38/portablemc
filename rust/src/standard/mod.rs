//! Standard installation procedure.

mod serde;
mod error;
mod specifier;

use std::collections::{HashMap, HashSet};
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::fmt::Write;
use std::fs::File;

use sha1::{Digest, Sha1};
use serde_json::Value;

use crate::util::PathExt;

pub use self::error::{Result, Error, ErrorKind, ErrorOrigin};
pub use self::specifier::LibrarySpecifier;

/// Base URL for downloading game's assets.
const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";
/// Base URL for downloading game's libraries.
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

/// This is the standard version installer that provides minimal and common installation
/// of Minecraft versions. The install procedure given by this installer is idempotent,
/// which mean that if the installer's configuration has not been modified, running it a
/// second time won't do any modification.
/// 
/// This various important directories used by the installer can be configured as needed.
#[derive(Debug)]
pub struct Installer {
    /// The main directory contains all static resources that will not be modified during
    /// runtime, this includes versions, libraries and assets.
    pub main_dir: PathBuf,
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
}

impl Installer {

    /// Create a new installer with default directories and meta OS values for filtering.
    /// Return none if one of the value is not available on your system.
    pub fn new() -> Option<Self> {
        let main_dir = default_main_dir()?;
        let work_dir = main_dir.clone();
        let bin_dir = main_dir.join("bin");
        Some(Self {
            main_dir,
            work_dir,
            bin_dir,
            meta_os_name: default_meta_os_name()?,
            meta_os_arch: default_meta_os_arch()?,
            meta_os_version: default_meta_os_version()?,
            meta_os_bits: default_meta_os_bits()?,
        })
    }

    /// Construct path to the versions directory.
    pub fn versions_dir(&self) -> PathBuf {
        self.main_dir.join("versions")
    }

    /// Construct path to a particular version directory.
    pub fn version_dir(&self, version: &str) -> PathBuf {
        let mut buf = self.versions_dir();
        buf.push(version);
        buf
    }

    /// Construct path to a particular version file inside the version directory.
    pub fn version_file(&self, version: &str, extension: &str) -> PathBuf {
        let mut buf = self.version_dir(version);
        buf.push(version);
        buf.with_extension(extension);
        buf
    }

    /// Ensure that a version, given its name, has all its resources properly installed 
    /// and is ready to be launched, the returned environment is returned if successful.
    /// 
    /// This function in itself doesn't fetch missing versions, for that the caller need
    /// to pass in a handler that will cover such case (for example with Mojang version),
    /// the handler also provides the download method, so handler predefined structures
    /// are made to be wrapped into other ones, each being specific.
    pub fn install(&self, version: &str, handler: &mut dyn Handler) -> Result<Environment> {

        // TODO: Make a global list of JSON errors so that we can list every problem
        // and return all of them at once.

        // All downloads to start at the end of resolution before launching.
        let mut downloads = Vec::new();

        // Start by resolving the version hierarchy, with requests if needed.
        let hierarchy = self.resolve_hierarchy(version, handler)?;

        // Build the features list, used when applying metadata rules.
        let mut features = HashMap::new();
        handler.filter_features(self, &mut features)?;

        // Assets may be absent and unspecified in metadata for some custom versions.
        let assets = self.resolve_assets(&hierarchy, &mut downloads, handler)?;

        let libraries = self.resolve_libraries(&hierarchy, &features, &mut downloads, handler)?;

        // Now we want to resolve the main version JAR file.
        let jar_file = self.resolve_jar(&hierarchy, &mut downloads, handler)?;

        // Finally download all required files.
        handler.download(&downloads)?;

        Ok(Environment {

        })

    }

    /// Resolve the version hierarchy and load all metadata. The returned hierarchy has
    /// the first resolved version as the first component (index 0).
    fn resolve_hierarchy(&self, version: &str, handler: &mut dyn Handler) -> Result<Vec<Version>> {

        let mut hierarchy = Vec::new();
        let mut version_id = Some(version.to_string());

        while let Some(current_version_name) = version_id.take() {
            let version = self.load_version(&current_version_name, handler)?;
            if let Some(next_version_id) = &version.metadata.inherits_from {
                version_id = Some(next_version_id.clone());
            }
            hierarchy.push(version);
        }

        // Hierarchy should not be empty here because we load at least one version.
        debug_assert!(!hierarchy.is_empty(), "hierarchy should never be empty before filtering");
        handler.filter_hierarchy(self, &mut hierarchy)?;
        assert!(!hierarchy.is_empty(), "hierarchy is empty after filtering");

        Ok(hierarchy)

    }

    /// Load a specific version given its name, and fallback to handler when needed.
    fn load_version(&self, id: &str, handler: &mut dyn Handler) -> Result<Version> {

        let metadata_file = self.version_file(&id, "json");
        match File::open(&metadata_file) {
            Ok(metadata_reader) => {

                let metadata: serde::Version = match serde_json::from_reader(metadata_reader) {
                    Ok(obj) => obj,
                    Err(e) => return Err(Error::new_file_json(metadata_file, e)),
                };

                let mut version = Version {
                    metadata,
                    id: id.to_string(),
                };

                if handler.filter_version(self, &mut version)? {
                    return Ok(version);
                }
                
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::new_file_io(metadata_file, e))
        };

        handler.fetch_version(self, id)

    }

    /// Resolve the given version's merged metadata assets to use for the version. This
    /// returns a full description of what assets to use (if so) and the list. This 
    /// function push downloads for each missing asset.
    fn resolve_assets(&self, 
        hierarchy: &[Version], 
        downloads: &mut Vec<Download>, 
        handler: &mut dyn Handler
    ) -> Result<Option<Assets>> {

        /// Internal description of asset information first found in hierarchy.
        #[derive(Debug)]
        struct AssetIndexInfo<'a> {
            download: Option<&'a serde::Download>,
            id: &'a str,
        }

        // We search the first version that provides asset informations, we also support
        // the legacy 'assets' that doesn't have download information.
        let asset_index_info = hierarchy.iter()
            .find_map(|version| {
                if let Some(asset_index) = &version.metadata.asset_index {
                    Some(AssetIndexInfo {
                        download: Some(&asset_index.download),
                        id: &asset_index.id,
                    })
                } else if let Some(asset_id) = &version.metadata.assets {
                    Some(AssetIndexInfo {
                        download: None,
                        id: &asset_id,
                    })
                } else {
                    None
                }
            });

        // Just ignore if no asset information is provided.
        let Some(asset_index_info) = asset_index_info else {
            return Ok(None)
        };

        // Resolve all used directories and files...
        let asset_dir = self.main_dir.join("assets");
        let asset_indexes_dir = asset_dir.join("indexes");
        let asset_index_file = asset_indexes_dir.join_with_extension(asset_index_info.id, "json");

        // Either download the index directly or open the file if no download info.
        let asset_index_reader = match asset_index_info.download {
            Some(dl) => self.check_and_read_file(&asset_index_file, dl.size, dl.sha1.as_deref().copied(), &dl.url, handler)?,
            None => File::open(&asset_index_file).map_err(|e| Error::new_file_io(&*asset_index_file, e))?,
        };

        let asset_index: serde::AssetIndex = match serde_json::from_reader(asset_index_reader) {
            Ok(obj) => obj,
            Err(e) => return Err(Error::new_file_json(asset_index_file, e)),
        };

        let mut assets = Assets {
            id: asset_index_info.id.to_string(),
            index: asset_index,
        };

        // Filter assets before checking ones to download.
        handler.filter_assets(self, &mut assets)?;

        // Now we check assets that needs to be downloaded...
        let mut asset_file = asset_dir.join("objects");
        let mut asset_file_name = String::new();

        for asset in assets.index.objects.values() {

            for byte in *asset.hash {
                write!(asset_file_name, "{byte:02x}").unwrap();
            }

            let asset_hash_name = &asset_file_name[0..2];
            asset_file.push(asset_hash_name);
            asset_file.push(&asset_file_name);

            // We intentionally don't check SHA-1 because there are too many assets, it
            // would be slow. TODO: Parameter on the installer to make it more robust and
            // therefore test SHA-1.
            if self.check_file(&asset_file, Some(asset.size), None)? {
                downloads.push(Download {
                    url: format!("{RESOURCES_URL}{asset_hash_name}/{asset_file_name}").into_boxed_str(),
                    file: asset_file.clone().into_boxed_path(),
                    size: Some(asset.size),
                    sha1: Some(*asset.hash),
                    executable: false,
                })
            }

            asset_file.pop();
            asset_file.pop();
            asset_file_name.clear();

        }

        Ok(Some(assets))

    }

    /// Resolve the entrypoint JAR file used for that version. This will first check if
    /// it is explicitly specified in the metadata, if so it will schedule it for 
    /// download if relevant, if not it will use the already present JAR file. If
    /// no JAR file exists, an [`Error::JarNotFound`] error is returned.
    fn resolve_jar(&self, 
        hierarchy: &[Version],
        downloads: &mut Vec<Download>, 
        handler: &mut dyn Handler
    ) -> Result<PathBuf> {

        let jar_file = self.version_file(&hierarchy[0].id, "jar");
        let downloads_client = hierarchy.iter()
            .find_map(|v| v.metadata.downloads.get("client"));

        match downloads_client {
            Some(dl) => {
                if self.check_file(&jar_file, dl.size, dl.sha1.as_deref().copied())? {
                    downloads.push(DownloadSource::from(dl).into_full(jar_file.clone().into_boxed_path(), false));
                }
            }
            None => {
                if !jar_file.is_file() {
                    return Err(Error::JarNotFound());
                }
            }
        }

        handler.notify_jar(self, &jar_file)?;
        Ok(jar_file)

    }

    /// Resolve all game libraries.
    /// 
    /// **Note that this is the most critical step and libraries resolving is really 
    /// important for running the game correctly.**
    /// 
    /// *This step has to support both older format where native libraries were given
    /// appart from regular class path libraries, all of this should also support 
    /// automatic downloading both from an explicit artifact URL, or with a maven repo
    /// URL.*
    fn resolve_libraries(&self, 
        hierarchy: &[Version],
        features: &HashMap<String, bool>,
        downloads: &mut Vec<Download>, 
        handler: &mut dyn Handler
    ) -> Result<()> {

        // Note that the metadata has been merged from all versions in the hierarchy,
        // if present, the libraries array will start with libraries defined by the root
        // version. This is important to notice because we want to define each version
        // only once, it's important for class path ordering for some corner cases with 
        // mod loaders.

        let mut libraries = HashMap::new();

        for version in hierarchy {

            for lib in &version.metadata.libraries {

                let mut lib_spec = lib.name.clone();

                if let Some(lib_natives) = &lib.natives {

                    // If natives object is present, the classifier associated to the
                    // OS overrides the library specifier classifier. If not existing,
                    // we just skip this library because natives are missing.
                    let Some(classifier) = lib_natives.get(&self.meta_os_name) else {
                        handler.notify_library(self, &lib.name, LibraryState::RejectedNatives);
                        continue;
                    };

                    // If we find a arch replacement pattern, we must replace it with
                    // the target architecture bit-ness (32, 64).
                    const ARCH_REPLACEMENT_PATTERN: &str = "${arch}";
                    if let Some(pattern_idx) = lib_spec.classifier().find(ARCH_REPLACEMENT_PATTERN) {
                        let mut classifier = classifier.clone();
                        classifier.replace_range(pattern_idx..pattern_idx + ARCH_REPLACEMENT_PATTERN.len(), &self.meta_os_bits);
                        lib_spec.set_classifier(Some(&classifier));
                    } else {
                        lib_spec.set_classifier(Some(&classifier));
                    }

                }

                // Start by applying rules before the actual parsing. Important, we do
                // that after checking natives, so this will override the lib state if
                // rejected, and we still benefit from classifier resolution.
                if let Some(lib_rules) = &lib.rules {
                    if !self.check_rules(lib_rules, features, None) {
                        handler.notify_library(self, &lib.name, LibraryState::RejectedRules);
                        continue;
                    }
                }

                // This library is retained so we insert it in the global libraries.
                handler.notify_library(self, &lib_spec, LibraryState::Retained);
                let lib_obj = libraries.entry(lib_spec.clone()).or_insert(Library {
                    spec: lib_spec,
                    path: None,
                    source: None,
                    natives: lib.natives.is_some(),
                });

                let lib_dl;
                if lib_obj.natives {
                    lib_dl = lib.downloads.classifiers.get(lib_obj.spec.classifier());
                } else {
                    lib_dl = lib.downloads.artifact.as_ref();
                }

                if let Some(lib_dl) = lib_dl {
                    lib_obj.path = lib_dl.path.as_ref().map(|p| PathBuf::from(p).into_boxed_path());
                    lib_obj.source = Some(DownloadSource::from(&lib_dl.download));
                } else if let Some(repo_url) = &lib.url {
                    
                    // If we don't have any download information, it's possible to use
                    // the 'url', which is the base URL of a maven repository, that we
                    // can derive with the library name to find a URL.

                    let mut url = repo_url.clone();
                    url.strip_suffix('/');
                    
                    for component in lib_obj.spec.file_components() {
                        url.push('/');
                        url.push_str(&component);
                    }
                    
                    lib_obj.source = Some(DownloadSource {
                        url: url.into_boxed_str(),
                        size: None,
                        sha1: None,
                    });

                }

            }

        }

        Err(Error::NotSupported("resolve_libraries"))

    }

    // fn check_and_read_download(&self,
    //     file: &Path,
    //     download: JsonDownload<'_>,
    //     handler: &mut dyn Handler,
    // ) -> Result<File> {
    //     self.check_and_read_file(file, download.size, download.sha1, download.url, handler)
    // }

    /// Ensure that a file exists from its download entry, checking that the file has the
    /// right size and SHA-1, if relevant. This will push the download to the handler and
    /// immediately flush the handler.
    fn check_and_read_file(&self, 
        file: &Path,
        size: Option<u32>,
        sha1: Option<[u8; 20]>,
        url: &str,
        handler: &mut dyn Handler,
    ) -> Result<File> {

        // If the file need to be (re)downloaded...
        if self.check_file(file, size, sha1)? {
            handler.download(&[Download {
                source: DownloadSource {
                    url: url.into(),
                    size,
                    sha1,
                },
                file: file.into(),
                executable: false,
            }])?;
        }

        // The handler should have checked it and it should be existing.
        match File::open(file) {
            Ok(reader) => Ok(reader),
            Err(e) if e.kind() == io::ErrorKind::NotFound =>
                unreachable!("handler returned no error but downloaded file is absent"),
            Err(e) => return Err(Error::new_file_io(file, e)),
        }
        
    }

    /// Check if a file at a given path should be downloaded by checking the given 
    /// properties, this also returns true if the file doesn't exists.
    fn check_file(&self,
        file: &Path,
        size: Option<u32>,
        sha1: Option<[u8; 20]>,
    ) -> Result<bool> {

        /// Just an internal block wrapper for I/O error.
        fn check_reader(
            mut reader: File,
            size: Option<u32>,
            sha1: [u8; 20],
        ) -> io::Result<bool> {

            // If relevant, start by checking the actual size of the file.
            if let Some(size) = size {
                let actual_size = reader.seek(SeekFrom::End(0))?;
                if size as u64 != actual_size {
                    return Ok(true);
                }
                reader.seek(SeekFrom::Start(0))?;
            }
            
            // Only after we compute hash...
            let mut digest = Sha1::new();
            io::copy(&mut reader, &mut digest)?;
            if digest.finalize().as_slice() != sha1 {
                return Ok(true);
            }
            
            Ok(false)

        }

        if let Some(sha1) = sha1 {
            // If we want to check SHA-1 we need to open the file and compute it...
            match File::open(file) {
                Ok(reader) => check_reader(reader, size, sha1)
                    .map_err(|e| Error::new_file_io(file, e)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
                Err(e) => Err(Error::new_file_io(file, e)),
            }
        } else {
            match (file.metadata(), size) {
                // File is existing and we want to check size...
                (Ok(metadata), Some(size)) => Ok(metadata.len() != size as u64),
                // File is existing but we don't have size to check, no need to download.
                (Ok(_metadata), None) => Ok(false),
                (Err(e), _) if e.kind() == io::ErrorKind::NotFound => Ok(true),
                (Err(e), _) => Err(Error::new_file_io(file, e)),
            }
        }

    }

    /// Resolve the given JSON array as rules and return true if allowed.
    fn check_rules(&self,
        rules: &[serde::Rule],
        features: &HashMap<String, bool>,
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
        features: &HashMap<String, bool>, 
        mut all_features: Option<&mut HashSet<String>>
    ) -> Option<serde::RuleAction> {

        if !self.check_rule_os(&rule.os) {
            return None;
        }

        for (feature, feature_expected) in &rule.features {

            // Only check if still valid...
            if features.get(feature).copied().unwrap_or_default() != *feature_expected {
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

}

/// A handler is given when installing a version and allows tracking installation progress
/// and also provides methods to alter the installed version, such as downloading missing
/// versions or downloading missing files.
pub trait Handler {

    /// Filter an individual version that have just been loaded from a file, this method
    /// should return false if the version should be requested again.
    fn filter_version(&mut self, installer: &Installer, version: &mut Version) -> Result<bool> {
        let _ = (installer, version);
        Ok(true)
    }

    /// When a version is missing, is it requested by calling this method. This method
    /// returns a [`Error::VersionNotFound`] by default. This method is responsible of
    /// writing the version metadata file if it's needed to be persistent.
    fn fetch_version(&mut self, installer: &Installer, version: &str) -> Result<Version> {
        let _ = installer;
        Err(Error::VersionNotFound(version.into()))
    }

    /// Filter the version hierarchy after full resolution. The given hierarchy is never
    /// empty and this function should not empty it.
    fn filter_hierarchy(&mut self, installer: &Installer, hierarchy: &mut Vec<Version>) -> Result<()> {
        let _ = (installer, hierarchy);
        Ok(())
    }

    /// Filter features that will be used to resolve metadata libraries and arguments.
    fn filter_features(&mut self, installer: &Installer, features: &mut HashMap<String, bool>) -> Result<()> {
        let _ = (installer, features);
        Ok(())
    }

    /// Filter assets that will be installed for that version, this can be altered but 
    /// you must be aware that changing any of the objects or index version will need 
    /// to be coherent because the game only depends on the asset index file.
    fn filter_assets(&mut self, installer: &Installer, assets: &mut Assets) -> Result<()> {
        let _ = (installer, assets);
        Ok(())
    }

    /// Notify the jar file that will be used as the entry point to launching the game.
    /// The JAR file may not already exists and may be bulk downloaded later.
    fn notify_jar(&mut self, installer: &Installer, jar_file: &Path) -> Result<()> {
        let _ = (installer, jar_file);
        Ok(())
    }

    // Notify the handler that a library has been resolved with the given state.
    fn notify_library(&mut self, installer: &Installer, spec: &LibrarySpecifier, state: LibraryState) {
        let _ = (installer, spec, state);
    }

    /// Filter libraries after initial resolution.
    fn filter_libraries(&mut self, installer: &Installer, libraries: &mut HashMap<LibrarySpecifier, Library>) -> Result<()> {
        let _ = (installer, libraries);
        Ok(())
    }

    /// Bulk download entries synchronously, this should be the preferred way to download
    /// a file as-is. When successful, this method should return the total bytes 
    /// downloaded.
    /// 
    /// This method should not check if the file already exists, it should always
    /// download it and only then check size and SHA-1, if relevant.
    fn download(&mut self, entries: &[Download]) -> Result<usize> {
        let _ = entries;
        Err(Error::NotSupported("Handler::download"))
    }

}

/// Default implementation that doesn't override the default method implementations,
/// useful to terminate generic handler wrappers.
impl Handler for () { }

/// Represent a single version in the versions hierarchy. This contains the loaded version
/// name and metadata that will be merged after filtering.
#[derive(Debug)]
pub struct Version {
    /// The name of the version.
    pub id: String,
    /// The serde object describing this version.
    pub metadata: serde::Version,
}

/// Represent all the assets used for the game.
#[derive(Debug)]
pub struct Assets {
    /// The version of assets index.
    pub id: String,
    /// The index contains the definition for all objects.
    pub index: serde::AssetIndex,
}

/// Resolution state for a library, before filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryState {
    /// The library has been retained for installation.
    Retained,
    /// Some rules have rejected this library.
    RejectedRules,
    /// The natives variant of the library got excluded because no classifier has been
    /// found for the current os name.
    RejectedNatives,
}

#[derive(Debug)]
pub struct Libraries {
    pub class: Vec<Library>,
    pub natives: Vec<Library>,
}

#[derive(Debug)]
pub struct Library {
    /// Specifier for this library.
    pub spec: LibrarySpecifier,
    /// The path to install the library at, relative to the libraries directory, by 
    /// default it is derived from the library specifier.
    pub path: Option<Box<Path>>,
    /// An optional download source for this library if it is missing.
    pub source: Option<DownloadSource>,
    /// True if this contains natives that should be extracted into the binaries 
    /// directory before launching the game, instead of being in the classpath.
    pub natives: bool,
}

/// The environment of an installed version, this is the entrypoint to run the game.
#[derive(Debug)]
pub struct Environment {

}

/// A download entry that can be delayed until a call to [`Handler::flush_download`].
/// This download object borrows the URL and file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// Source of the download.
    pub source: DownloadSource,
    /// Path to the file to ultimately download.
    pub file: Box<Path>,
    /// True if the file should be made executable on systems where its relevant to 
    /// later execute a binary.
    pub executable: bool,
}

/// A download source, with the URL, expected size (optional) and hash (optional),
/// it doesn't contain any information about the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadSource {
    /// Url of the file to download.
    pub url: Box<str>,
    /// Expected size of the file, checked after downloading.
    pub size: Option<u32>,
    /// Expected SHA-1 of the file, checked after downloading.
    pub sha1: Option<[u8; 20]>,
}

impl<'a> From<&'a serde::Download> for DownloadSource {

    fn from(serde: &'a serde::Download) -> Self {
        Self {
            url: serde.url.clone().into(),
            size: serde.size,
            sha1: serde.sha1.as_deref().copied(),
        }
    }

}

impl DownloadSource {

    #[inline]
    pub fn into_full(self, file: Box<Path>, executable: bool) -> Download {
        Download {
            source: self,
            file,
            executable,
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

// /// Merge two version metadata JSON values. Merging object is recursive and merging 
// /// arrays just append to the destination.
// fn merge_json_metadata(dst: &mut Object, src: &Object) {
//     for (src_key, src_value) in src.iter() {
//         if let Some(dst_value) = dst.get_mut(src_key) {
//             match (dst_value, src_value) {
//                 (Value::Object(dst_object), Value::Object(src_object)) => 
//                     merge_json_metadata(dst_object, src_object),
//                 (Value::Array(dst), Value::Array(src)) =>
//                     dst.extend(src.iter().cloned()),
//                 _ => {}  // Do nothing, do not override destination if mismatch.
//             }
//         } else {
//             dst.insert(src_key.clone(), src_value.clone());
//         }
//     }
// }
