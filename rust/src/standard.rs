//! Standard installer.

use std::result::Result as StdResult;
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::num::NonZeroU16;
use std::fmt::Write;
use std::fs::File;

use sha1::{Digest, Sha1};
use serde_json::Value;

use crate::util::{PathExt, DigestReader};


/// Base URL for downloading game's assets.
const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";
/// Base URL for downloading game's libraries.
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

/// Type alias for a JSON object or string key and values.
pub type Object = serde_json::Map<String, Value>;

/// Type alias for JSON array of values.
pub type Array = Vec<Value>;

/// Type alias for 20 bytes used for computed SHA-1 hash.
pub type Sha1Hash = [u8; 20];

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
    pub fn install(&self, version: &str, handler: &mut dyn Handler) -> Result<()> {

        // All downloads to start at the end of resolution before launching.
        let mut downloads = Vec::new();

        // Start by resolving the version hierarchy, with requests if needed.
        let hierarchy = self.resolve_hierarchy(version, handler)?;
        // Merge the full metadata, because we can only interpret a single metadata.
        let metadata = self.resolve_metadata(&hierarchy, handler)?;

        // Build the features list, used when applying metadata rules.
        let mut features = HashMap::new();
        handler.filter_features(self, &mut features)?;

        // Assets may be absent and unspecified in metadata for some custom versions.
        let assets = self.resolve_assets(&metadata, &mut downloads, handler)?;

        let libraries = self.resolve_libraries(&hierarchy, &mut downloads, handler)?;

        // Now we want to resolve the main version JAR file.
        let jar_file = self.resolve_jar(&metadata, &hierarchy, &mut downloads, handler)?;

        // Finally download all required files.
        handler.download(&downloads)?;

        Ok(())

    }

    /// Resolve the version hierarchy and load all metadata. The returned hierarchy has
    /// the first resolved version as the first component (index 0).
    fn resolve_hierarchy(&self, version: &str, handler: &mut dyn Handler) -> Result<Vec<Version>> {

        let mut hierarchy = Vec::new();
        let mut version_name = Some(version.to_string());

        while let Some(current_version_name) = version_name.take() {

            let mut version = self.load_version(&current_version_name, handler)?;

            if let Some(metadata_inherits) = version.metadata.remove("inheritsFrom") {
                if let Value::String(next_version_name) = metadata_inherits {
                    version_name = Some(next_version_name);
                } else {
                    return Err(Error::JsonSchema(format!("metadata ({current_version_name}): /inheritsFrom must be a string")));
                }
            }

            hierarchy.push(version);

        }

        debug_assert!(!hierarchy.is_empty(), "hierarchy should never be empty before filtering");
        handler.filter_hierarchy(self, &mut hierarchy)?;
        assert!(!hierarchy.is_empty(), "hierarchy is empty after filtering");
        Ok(hierarchy)

    }

    /// Load a specific version given its name, and fallback to handler when needed.
    fn load_version(&self, version: &str, handler: &mut dyn Handler) -> Result<Version> {

        match File::open(self.version_file(&version, "json")) {
            Ok(metadata_reader) => {

                let mut version = Version {
                    metadata: serde_json::from_reader(metadata_reader)?,
                    name: version.to_string(),
                };

                if handler.filter_version(self, &mut version)? {
                    return Ok(version);
                }
                
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::Io(e))
        };

        handler.fetch_version(self, version)

    }

    /// Resolve the full merged metadata from the given loaded version hierarchy.
    fn resolve_metadata(&self, hierarchy: &[Version], handler: &mut dyn Handler) -> Result<Object> {
        
        let mut metadata = Object::new();
        for version in hierarchy {
            merge_json_metadata(&mut metadata, &version.metadata);
        }
        
        handler.filter_metadata(self, &mut metadata)?;
        Ok(metadata)

    }

    /// Resolve the given version's merged metadata assets to use for the version. This
    /// returns a full description of what assets to use (if so) and the list. This 
    /// function push downloads for each missing asset.
    fn resolve_assets(&self, 
        metadata: &Object, 
        downloads: &mut Vec<Download>, 
        handler: &mut dyn Handler
    ) -> Result<Option<Assets>> {

        let Some(assets_index_info) = metadata.get("assetIndex") else {
            // Asset info may not be present, it's not required because some custom 
            // versions may want to use there own internal assets.
            return Ok(None);
        };

        let Value::Object(assets_index_info) = assets_index_info else {
            return Err(Error::JsonSchema(format!("metadata: /assetIndex must be an object")));
        };

        // We also keep the path used, for a more useful error message.
        let Some((assets_index_version, assets_index_version_path)) = 
            metadata.get("assets")
                .map(|val| (val, "/assets"))
                .or_else(|| assets_index_info.get("id")
                    .map(|val| (val, "/assetIndex/id"))) 
        else {
            // Asset info may not be present, same as above.
            return Ok(None);
        };

        let Value::String(assets_index_version) = assets_index_version else {
            return Err(Error::JsonSchema(format!("metadata: {assets_index_version_path} must be a string")));
        };

        // Resolve all used directories and files...
        let assets_dir = self.main_dir.join("assets");
        let assets_indexes_dir = assets_dir.join("indexes");
        let assets_index_file = assets_indexes_dir.join_with_extension(assets_index_version, "json");

        // The assets index info can be parsed as a download entry at this point.
        let assets_index_download = parse_json_download(assets_index_info)
            .map_err(|err| Error::JsonSchema(format!("metadata: /assetIndex{err}")))?;

        let assets_index_reader = self.check_and_read_download(&assets_index_file, assets_index_download, handler)?;
        let assets_index: Object = serde_json::from_reader(assets_index_reader)?;
        
        // For version <= 13w23b (1.6.1)
        let assets_resources = match assets_index.get("map_to_resources") {
            Some(&Value::Bool(val)) => val,
            Some(_) => return Err(Error::JsonSchema(format!("assets index: /map_to_resources must be a boolean"))),
            None => false,
        };

        // For 13w23b (1.6.1) < version <= 13w48b (1.7.2)
        let assets_virtual = match assets_index.get("virtual") {
            Some(&Value::Bool(val)) => val,
            Some(_) => return Err(Error::JsonSchema(format!("assets index: /virtual must be a boolean"))),
            None => false,
        };

        // Objects are mandatory...
        let Some(Value::Object(assets_objects)) = assets_index.get("objects") else {
            return Err(Error::JsonSchema(format!("assets index: /objects must be an object")));
        };

        let mut assets = Assets {
            version: assets_index_version.clone(),
            with_resources: assets_resources,
            with_virtual: assets_virtual,
            objects: HashMap::new(),
        };

        for (asset_path, asset_obj) in assets_objects.iter() {

            let Value::Object(asset_obj) = asset_obj else {
                return Err(Error::JsonSchema(format!("assets index: /objects/{asset_path} must be an object")));
            };

            let size_make_err = || Error::JsonSchema(format!("assets index: /objects/{asset_path}/size must be a number (32-bit unsigned)"));
            let hash_make_err = || Error::JsonSchema(format!("assets index: /objects/{asset_path}/hash must be a string (40 hex characters)"));

            let Some(Value::Number(asset_size)) = asset_obj.get("size") else {
                return Err(size_make_err());
            };

            let asset_size = asset_size.as_u64()
                .and_then(|size| u32::try_from(size).ok())
                .ok_or_else(size_make_err)?;
            
            let Some(Value::String(asset_hash)) = asset_obj.get("hash") else {
                return Err(hash_make_err());
            };

            let asset_hash = parse_hex_bytes::<20>(asset_hash)
                .ok_or_else(hash_make_err)?;

            assets.objects.insert(PathBuf::from(asset_path), Asset {
                sha1: asset_hash,
                size: asset_size,
            });

        }

        // Filter assets before checking ones to download.
        handler.filter_assets(self, &mut assets)?;

        // Now we check assets that needs to be downloaded...
        let mut asset_file = assets_dir.join("objects");
        let mut asset_file_name = String::new();

        for asset in assets.objects.values() {

            for byte in asset.sha1 {
                write!(asset_file_name, "{byte:02x}").unwrap();
            }

            let asset_hash_name = &asset_file_name[0..2];
            asset_file.push(asset_hash_name);
            asset_file.push(&asset_file_name);

            if self.check_file(&asset_file, Some(asset.size), None)? {
                downloads.push(Download {
                    url: format!("{RESOURCES_URL}{asset_hash_name}/{asset_file_name}"),
                    file: asset_file.clone(),
                    size: Some(asset.size),
                    sha1: Some(asset.sha1),
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
        metadata: &Object,
        hierarchy: &[Version],
        downloads: &mut Vec<Download>, 
        handler: &mut dyn Handler
    ) -> Result<PathBuf> {

        let jar_file = self.version_file(&hierarchy[0].name, "jar");

        if let Some(downloads_info) = metadata.get("downloads") {

            let Value::Object(downloads_info) = downloads_info else {
                return Err(Error::JsonSchema(format!("metadata: /downloads must be an object")));
            };

            if let Some(downloads_client) = downloads_info.get("client") {

                let Value::Object(downloads_client) = downloads_client else {
                    return Err(Error::JsonSchema(format!("metadata: /downloads/client must be an object")));
                };

                let download = parse_json_download(downloads_client)
                    .map_err(|err| Error::JsonSchema(format!("metadata: /downloads/client{err}")))?;

                if self.check_file(&jar_file, download.size, download.sha1)? {
                    downloads.push(download.to_owned(jar_file.to_owned(), false));
                }

            }

        }

        // If no download entry has been found, but the JAR exists, we use it.
        if !jar_file.is_file() {
            return Err(Error::JarNotFound);
        }
        
        handler.filter_jar(self, &jar_file)?;
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
        downloads: &mut Vec<Download>, 
        handler: &mut dyn Handler
    ) -> Result<()> {

        // Recursion order is important for libraries resolving, because the class path
        // order is important (for some corner cases with mod loaders). The root version's
        // libraries should be placed at the beginning of the hierarchy. Also, a library
        // defined in parent metadata should not override higher versions in hierarchy.
        for version in hierarchy {

            // Used for error formatting...
            let version_name = version.name.as_str();

            // Libraries object may be missing.
            let Some(libs) = version.metadata.get("libraries") else {
                continue;
            };

            let Value::Array(libs) = libs else {
                return Err(Error::JsonSchema(format!("metadata({version_name}): /libraries must be a list")));
            };

            for (lib_idx, lib) in libs.iter().enumerate() {

                let Value::Object(lib) = lib else {
                    return Err(Error::JsonSchema(format!("metadata({version_name}): /libraries/{lib_idx} must be an object")));
                };
                
                let lib_spec_err = || Error::JsonSchema(format!("metadata({version_name}): /libraries/{lib_idx}/name must be a string (library specifier)"));

                let Some(Value::String(lib_spec)) = lib.get("name") else {
                    return Err(lib_spec_err());
                };

                let lib_spec = LibrarySpecifier::from_str(lib_spec.clone())
                    .ok_or_else(lib_spec_err)?;

                // Start by applying rules before the actual parsing.
                // TODO: Continue parsing after that just to check syntax?
                if let Some(lib_rules) = lib.get("rules") {

                    let Value::Array(lib_rules) = lib_rules else {
                        return Err(Error::JsonSchema(format!("metadata({version_name}): /libraries/{lib_idx}/rules must be a list")));
                    };

                    // TODO: Interpret rules...

                }

                // Old metadata files provides a 'natives' mapping from OS to the classifier
                // specific for this OS, this kind of libs are "native libs", we need to
                // extract their dynamic libs into the "bin" directory before running.
                if let Some(lib_natives) = lib.get("natives") {

                    let Value::Object(lib_natives) = lib_natives else {
                        return Err(Error::JsonSchema(format!("metadata({version_name}): /libraries/{lib_idx}/natives must be an object")));
                    };

                    

                }

            }

        }

        Err(Error::UnsupportedOperation("resolve_libraries"))

    }

    fn check_and_read_download(&self,
        file: &Path,
        download: JsonDownload<'_>,
        handler: &mut dyn Handler,
    ) -> Result<File> {
        self.check_and_read_file(file, download.size, download.sha1, download.url, handler)
    }

    /// Ensure that a file exists from its download entry, checking that the file has the
    /// right size and SHA-1, if relevant. This will push the download to the handler and
    /// immediately flush the handler.
    fn check_and_read_file(&self, 
        file: &Path,
        size: Option<u32>,
        sha1: Option<Sha1Hash>,
        url: &str,
        handler: &mut dyn Handler,
    ) -> Result<File> {

        // If the file need to be (re)downloaded...
        if self.check_file(file, size, sha1)? {
            handler.download(&[Download {
                url: url.to_string(),
                file: file.to_path_buf(),
                size,
                sha1,
                executable: false,
            }])?;
        }

        // The handler should have checked it and it should be existing.
        match File::open(file) {
            Ok(reader) => Ok(reader),
            Err(e) if e.kind() == io::ErrorKind::NotFound =>
                unreachable!("handler returned no error but downloaded file is absent"),
            Err(e) => return Err(e.into()),
        }
        
    }

    /// Check if a file at a given path should be downloaded by checking the given 
    /// properties, this also returns true if the file doesn't exists.
    fn check_file(&self,
        file: &Path,
        size: Option<u32>,
        sha1: Option<Sha1Hash>,
    ) -> Result<bool> {

        if let Some(sha1) = sha1 {
            // If we want to check SHA-1 we need to open the file and compute it...
            match File::open(file) {
                Ok(mut reader) => {

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
                    reader.seek(SeekFrom::Start(0))?;
                    
                    Ok(false)

                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
                Err(e) => Err(e.into()),
            }
        } else {
            match (file.metadata(), size) {
                // File is existing and we want to check size...
                (Ok(metadata), Some(size)) => Ok(metadata.len() != size as u64),
                // File is existing but we don't have size to check, no need to download.
                (Ok(_metadata), None) => Ok(false),
                (Err(e), _) if e.kind() == io::ErrorKind::NotFound => Ok(true),
                (Err(e), _) => Err(e.into()),
            }
        }

    }

}

/// A handler is given when installing a version and allows altering the installation
/// process during the installation. It also provides various methods that are required
/// to provide missing version and download missing files.
pub trait Handler {

    /// Filter an individual version that have just been loaded from a file, this method
    /// should return false if the version should be requested again.
    fn filter_version(&mut self, installer: &Installer, version: &mut Version) -> Result<bool> {
        let _ = (installer, version);
        Ok(true)
    }

    /// When a version is missing, is it requested by calling this method. This method
    /// returns a [`Error::VersionNotFound`] by default. This method is responsible of
    /// writing the version metadata file if it's needed for it to be persistent.
    fn fetch_version(&mut self, installer: &Installer, version: &str) -> Result<Version> {
        let _ = installer;
        Err(Error::VersionNotFound(version.to_string()))
    }

    /// Filter the version hierarchy after full resolution.
    fn filter_hierarchy(&mut self, installer: &Installer, hierarchy: &mut Vec<Version>) -> Result<()> {
        let _ = (installer, hierarchy);
        Ok(())
    }

    /// Filter the merged metadata of the version hierarchy.
    fn filter_metadata(&mut self, installer: &Installer, metadata: &mut Object) -> Result<()> {
        let _ = (installer, metadata);
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

    /// Filter the jar file that will be used as the entry point to launching the game.
    /// It is not possible for now to modify the JAR file used.
    fn filter_jar(&mut self, installer: &Installer, jar_file: &Path) -> Result<()> {
        let _ = (installer, jar_file);
        Ok(())
    }

    /// Filter libraries after initial resolution.
    fn filter_libraries(&mut self, installer: &Installer, libraries: &mut HashMap<LibrarySpecifier, Library>) -> Result<()> {
        let _ = (installer, libraries);
        Ok(())
    }

    /// Download entries synchronously, this should be the preferred way to download a
    /// file as-is. When successful, this method should return the total bytes downloaded.
    /// 
    /// This method should not check if the file already exists, it should always
    /// download it and only then check size and SHA-1, if relevant.
    fn download(&mut self, entries: &[Download]) -> Result<usize> {
        let _ = entries;
        Err(Error::UnsupportedOperation("Handler::download"))
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
    pub name: String,
    /// The JSON metadata of the version, defaults to an empty object.
    pub metadata: Object,
}

/// Represent all the assets used for the game.
#[derive(Debug)]
pub struct Assets {
    /// The version of assets index.
    pub version: String,
    /// For version <= 13w23b (1.6.1)
    pub with_resources: bool,
    /// For 13w23b (1.6.1) < version <= 13w48b (1.7.2)
    pub with_virtual: bool,
    /// Assets objects mapped from their relative path to their object.
    pub objects: HashMap<PathBuf, Asset>,
}

#[derive(Debug)]
pub struct Asset {
    /// The SHA-1 hash of the content of this asset, it also defines its path inside the
    /// objects directory structure.
    pub sha1: Sha1Hash,
    /// Size of this asset in bytes.
    pub size: u32,
}

#[derive(Debug)]
pub struct Library {

}

/// A download entry that can be delayed until a call to [`Handler::flush_download`].
/// This download object borrows the URL and file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// Url of the file to download.
    pub url: String,
    /// Path to the file to ultimately download.
    pub file: PathBuf,
    /// Expected size of the file, checked after downloading, this use a `u32` because
    /// we are not downloading ubuntu ISO...
    pub size: Option<u32>,
    /// Expected SHA-1 of the file, checked after downloading.
    pub sha1: Option<Sha1Hash>,
    /// True if the file should be made executable on systems where its relevant to 
    /// later execute a binary.
    pub executable: bool,
}

/// A maven-style library specifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LibrarySpecifier {
    /// Internal buffer containing the whole specifier. This should follows the pattern 
    /// `group:artifact:version[:classifier][@extension]`.
    raw: String,
    /// Length of the group part in the specifier.
    group_len: NonZeroU16,
    /// Length of the artifact part in the specifier.
    artifact_len: NonZeroU16,
    /// Length of the version part in the specifier.
    version_len: NonZeroU16,
    /// Length of the classifier part in the specifier, if relevant.
    classifier_len: Option<NonZeroU16>,
    /// Length of the extension part in the specifier, if relevant.
    extension_len: Option<NonZeroU16>,
}

impl LibrarySpecifier {

    /// Parse the given library specifier and return it if successful.
    /// TODO: Move to FromStr trait.
    pub fn from_str(raw: String) -> Option<Self> {

        // FIXME: Remove "as"

        let mut split = raw.split('@');
        let raw0 = split.next()?;
        let extension_len = match split.next() {
            Some(s) => Some(NonZeroU16::new(s.len() as _)?),
            None => None,
        };

        if split.next().is_some() {
            return None;
        }

        let mut split = raw0.split(':');
        let group_len = NonZeroU16::new(split.next()?.len() as _)?;
        let artifact_len = NonZeroU16::new(split.next()?.len() as _)?;
        let version_len = NonZeroU16::new(split.next()?.len() as _)?;
        let classifier_len = match split.next() {
            Some(s) => Some(NonZeroU16::new(s.len() as _)?),
            None => None,
        };

        if split.next().is_some() {
            return None;
        }

        Some(Self {
            raw,
            group_len,
            artifact_len,
            version_len,
            classifier_len,
            extension_len,
        })

    }

    pub fn new(group: &str, artifact: &str, version: &str, classifier: Option<&str>, extension: Option<&str>) -> Self {
        
        let mut raw = format!("{group}:{artifact}:{version}");
        
        if let Some(classifier) = classifier {
            raw.push(':');
            raw.push_str(classifier);
        }

        if let Some(extension) = extension {
            raw.push('@');
            raw.push_str(extension);
        }

        Self {
            raw,
            group_len: NonZeroU16::new(group.len().try_into().expect("group too long")).expect("group empty"),
            artifact_len: NonZeroU16::new(artifact.len().try_into().expect("artifact too long")).expect("artifact empty"),
            version_len: NonZeroU16::new(version.len().try_into().expect("version too long")).expect("version empty"),
            classifier_len: classifier.map(|classifier| NonZeroU16::new(classifier.len().try_into().expect("classifier too long")).expect("classifier empty")),
            extension_len: extension.map(|extension| NonZeroU16::new(extension.len().try_into().expect("extension too long")).expect("extension empty")),
        }

    }

    /// Internal method to split the specifier in all of its component.
    #[inline(always)]
    fn split(&self) -> (&str, &str, &str, &str, &str) {
        let (group, rem) = self.raw.split_at(self.group_len.get() as usize);
        let (artifact, rem) = rem[1..].split_at(self.artifact_len.get() as usize);
        let (version, rem) = rem[1..].split_at(self.version_len.get() as usize);
        let (classifier, rem) = self.classifier_len.map(|len| rem[1..].split_at(len.get() as usize)).unwrap_or(("", rem));
        let (extension, rem) = self.extension_len.map(|len| rem[1..].split_at(len.get() as usize)).unwrap_or(("jar", rem));
        debug_assert!(rem.is_empty());
        (group, artifact, version, classifier, extension)
    }

    /// Return the group name of the library, never empty.
    #[inline]
    pub fn group(&self) -> &str {
        self.split().0
    }

    /// Return the artifact name of the library, never empty.
    #[inline]
    pub fn artifact(&self) -> &str {
        self.split().1
    }

    /// Return the version of the library, never empty.
    #[inline]
    pub fn version(&self) -> &str {
        self.split().2
    }

    /// Return the classifier of the library, empty if no specifier.
    #[inline]
    pub fn classifier(&self) -> &str {
        self.split().3
    }

    /// Return the extension of the library, never empty, defaults to "jar".
    #[inline]
    pub fn extension(&self) -> &str {
        self.split().4
    }

}

/// The error type for standard installer.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("json schema error: {0}")]
    JsonSchema(String),
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(&'static str),
    #[error("version not found: {0}")]
    VersionNotFound(String),
    #[error("jar not found")]
    JarNotFound,
}

/// Type alias for result with the install error type.
pub type Result<T> = StdResult<T, Error>;

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

/// Merge two version metadata JSON values.
fn merge_json_metadata(dst: &mut Object, src: &Object) {
    for (src_key, src_value) in src.iter() {
        if let Some(dst_value) = dst.get_mut(src_key) {
            match (dst_value, src_value) {
                (Value::Object(dst_object), Value::Object(src_object)) => 
                    merge_json_metadata(dst_object, src_object),
                (Value::Array(dst), Value::Array(src)) =>
                    dst.extend(src.iter().cloned()),
                _ => {}  // Do nothing
            }
        } else {
            dst.insert(src_key.clone(), src_value.clone());
        }
    }
}

/// Parse the given hex bytes string into the given destination slice, returning none if 
/// the input string cannot be parsed, is too short or too long.
fn parse_hex_bytes<const LEN: usize>(mut string: &str) -> Option<[u8; LEN]> {
    
    let mut dst = [0; LEN];
    for dst in &mut dst {
        if string.is_char_boundary(2) {

            let (num, rem) = string.split_at(2);
            string = rem;

            *dst = u8::from_str_radix(num, 16).ok()?;

        } else {
            return None;
        }
    }

    // Only successful if no string remains.
    string.is_empty().then_some(dst)

}

/// Parse a download file from its JSON value, expected to be an object that contains a
/// `url` string, and optionally a number `size` and a string`sha1`. 
fn parse_json_download<'json>(object: &'json Object) -> StdResult<JsonDownload<'json>, String> {

    let Some(Value::String(url)) = object.get("url") else {
        return Err(format!("/url must be a string"));
    };

    let mut download = JsonDownload {
        url: url.as_str(),
        size: None,
        sha1: None,
    };

    if let Some(size) = object.get("size") {

        let make_err = || format!("/size must be a number (32-bit unsigned)");

        let Value::Number(size) = size else {
            return Err(make_err());
        };
    
        let size = size.as_u64()
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(make_err)?;
        
        download.size = Some(size);

    }

    if let Some(sha1) = object.get("sha1") {

        let make_err = || format!("/sha1 must be a string (40 hex characters)");

        let Value::String(sha1) = sha1 else {
            return Err(make_err());
        };

        let sha1 = parse_hex_bytes::<20>(sha1)
            .ok_or_else(make_err)?;

        download.sha1 = Some(sha1);

    }

    Ok(download)

}

/// Internal structure to parse a JSON download entry.
#[derive(Debug)]
struct JsonDownload<'json> {
    url: &'json str,
    size: Option<u32>,
    sha1: Option<Sha1Hash>,
}

impl<'json> JsonDownload<'json> {

    pub fn to_owned(&self, file: PathBuf, executable: bool) -> Download {
        Download {
            url: self.url.to_string(),
            file,
            size: self.size,
            sha1: self.sha1,
            executable,
        }
    }

}
