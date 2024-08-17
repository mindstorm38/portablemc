//! Standard installation procedure.

pub mod serde;

use std::io::{self, BufReader, Seek, SeekFrom};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::process::Command;
use std::fmt::Write as _;
use std::{env, os};

use sha1::{Digest, Sha1};

use crate::download::{self, Batch, Entry, EntrySource};
use crate::path::{PathExt, PathBufExt};
use crate::gav::Gav;


/// Base URL for downloading game's assets.
const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";

/// The URL to meta manifest for Mojang-provided JVMs. 
const JVM_META_MANIFEST_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";


/// Standard installer handle to install versions, this object is just the configuration
/// of the installer when a version will be installed, such as directories to install 
/// into, the installation will not mutate this object.
#[derive(Debug)]
pub struct Installer {
    /// The directory where versions are stored.
    pub versions_dir: PathBuf,
    /// The directory where assets, assets index, cached skins and logs config are stored.
    /// Note that this directory stores caches player skins, so this is the only 
    /// directory where the client will need to write, and so it needs the permission
    /// to do so.
    pub assets_dir: PathBuf,
    /// The directory where Mojang-provided JVM has been installed.
    pub jvm_dir: PathBuf,
    /// The directory where libraries are stored, organized like a maven repository.
    pub libraries_dir: PathBuf,
    /// When enabled, all assets are strictly checked against their expected SHA-1,
    /// this is disabled by default because it's heavy on CPU.
    pub strict_assets_check: bool,
    /// When enabled, all libraries are strictly checked against their expected SHA-1,
    /// this is disabled by default because it's heavy on CPU.
    pub strict_libraries_check: bool,
    /// When enabled, all files from Mojang-provided JVMs are strictly checked against
    /// their expected SHA-1, this is disabled by default because it's heavy on CPU.
    pub strict_jvm_check: bool,
    /// The OS name used when applying rules for the version metadata.
    pub os_name: String,
    /// The OS system architecture name used when applying rules for version metadata.
    pub os_arch: String,
    /// The OS version name used when applying rules for version metadata.
    pub os_version: String,
    /// The OS bits replacement for "${arch}" replacement of library natives.
    pub os_bits: String,
    /// The platform used for selecting from Mojang-provided JVMs.
    pub jvm_platform: Option<String>,
    /// The policy for finding a JVM to run the game on.
    pub jvm_policy: JvmPolicy,
}

impl Installer {

    /// Create a new installer with default configuration and pointing to defaults
    /// directories.
    pub fn new() -> Self {
        let dir = default_main_dir().unwrap();
        Self::with_dir(dir)
    }

    /// Create a new installer with default configuration and pointing to given 
    /// directories.
    pub fn with_dir(main_dir: PathBuf) -> Self {
        Self {
            versions_dir: main_dir.join("versions"),
            assets_dir: main_dir.join("assets"),
            libraries_dir: main_dir.join("libraries"),
            jvm_dir: main_dir.join("jvm"),
            strict_assets_check: false,
            strict_libraries_check: false,
            strict_jvm_check: false,
            os_name: default_os_name().unwrap(),
            os_arch: default_os_arch().unwrap(),
            os_version: default_os_version().unwrap(),
            os_bits: default_os_bits().unwrap(),
            jvm_platform: default_jvm_platform(),
            jvm_policy: JvmPolicy::SystemThenMojang,
        }
    }

    /// Ensure that a the given version, from its id, is properly installed.
    pub fn install(&self, mut handler: impl Handler, id: &str) -> Result<Installed> {
        
        let mut batch = Batch::new();
        let mut features = HashSet::new();
        handler.handle_standard_event(Event::FeaturesLoaded { features: &mut features });

        let hierarchy = self.load_hierarchy(&mut handler, id)?;
        let client_file = self.load_client(&mut handler, &hierarchy, &mut batch)?;
        let lib_files = self.load_libraries(&mut handler, &hierarchy, &features, &mut batch)?;
        let logger_config = self.load_logger(&mut handler, &hierarchy, &mut batch)?;
        let assets = self.load_assets(&mut handler, &hierarchy, &mut batch)?;
        let jvm = self.load_jvm(&mut handler, &hierarchy, &mut batch)?;

        if !batch.is_empty() {
            handler.handle_standard_event(Event::ResourcesDownloading {  });
            batch.download(handler.as_download_dyn())?;
            handler.handle_standard_event(Event::ResourcesDownloaded {  });
        }

        if let Some(assets) = &assets {
            self.finalize_assets(assets)?;
        }

        self.finalize_jvm(&jvm)?;

        // Resolve arguments from the hierarchy of versions.
        let mut jvm_args = Vec::new();
        let mut game_args = Vec::new();
        
        for version in &hierarchy {
            if let Some(version_args) = &version.metadata.arguments {
                self.check_args(&mut jvm_args, &version_args.jvm, &features, None);
                self.check_args(&mut game_args, &version_args.game, &features, None);
            } else if let Some(version_legacy_args) = &version.metadata.legacy_arguments {
                game_args.extend(version_legacy_args.split_whitespace().map(str::to_string));
            }
        }

        // The logger configuration is an additional JVM argument.
        if let Some(logger_config) = &logger_config {
            
            let logger_file = logger_config.file.canonicalize()
                .map_err(Error::new_io)?;

            jvm_args.push(logger_config.argument.replace("${path}", &logger_file.to_string_lossy()));

        }

        Ok(Installed {
            hierarchy,
            client_file,
            class_files: lib_files.class_files,
            natives_files: lib_files.natives_files,
            assets_virtual_dir: None,
            jvm_args,
            game_args,
        })

    }

    /// Internal function that loads the version hierarchy from their JSON metadata files.
    fn load_hierarchy(&self, 
        handler: &mut impl Handler, 
        root_id: &str
    ) -> Result<Vec<Version>> {

        handler.handle_standard_event(Event::HierarchyLoading { root_id });

        let mut hierarchy = Vec::new();
        let mut current_id = Some(root_id.to_string());

        while let Some(load_id) = current_id.take() {
            let version = self.load_version(handler, load_id)?;
            if let Some(next_id) = &version.metadata.inherits_from {
                current_id = Some(next_id.clone());
            }
            hierarchy.push(version);
        }

        handler.handle_standard_event(Event::HierarchyLoaded { hierarchy: &mut hierarchy });

        Ok(hierarchy)

    }

    /// Internal function that loads a version from its JSON metadata file.
    fn load_version(&self, 
        handler: &mut impl Handler, 
        id: String
    ) -> Result<Version> {

        if id.is_empty() {
            return Err(Error::VersionNotFound { id });
        }

        let dir = self.versions_dir.join(&id);
        let file = dir.join_with_extension(&id, "json");

        handler.handle_standard_event(Event::VersionLoading { id: &id, file: &file });

        loop {

            let reader = match File::open(&file) {
                Ok(reader) => BufReader::new(reader),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {

                    let mut retry = false;
                    handler.handle_standard_event(Event::VersionNotFound { id: &id, file: &file, error: None, retry: &mut retry });

                    if retry {
                        continue;
                    } else {
                        // If not retried, we return a version not found error.
                        return Err(Error::VersionNotFound { id });
                    }

                }
                Err(e) => return Err(Error::new_io_file(e, file))
            };

            let mut deserializer = serde_json::Deserializer::from_reader(reader);
            let mut metadata: serde::VersionMetadata = match serde_path_to_error::deserialize(&mut deserializer) {
                Ok(obj) => obj,
                Err(e) => return Err(Error::new_json_file(e, file)),
            };

            handler.handle_standard_event(Event::VersionLoaded { id: &id, file: &file, metadata: &mut metadata });

            break Ok(Version {
                id,
                dir,
                metadata,
            });

        }

    }

    /// Load the entry point version JAR file.
    fn load_client(&self, 
        handler: &mut impl Handler, 
        hierarchy: &[Version], 
        batch: &mut Batch,
    ) -> Result<PathBuf> {
        
        let root_version = &hierarchy[0];
        let client_file = root_version.dir.join_with_extension(&root_version.id, "jar");

        handler.handle_standard_event(Event::ClientLoading {  });

        let dl = hierarchy.iter()
            .filter_map(|version| version.metadata.downloads.get("client"))
            .next();

        if let Some(dl) = dl {
            let check_client_sha1 = dl.sha1.as_deref().filter(|_| self.strict_libraries_check);
            if !check_file(&client_file, dl.size, check_client_sha1)? {
                batch.push(EntrySource::from(dl).with_file(client_file.clone()));
            }
        } else if !client_file.is_file() {
            return Err(Error::ClientNotFound);
        }

        handler.handle_standard_event(Event::ClientLoaded {  });
        Ok(client_file)

    }

    /// Load libraries required to run the game.
    fn load_libraries(&self,
        handler: &mut impl Handler,
        hierarchy: &[Version], 
        features: &HashSet<String>,
        batch: &mut Batch,
    ) -> Result<LibraryFiles> {

        handler.handle_standard_event(Event::LibrariesLoading {});

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
                    let Some(classifier) = lib_natives.get(&self.os_name) else {
                        continue;
                    };

                    // If we find a arch replacement pattern, we must replace it with
                    // the target architecture bit-ness (32, 64).
                    const ARCH_REPLACEMENT_PATTERN: &str = "${arch}";
                    if let Some(pattern_idx) = lib_gav.classifier().find(ARCH_REPLACEMENT_PATTERN) {
                        let mut classifier = classifier.clone();
                        classifier.replace_range(pattern_idx..pattern_idx + ARCH_REPLACEMENT_PATTERN.len(), &self.os_bits);
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
                    lib_obj.source = Some(EntrySource::from(&lib_dl.download));
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
                    
                    lib_obj.source = Some(EntrySource {
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

        handler.handle_standard_event(Event::LibrariesLoaded { libraries: &mut libraries });

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
                let check_source_sha1 = source.sha1.as_ref().filter(|_| self.strict_libraries_check);
                if !check_file(&lib_file, source.size, check_source_sha1)? {
                    batch.push(source.with_file(lib_file.clone()));
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

        handler.handle_standard_event(Event::LibrariesVerified {
            class_files: &lib_files.class_files,
            natives_files: &lib_files.natives_files,
        });

        Ok(lib_files)

    }

    /// Load libraries required to run the game.
    fn load_logger(&self,
        handler: &mut impl Handler,
        hierarchy: &[Version], 
        batch: &mut Batch,
    ) -> Result<Option<LoggerConfig>> {

        let config = hierarchy.iter()
            .filter_map(|version| version.metadata.logging.get("client"))
            .next();

        let Some(config) = config else {
            handler.handle_standard_event(Event::LoggerAbsent {  });
            return Ok(None);
        };

        handler.handle_standard_event(Event::LoggerLoading { id: &config.file.id });

        let file = self.assets_dir
            .join("log_configs")
            .joined(config.file.id.as_str());

        if !check_file(&file, config.file.download.size, config.file.download.sha1.as_deref())? {
            batch.push(EntrySource::from(&config.file.download).with_file(file.clone()));
        }

        handler.handle_standard_event(Event::LoggerLoaded { id: &config.file.id });

        Ok(Some(LoggerConfig {
            kind: config.r#type,
            argument: config.argument.clone(),
            file,
        }))

    }

    /// Load and verify all assets of the game.
    fn load_assets(&self, 
        handler: &mut impl Handler, 
        hierarchy: &[Version], 
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
            handler.handle_standard_event(Event::AssetsAbsent {  });
            return Ok(None);
        };

        handler.handle_standard_event(Event::AssetsLoading { id: index_info.id });

        // Resolve all used directories and files...
        let indexes_dir = self.assets_dir.join("indexes");
        let index_file = indexes_dir.join_with_extension(index_info.id, "json");

        // All modern version metadata have download information attached to the assets
        // index identifier, we check the file against the download information and then
        // download this single file. If the file has no download info
        if let Some(dl) = index_info.download {
            if !check_file(&index_file, dl.size, dl.sha1.as_deref())? {
                EntrySource::from(dl)
                    .with_file(index_file.clone())
                    .download(handler.as_download_dyn())?;
            }
        }

        let reader = match File::open(&index_file) {
            Ok(reader) => BufReader::new(reader),
            Err(e) if e.kind() == io::ErrorKind::NotFound =>
                return Err(Error::AssetsNotFound { id: index_info.id.to_owned() }),
            Err(e) => 
                return Err(Error::new_io_file(e, index_file))
        };

        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let asset_index: serde::AssetIndex = match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(obj) => obj,
            Err(e) => return Err(Error::new_json_file(e, index_file))
        };
        
        handler.handle_standard_event(Event::AssetsLoaded { id: index_info.id, index: &asset_index });

        // Now we check assets that needs to be downloaded...
        let objects_dir = self.assets_dir.join("objects");
        let mut asset_file_name = String::new();
        let mut unique_hashes = HashSet::new();

        let mut assets = Assets {
            id: index_info.id.to_string(),
            mapping: None,
        };

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
                batch.push(EntrySource {
                    url: format!("{RESOURCES_URL}{asset_hash_prefix}/{asset_file_name}").into_boxed_str(),
                    size: Some(asset.size),
                    sha1: Some(*asset.hash),
                }.with_file(asset_hash_file));
            }

        }

        handler.handle_standard_event(Event::AssetsVerified { id: index_info.id, index: &asset_index });

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

        for object in &mapping.objects {
            let virtual_file = mapping.virtual_dir.join(&object.rel_file);
            fs::hard_link(&object.object_file, virtual_file).map_err(Error::new_io)?;
        }

        Ok(())

    }
    
    /// The goal of this step is to find a valid JVM to run the game on.
    fn load_jvm(&self, 
        handler: &mut impl Handler, 
        hierarchy: &[Version], 
        batch: &mut Batch,
    ) -> Result<Jvm> {

        let version = hierarchy.iter()
            .find_map(|version| version.metadata.java_version.as_ref());

        let mut jvm;
        
        // We simplify the code with this condition and duplicated match, because in the
        // 'else' case we can simplify any policy that contains Mojang and System to
        // System, because we don't have instructions for finding Mojang version.
        if let Some(version) = version {
            
            let major_version_num = version.major_version;
            let major_version = if major_version_num < 8 { 
                format!("1.{major_version_num}.")
            } else {
                format!("{major_version_num}.")
            };

            let distribution = version.component.as_deref().unwrap_or("jre-legacy");

            handler.handle_standard_event(Event::JvmLoading { 
                major_version: Some(major_version_num),
            });

            match self.jvm_policy {
                JvmPolicy::Static { ref file, strict_check } => 
                    jvm = self.load_static_jvm(handler, file.clone(), strict_check, Some(&major_version))?,
                JvmPolicy::System => 
                    jvm = self.load_system_jvm(handler, Some(&major_version))?,
                JvmPolicy::Mojang => 
                    jvm = self.load_mojang_jvm(handler, distribution, batch)?,
                JvmPolicy::SystemThenMojang => {
                    jvm = self.load_system_jvm(handler, Some(&major_version))?;
                    if jvm.is_none() {
                        jvm = self.load_mojang_jvm(handler, distribution, batch)?;
                    }
                }
                JvmPolicy::MojangThenSystem => {
                    jvm = self.load_mojang_jvm(handler, distribution, batch)?;
                    if jvm.is_none() {
                        jvm = self.load_system_jvm(handler, Some(&major_version))?;
                    }
                }
            }
            
            if jvm.is_none() {
                return Err(Error::JvmNotFound { major_version: Some(major_version_num) });
            }

        } else {

            handler.handle_standard_event(Event::JvmLoading { 
                major_version: None,
            });

            jvm = match self.jvm_policy {
                JvmPolicy::Static { ref file, strict_check } => 
                    self.load_static_jvm(handler, file.clone(), strict_check, None)?,
                JvmPolicy::System | 
                JvmPolicy::SystemThenMojang | 
                JvmPolicy::MojangThenSystem => 
                    self.load_system_jvm(handler, None)?,
                JvmPolicy::Mojang => None,
            };

            if jvm.is_none() {
                return Err(Error::JvmNotFound { major_version: None });
            }

        }

        // This has been checked above.
        let jvm = jvm.unwrap();

        handler.handle_standard_event(Event::JvmLoaded { 
            file: &jvm.file, 
            version: jvm.version.as_deref(),
        });

        Ok(jvm)

    }

    /// Load the JVM by checking its version,
    fn load_static_jvm(&self,
        handler: &mut impl Handler,
        file: PathBuf,
        strict_check: bool,
        major_version: Option<&str>,
    ) -> Result<Option<Jvm>> {

        // If the given JVM don't work, this returns None.
        let Some(jvm) = self.find_jvm_version(file) else {
            return Ok(None)
        };

        // Only check if both major version is required and JVM version has been checked.
        if let Some(major_version) = major_version {
            if let Some(jvm_version) = jvm.version.as_deref() {
                if !jvm_version.starts_with(major_version) {
                    
                    handler.handle_standard_event(Event::JvmVersionRejected { 
                        file: &jvm.file,
                        version: Some(jvm_version),
                    });
                    
                    // Only return no JVM is strict checking is enabled.
                    if strict_check {
                        return Ok(None);
                    }

                }
            } else if strict_check {

                // If the JVM version was not found by running the JVM but strict 
                // checking is enabled we reject this JVM because we can't guarantee
                // the JVM version.
                handler.handle_standard_event(Event::JvmVersionRejected { 
                    file: &jvm.file,
                    version: None,
                });

                return Ok(None);

            }
        }

        Ok(Some(jvm))

    }

    /// Try to find a JVM executable installed on the system in standard paths.
    fn load_system_jvm(&self,
        handler: &mut impl Handler,
        major_version: Option<&str>,
    ) -> Result<Option<Jvm>> {

        let mut candidates = Vec::new();
        let exec_name = jvm_exec_name();

        // Check every JVM available in PATH.
        if let Some(path) = env::var_os("PATH") {
            for mut path in env::split_paths(&path) {
                path.push(exec_name);
                if path.is_file() {
                    candidates.push(path);
                }
            }
        }

        // On some Linux distributions (Arch) the different JVMs are in /usr/lib/jvm/
        if cfg!(target_os = "linux") {
            if let Ok(read_dir) = fs::read_dir("/usr/lib/jvm/") {
                for entry in read_dir {
                    let Ok(entry) = entry else { continue };
                    let path = entry.path()
                        .joined("bin")
                        .joined(exec_name);
                    if path.is_file() {
                        candidates.push(path);
                    }
                }
            }
        }

        for jvm_file in candidates {
            
            let Some(jvm) = self.find_jvm_version(jvm_file) else {
                continue
            };

            if let Some(major_version) = major_version.as_deref() {
                
                // If we have a major version requirement but the JVM version couldn't
                // be determined, we skip this candidate.
                let Some(jvm_version) = jvm.version.as_deref() else {
                    handler.handle_standard_event(Event::JvmVersionRejected { 
                        file: &jvm.file, 
                        version: None,
                    });
                    continue
                };

                if !jvm_version.starts_with(major_version) {
                    handler.handle_standard_event(Event::JvmVersionRejected { 
                        file: &jvm.file, 
                        version: Some(jvm_version),
                    });
                    continue;
                }

            }

            return Ok(Some(jvm));
            
        }

        Ok(None)

    }

    fn load_mojang_jvm(&self,
        handler: &mut impl Handler,
        distribution: &str,
        batch: &mut Batch,
    ) -> Result<Option<Jvm>> {

        // On Linux, only glibc dynamic linkage is supported by Mojang-provided JVMs.
        if cfg!(target_os = "linux") && !cfg!(target_feature = "crt-static") {
            handler.handle_standard_event(Event::JvmDynamicCrtUnsupported {  });
        }

        // If we don't have JVM platform this means that we can't load Mojang JVM.
        let Some(jvm_platform) = self.jvm_platform.as_deref() else {
            handler.handle_standard_event(Event::JvmPlatformUnsupported {  });
            return Ok(None);
        };

        // Start by ensuring that we have a cached version of the JVM meta-manifest.
        let meta_manifest_entry = Entry::new_cached(JVM_META_MANIFEST_URL);
        let meta_manifest_file = meta_manifest_entry.file.to_path_buf();
        meta_manifest_entry.download(&mut *handler)?;

        let reader = match File::open(&meta_manifest_file) {
            Ok(reader) => BufReader::new(reader),
            Err(e) => return Err(Error::new_io_file(e, meta_manifest_file)),
        };

        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let meta_manifest: serde::JvmMetaManifest = match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(obj) => obj,
            Err(e) => return Err(Error::new_json_file(e, meta_manifest_file)),
        };

        let Some(meta_platform) = meta_manifest.platforms.get(jvm_platform) else {
            handler.handle_standard_event(Event::JvmPlatformUnsupported {  });
            return Ok(None);
        };

        let Some(meta_distribution) = meta_platform.distributions.get(distribution) else {
            handler.handle_standard_event(Event::JvmDistributionNotFound {  });
            return Ok(None);
        };

        // We take the first variant for now.
        let Some(meta_variant) = meta_distribution.variants.get(0) else {
            handler.handle_standard_event(Event::JvmDistributionNotFound {  });
            return Ok(None);
        };

        let dir = self.jvm_dir.join(distribution);
        let manifest_file = self.jvm_dir.join_with_extension(distribution, "json");
        let bin_file = if cfg!(target_os = "macos") {
            dir.join("jre.bundle/Contents/Home/bin/java")
        } else {
            dir.join("bin").joined(jvm_exec_name())
        };

        // Check the manifest, download it, read and parse it...
        if !check_file(&manifest_file, meta_variant.manifest.size, meta_variant.manifest.sha1.as_deref())? {
            EntrySource::from(&meta_variant.manifest)
                .with_file(manifest_file.clone())
                .download(handler.as_download_dyn())?;
        }

        let reader = match File::open(&manifest_file) {
            Ok(reader) => BufReader::new(reader),
            Err(e) => return Err(Error::new_io_file(e, manifest_file)),
        };

        let mut deserializer = serde_json::Deserializer::from_reader(reader);
        let manifest: serde::JvmManifest = match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(obj) => obj,
            Err(e) => return Err(Error::new_json_file(e, manifest_file)),
        };

        let mut mojang_jvm = MojangJvm::default();
        
        // Here we only check files because it's too early to assert symlinks.
        for (rel_file, manifest_file) in &manifest.files {

            // TODO: We could optimize this repeated allocation maybe.
            let rel_file = Path::new(rel_file);
            let file = dir.join(rel_file);

            match manifest_file {
                serde::JvmManifestFile::Directory => {}
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
                        batch.push(EntrySource::from(dl).with_file(file));
                    }

                }
                serde::JvmManifestFile::Link { 
                    target
                } => {

                    // The .parent() function returns an empty string for parent of 
                    // relative files, this should never return None because the 
                    // relative file should be _relative_.
                    let Some(rel_parent_dir) = rel_file.parent() else {
                        continue
                    };

                    let target_file = rel_parent_dir.join(target);
                    
                    mojang_jvm.links.push(MojangJvmLink {
                        file: file.into_boxed_path(),
                        target_file: target_file.into_boxed_path(),
                    });

                }
            }

        }

        Ok(Some(Jvm {
            file: bin_file,
            version: Some(meta_variant.version.name.clone()),
            mojang: Some(mojang_jvm),
        }))

    }

    /// Find the version of a JVM given its path. It returns none if the JVM doesn't
    /// work.
    fn find_jvm_version(&self, file: PathBuf) -> Option<Jvm> {

        // Try to execute JVM executable to get the version, ignore any error.
        // TODO: Timeout on command exec?
        let output = Command::new(&file)
            .arg("-version")
            .output()
            .ok()?;

        // The standard doc says that -version outputs version on stderr. This
        // argument -version is also practical because the version is given between
        // double quotes.
        let output = String::from_utf8(output.stderr).ok()?;

        // Parse the Java version from its output, the first line with quotes.
        Some(Jvm {
            file,
            version: output.lines()
                .flat_map(|line| line.split_once('"'))
                .flat_map(|(_, line)| line.split_once('"'))
                .map(|(version, _)| version)
                .next()
                .map(str::to_string),
            mojang: None,
        })

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
                .map_err(|e| Error::new_io_file(e, exec_file.to_path_buf()))?
                .permissions();

            // Set executable permission for every owner/group/other with read access.
            let mode = perm.mode();
            let new_mode = mode | ((mode & 0o444) >> 2);
            if new_mode != mode {
                perm.set_mode(new_mode);
            }

            fs::set_permissions(exec_file, perm)
                .map_err(|e| Error::new_io_file(e, exec_file.to_path_buf()))?;
            
        }

        // On Unix we simply use a symlink, on other systems (Windows) we hard link,
        // this act like a copy but is way cheaper.
        for link in &mojang_jvm.links {
            
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link.target_file, &link.file).map_err(Error::new_io)?;

            #[cfg(not(unix))]
            fs::hard_link(&link.target_file, &link.file).map_err(Error::new_io)?;

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

        if let Some(name) = &rule_os.name {
            if name != &self.os_name {
                return false;
            }
        }

        if let Some(arch) = &rule_os.arch {
            if arch != &self.os_arch {
                return false;
            }
        }

        if let Some(version) = &rule_os.version {
            if !version.is_match(&self.os_version) {
                return false;
            }
        }

        true

    }

}

/// Handler for events happening when installing.
pub trait Handler: download::Handler {

    /// Handle an even from the installer.
    fn handle_standard_event(&mut self, event: Event);

    fn as_standard_dyn(&mut self) -> &mut dyn Handler 
    where Self: Sized {
        self
    }

}

/// Blanket implementation that does nothing.
impl Handler for () {
    fn handle_standard_event(&mut self, event: Event) {
        let _ = event;
    }
}

impl<H: Handler + ?Sized> Handler for  &'_ mut H {
    fn handle_standard_event(&mut self, event: Event) {
        (*self).handle_standard_event(event)
    }
}

/// An event produced by the installer that can be handled by the install handler.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    /// Features for rules have been loaded, the handler can still modify them. 
    FeaturesLoaded {
        features: &'a mut HashSet<String>,
    },
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
        retry: &'a mut bool,
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
    /// The JVM will be loaded, depending on the policy configured in the installer. An
    /// optional major version may be required by the version.
    JvmLoading {
        major_version: Option<u32>,
    },
    /// A JVM has been rejected because its version is invalid or if the version has 
    /// not been detected but it's required in that context. 
    JvmVersionRejected {
        file: &'a Path,
        version: Option<&'a str>,
    },
    /// The system runs on Linux and has C runtime not dynamically linked (static, musl
    /// for example), suggesting that your system doesn't provide dynamic C runtime 
    /// (glibc), but this is not provided by Mojang. 
    JvmDynamicCrtUnsupported { },
    /// When trying to find a Mojang JVM to install, your operating system and 
    /// architecture are not supported.
    JvmPlatformUnsupported { },
    /// When trying to find a Mojang JVM to install, your operating sustem and 
    /// architecture are supported but no distribution (the java version packaged and
    /// distributed by Mojang) has been found.
    JvmDistributionNotFound { },
    /// The JVM has been loaded to the given version.
    JvmLoaded {
        file: &'a Path,
        version: Option<&'a str>,
    },
    ResourcesDownloading {},
    ResourcesDownloaded {},
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
    /// No JVM was found when installing the version, this depends on installer policy.
    #[error("jvm not found")]
    JvmNotFound {
        major_version: Option<u32>,
    },
    /// A generic system's IO error with optional file source.
    #[error("io: {error} @ {file:?}")]
    Io {
        #[source]
        error: io::Error,
        file: Option<Box<Path>>,
    },
    /// A JSON deserialization error with a file source.
    #[error("json: {error} @ {file}")]
    Json {
        #[source]
        error: serde_path_to_error::Error<serde_json::Error>,
        file: Box<Path>,
    },
    /// Download error, associating its failed download entry to the download error.
    #[error("download: {0}")]
    Download(#[from] download::Error),
    // #[error("reqwest: {0}")]
    // Reqwest(#[from] reqwest::Error),
}

/// Type alias for a result with the standard error type.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    
    #[inline]
    pub fn new_io(error: io::Error) -> Self {
        Self::Io { error, file: None }
    }
    
    #[inline]
    pub fn new_io_file(error: io::Error, file: impl Into<Box<Path>>) -> Self {
        Self::Io { error, file: Some(file.into()) }
    }
    
    #[inline]
    pub fn new_json_file(error: serde_path_to_error::Error<serde_json::Error>, file: impl Into<Box<Path>>) -> Self {
        Self::Json { error, file: file.into() }
    }

}

/// The policy for finding or installing the JVM executable to be used for launching
/// the game.
#[derive(Debug, Clone)]
pub enum JvmPolicy {
    /// The path to the JVM executable is given and will be used for sure. If the version
    /// needs a specific JVM major version, it is checked and a warning is triggered 
    /// ([`Event::JvmCandidateRejected`]) to notify that the version may not be suited 
    /// for that version, but this doesn't error out.
    Static {
        /// Path to the executable JVM file.
        file: PathBuf,
        /// True to error if the JVM has no version found.
        strict_check: bool,
    },
    /// The installer will try to find a suitable JVM executable in the path, searching
    /// a `java` (or `javaw.exe` on Windows) executable. On operating systems where it's
    /// supported, this will also check for known directories (on Arch for exemple).
    /// If the version needs a specific JVM major version, each candidate executable is 
    /// checked and a warning is triggered to notify that the version is not suited.
    /// Invalid versions are not kept, and if no valid version is found at the end then
    /// a [`Error::JvmNotFound`] error is returned.
    System,
    /// The installer will try to find a suitable JVM to install from Mojang-provided
    /// distributions, if no JVM is available for the platform (`jvm_platform` on the
    /// installer) and for the required distribution then a 
    Mojang,
    /// The installer search system and then mojang as a fallback.
    SystemThenMojang,
    /// The installer search Mojang and then system as a fallback.
    MojangThenSystem,
}

/// Represent a loaded version.
#[derive(Debug, Clone)]
pub struct Version {
    /// Identifier of this version.
    pub id: String,
    /// Directory of that version, where metadata is stored with the JAR file.
    pub dir: PathBuf,
    /// The loaded metadata of the version.
    pub metadata: serde::VersionMetadata,
}

/// Represent a loaded library.
#[derive(Debug, Clone)]
pub struct Library {
    /// GAV for this library.
    pub gav: Gav,
    /// The path to install the library at, relative to the libraries directory, by 
    /// default it will be derived from the library specifier.
    pub path: Option<PathBuf>,
    /// An optional download entry source for this library if it is missing.
    pub source: Option<download::EntrySource>,
    /// True if this contains natives that should be extracted into the binaries 
    /// directory before launching the game, instead of being in the class path.
    pub natives: bool,
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
/// - Virtual assets has been used between 13w23b (excluded) and 13w48b (1.7.2).
/// - Resource mapped assets has been used for versions 13w23b and anterior.
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

/// The asse
#[derive(Debug)]
struct AssetObject {
    rel_file: Box<Path>,
    object_file: Box<Path>,
}

/// Internal resolved libraries file paths.
#[derive(Debug, Default)]
struct LibraryFiles {
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

/// Internal resolved JVM.
#[derive(Debug)]
struct Jvm {
    file: PathBuf,
    version: Option<String>,
    mojang: Option<MojangJvm>,
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

/// Collection of the installation environment of a version, return by the install 
/// function, this contains all informations to launch any number of instances from this
/// installation.
#[derive(Debug, Clone)]
pub struct Installed {
    /// The hierarchy of versions and their associated loaded metadata.
    pub hierarchy: Vec<Version>,
    /// The path to the client JAR file.
    pub client_file: PathBuf,
    /// The list of class files.
    pub class_files: Vec<PathBuf>,
    /// The list of natives files to symlink (copy if not possible) and archives to 
    /// extract in the bin directory.
    pub natives_files: Vec<PathBuf>,
    /// If the assets index has been mapped into the virtual dir then we have the path
    /// here, .
    pub assets_virtual_dir: Option<PathBuf>,
    /// List of JVM arguments (before the main class in the command line).
    pub jvm_args: Vec<String>,
    /// List of game arguments (after the main class in the command line).
    pub game_args: Vec<String>,
}

impl Installed {

    

}

/// Check if a file at a given path has the corresponding properties (size and/or SHA-1), 
/// returning true if it is valid, so false is returned anyway if the file doesn't exists.
pub(crate) fn check_file(
    file: &Path,
    size: Option<u32>,
    sha1: Option<&[u8; 20]>,
) -> Result<bool> {
    check_file_inner(file, size, sha1).map_err(|e| Error::new_io_file(e, file.to_path_buf()))
}

fn check_file_inner(
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
fn default_os_name() -> Option<String> {
    Some(match env::consts::OS {
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
fn default_os_arch() -> Option<String> {
    Some(match env::consts::ARCH {
        "x86" => "x86",
        "x86_64" => "x86_64",
        "arm" => "arm32",
        "aarch64" => "arm64",
        _ => return None
    }.to_string())
}

/// Return the default OS version name for rules.
fn default_os_version() -> Option<String> {
    use os_info::Version;
    match os_info::get().version() {
        Version::Unknown => None,
        version => Some(version.to_string())
    }
}

/// Return the default OS version name for rules.
fn default_os_bits() -> Option<String> {
    match env::consts::ARCH {
        "x86" | "arm" => Some("32".to_string()),
        "x86_64" | "aarch64" => Some("64".to_string()),
        _ => return None
    }
}

/// Return the default JVM platform to install from Mojang-provided ones.
fn default_jvm_platform() -> Option<String> {
    Some(match (env::consts::OS, env::consts::ARCH) {
        ("macos", "x86_64") => "mac-os",
        ("macos", "aarch64") => "mac-os-arm64",
        ("linux", "x86") => "linux-i386",
        ("linux", "x86_64") => "linux",
        ("windows", "x86") => "windows-x86",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        _ => return None,
    }.to_string())
}

/// Return the JVM exec file name. 
#[inline]
fn jvm_exec_name() -> &'static str {
    match env::consts::OS {
        "windows" => "javaw.exe",
        _ => "java"
    }
}
