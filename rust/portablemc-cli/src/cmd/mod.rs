//! Implementing the logic for the different CLI commands.

pub mod start;
pub mod search;

use std::process::ExitCode;
use std::path::PathBuf;
use std::time::Instant;
use std::error::Error;
use std::io;

use portablemc::{download, standard, mojang, fabric, forge};

use crate::parse::{CliArgs, CliCmd, CliOutput};
use crate::output::{Output, LogLevel};
use crate::format::{self, BytesFmt};


pub fn main(args: CliArgs) -> ExitCode {
    
    ctrlc::set_handler(|| {

        // No unwrap to avoid panicking if poisoned.
        if let Ok(mut guard) = start::GAME_CHILD.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
            }
        }
        
    }).unwrap();

    let mut out = match args.output {
        CliOutput::Human => Output::human(match args.verbose {
            0 => LogLevel::Pending,
            1.. => LogLevel::Info,
        }),
        CliOutput::Machine => Output::tab_separated(),
    };

    let Some(main_dir) = args.main_dir.or_else(standard::default_main_dir) else {
        
        out.log("error_missing_main_dir")
            .error("There is no default main directory for your platform, please specify it using --main-dir");
        
        return ExitCode::FAILURE;

    };

    let mut cli = Cli {
        out,
        versions_dir: main_dir.join("versions"),
        libraries_dir: main_dir.join("libraries"),
        assets_dir: main_dir.join("assets"),
        jvm_dir: main_dir.join("jvm"),
        bin_dir: main_dir.join("bin"),
        mc_dir: main_dir.clone(),
        main_dir,
    };

    match &args.cmd {
        CliCmd::Start(start_args) => start::main(&mut cli, start_args),
        CliCmd::Search(search_args) => search::main(&mut cli, search_args),
        CliCmd::Info(_) => todo!(),
        CliCmd::Login(_) => todo!(),
        CliCmd::Logout(_) => todo!(),
        CliCmd::Show(_) => todo!(),
    }

}


/// Shared CLI data.
#[derive(Debug)]
pub struct Cli {
    pub out: Output,
    pub main_dir: PathBuf,
    pub versions_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub jvm_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub mc_dir: PathBuf,
}


/// Generic handler for various event handlers type (download and installers).
#[derive(Debug)]
pub struct CommonHandler<'a> {
    /// Handle to the output.
    out: &'a mut Output,
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
    /// When an installer with different supported APIs (for finding game or loader 
    /// versions) is used, this defines the id used for log messages.
    api_id: &'static str,
    /// For the same reason as above, this field is used for human-readable messages.
    api_name: &'static str,
}

impl<'a> CommonHandler<'a> {

    pub fn new(out: &'a mut Output) -> Self {
        Self {
            out,
            download_start: None,
            api_id: "",
            api_name: "",
        }
    }

    pub fn set_api(&mut self, api_id: &'static str, api_name: &'static str) {
        self.api_id = api_id;
        self.api_name = api_name;
    }

}

impl download::Handler for CommonHandler<'_> {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        
        if self.download_start.is_none() {
            self.download_start = Some(Instant::now());
        }

        if size == 0 {
            if count == total_count {
                // If all entries have been downloaded but the weight nothing, reset the
                // download start. This is possible with zero-sized files or cache mode.
                self.download_start = None;
            }
            return;
        }

        let elapsed = self.download_start.unwrap().elapsed();
        let speed = size as f32 / elapsed.as_secs_f32();

        if count == total_count {
            self.download_start = None;
        }

        let progress = (size as f32 / total_size as f32).min(1.0) * 100.0;
        let (speed_fmt, speed_suffix) = format::number_si_unit(speed);
        let (size_fmt, size_suffix) = format::number_si_unit(size as f32);

        let mut log = self.out.log_background("download");
        if count == total_count {
            log.message(format_args!("{speed_fmt:.1} {speed_suffix}B/s {size_fmt:.0} {size_suffix}B ({count})"));
        } else {
            log.message(format_args!("{speed_fmt:.1} {speed_suffix}B/s {progress:.1}% ({count}/{total_count})"));
        }
        
        log.arg(format_args!("{count}/{total_count}"));
        log.arg(format_args!("{size}/{total_size}"));
        log.arg(format_args!("{}", elapsed.as_secs_f32()));
        log.arg(format_args!("{speed}"));
        
    }
}

impl standard::Handler for CommonHandler<'_> {
    fn handle_standard_event(&mut self, event: standard::Event) {
        
        use standard::Event;

        let out = &mut *self.out;
        
        match event {
            Event::FeaturesFilter { .. } => {}
            Event::FeaturesLoaded { features } => {

                let mut buffer = String::new();
                for version in features.iter() {
                    if !buffer.is_empty() {
                        buffer.push_str(", ");
                    } else {
                        buffer.push_str(&version);
                    }
                }

                if buffer.is_empty() {
                    buffer.push_str("{}");
                }

                out.log("features_loaded")
                    .args(features.iter())
                    .info(format_args!("Features loaded: {buffer}"));
            }
            Event::HierarchyLoading { root_version } => {
                out.log("hierarchy_loading")
                    .arg(root_version)
                    .info(format_args!("Hierarchy loading from {root_version}"));
            }
            Event::HierarchyFilter { .. } => {}
            Event::HierarchyLoaded { hierarchy } => {

                let mut buffer = String::new();
                for version in hierarchy {
                    if !buffer.is_empty() {
                        buffer.push_str(" -> ");
                    }
                    buffer.push_str(&version.name);
                }

                out.log("hierarchy_loaded")
                    .args(hierarchy.iter().map(|v| &*v.name))
                    .info(format_args!("Hierarchy loaded: {buffer}"));

            }
            Event::VersionLoading { version, .. } => {
                out.log("version_loading")
                    .arg(version)
                    .pending(format_args!("Loading version {version}"));
            }
            Event::VersionNotFound { .. } => {}
            Event::VersionLoaded { version, .. } => {
                out.log("version_loaded")
                    .arg(version)
                    .success(format_args!("Loaded version {version}"));
            }
            Event::ClientLoading {  } => {
                out.log("client_loading")
                    .pending("Loading client");
            }
            Event::ClientLoaded { file } => {
                out.log("client_loaded")
                    .arg(file.display())
                    .success("Loaded client");
            }
            Event::LibrariesLoading {  } => {
                out.log("libraries_loading")
                    .pending("Loading libraries");
            }
            Event::LibrariesFilter { .. } => {}
            Event::LibrariesLoaded { libraries } => {
                out.log("libraries_loaded")
                    .args(libraries.iter().map(|lib| &lib.gav))
                    .pending(format_args!("Loaded {} libraries, now verifying", libraries.len()));
            }
            Event::LibrariesFilesFilter { .. } => {}
            Event::LibrariesFilesLoaded { class_files, natives_files } => {
                
                out.log("libraries_files_loaded")
                    .success(format_args!("Loaded and verified {}+{} libraries", class_files.len(), natives_files.len()));

                out.log("class_files_loaded")
                    .args(class_files.iter().map(|p| p.display()));
                out.log("natives_files_loaded")
                    .args(natives_files.iter().map(|p| p.display()));

            }
            Event::LoggerAbsent {  } => {
                out.log("logger_absent")
                    .success("No logger");
            }
            Event::LoggerLoading { id } => {
                out.log("logger_loading")
                    .arg(id)
                    .pending(format_args!("Loading logger {id}"));
            }
            Event::LoggerLoaded { id } => {
                out.log("logger_loaded")
                    .arg(id)
                    .success(format_args!("Loaded logger {id}"));
            }
            Event::AssetsAbsent {  } => {
                out.log("assets_absent")
                    .success("No assets");
            }
            Event::AssetsLoading { id } => {
                out.log("assets_loading")
                    .arg(id)
                    .pending(format_args!("Loading assets {id}"));
            }
            Event::AssetsLoaded { id, index } => {
                out.log("assets_loaded")
                    .arg(id)
                    .arg(index.objects.len())
                    .pending(format_args!("Loaded {} assets {id}", index.objects.len()));
            }
            Event::AssetsVerified { id, index } => {
                out.log("assets_verified")
                    .arg(id)
                    .arg(index.objects.len())
                    .success(format_args!("Loaded and verified {} assets {id}", index.objects.len()));
            }
            Event::ResourcesDownloading {  } => {
                out.log("resources_downloading")
                    .pending("Downloading");
            }
            Event::ResourcesDownloaded {  } => {
                out.log("resources_downloaded")
                    .success("Downloaded");
            }
            Event::JvmLoading { major_version } => {
                out.log("jvm_loading")
                    .arg(major_version)
                    .pending(format_args!("Loading JVM (preferred: {major_version:?})"));
            }
            Event::JvmVersionRejected { file, version } => {
                
                let mut log = out.log("jvm_version_rejected");
                log.arg(file.display());
                log.args(version.into_iter());

                if let Some(version) = version {
                    log.info(format_args!("Rejected JVM (version {version}) at {}", file.display()));
                } else {
                    log.info(format_args!("Rejected JVM at {}", file.display()));
                }
                
            }
            Event::JvmDynamicCrtUnsupported {  } => {
                out.log("jvm_dynamic_crt_unsupported")
                    .info("Couldn't find a Mojang JVM because your launcher is compiled with a static C runtime");
            }
            Event::JvmPlatformUnsupported {  } => {
                out.log("jvm_platform_unsupported")
                    .info("Couldn't find a Mojang JVM because your platform is not supported");
            }
            Event::JvmDistributionNotFound {  } => {
                out.log("jvm_distribution_not_found")
                    .info("Couldn't find a Mojang JVM because the required distribution was not found");
            }
            Event::JvmLoaded { file, version } => {
                
                let mut log = out.log("jvm_loaded");
                log.arg(file.display());
                log.args(version.into_iter());
                
                if let Some(version) = version {
                    log.success(format_args!("Loaded JVM ({version})"));
                } else {
                    log.success(format_args!("Loaded JVM"));
                }

                log.info(format_args!("Loaded JVM at {}", file.display()));

            }
            Event::BinariesExtracted { dir } => {
                out.log("binaries_extracted")
                    .arg(dir.display())
                    .info(format_args!("Binaries extracted to {}", dir.display()));
            }
            _ => todo!("{event:?}")
        };

    }
}

impl mojang::Handler for CommonHandler<'_> {
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        
        use mojang::Event;

        let out = &mut *self.out;

        match event {
            Event::VersionInvalidated { version: id } => {
                out.log("mojang_version_invalidated")
                    .arg(id)
                    .info(format_args!("Mojang version {id} invalidated"));
            }
            Event::VersionFetching { version: id } => {
                out.log("mojang_version_fetching")
                    .arg(id)
                    .pending(format_args!("Fetching Mojang version {id}"));
            }
            Event::VersionFetched { version: id } => {
                out.log("mojang_version_fetched")
                    .arg(id)
                    .success(format_args!("Fetched Mojang version {id}"));
            }
            Event::FixLegacyQuickPlay {  } => {
                out.log("fix_legacy_quick_play")
                    .info("Fixed: legacy quick play");
            }
            Event::FixLegacyProxy { host, port } => {
                out.log("fix_legacy_proxy")
                    .arg(host)
                    .arg(port)
                    .info(format_args!("Fixed: legacy proxy ({host}:{port})"));
            }
            Event::FixLegacyMergeSort {  } => {
                out.log("fix_legacy_merge_sort")
                    .info("Fixed: legacy merge sort");
            }
            Event::FixLegacyResolution {  } => {
                out.log("fix_legacy_resolution")
                    .info("Fixed: legacy resolution");
            }
            Event::FixBrokenAuthlib {  } => {
                out.log("fix_broken_authlib")
                    .info("Fixed: broken authlib");
            }
            Event::QuickPlayNotSupported {  } => {
                out.log("quick_play_not_supported")
                    .warning("Quick play has been requested but is not supported");
            }
            Event::ResolutionNotSupported {  } => {
                out.log("resolution_not_supported")
                    .warning("Resolution has been requested but is not supported");
            }
            _ => todo!("{event:?}")
        };

    }
}

impl fabric::Handler for CommonHandler<'_> {
    fn handle_fabric_event(&mut self, event: fabric::Event) {
        
        use fabric::Event;

        let out = &mut *self.out;

        let api_id = self.api_id;
        let api_name = self.api_name;
        debug_assert!(!api_id.is_empty() && !api_name.is_empty());

        match event {
            Event::VersionFetching { game_version, loader_version } => {
                out.log(format_args!("{api_id}_version_fetching"))
                    .arg(game_version)
                    .arg(loader_version)
                    .pending(format_args!("Fetching {api_name} loader {loader_version} for {game_version}"));
            }
            Event::VersionFetched { game_version, loader_version } => {
                out.log(format_args!("{api_id}_version_fetched"))
                    .arg(game_version)
                    .arg(loader_version)
                    .info(format_args!("Fetched {api_name} loader {loader_version} for {game_version}"));
            }
            _ => todo!("{event:?}")
        }

    }
}

impl forge::Handler for CommonHandler<'_> {
    fn handle_forge_event(&mut self, event: forge::Event) {
        
        use forge::{Event, InstallReason};

        let out = &mut *self.out;

        let api_id = self.api_id;
        let api_name = self.api_name;
        debug_assert!(!api_id.is_empty() && !api_name.is_empty());

        match event {
            Event::Installing { tmp_dir, reason } => {

                let (reason_code, log_level, reason_desc) = match reason {
                    InstallReason::MissingVersionMetadata => 
                        ("missing_version_metadata", LogLevel::Success, "The version metadata is absent, installing"),
                    InstallReason::MissingCoreLibrary => 
                        ("missing_universal_client", LogLevel::Warn, "The core loader library is absent, reinstalling"),
                    InstallReason::MissingClientExtra => 
                        ("missing_client_extra", LogLevel::Warn, "The client extra is absent, reinstalling"),
                    InstallReason::MissingClientSrg => 
                        ("missing_client_srg", LogLevel::Warn, "The client srg is absent, reinstalling"),
                    InstallReason::MissingPatchedClient => 
                        ("missing_patched_client", LogLevel::Warn, "The patched client is absent, reinstalling"),
                    InstallReason::MissingUniversalClient => 
                        ("missing_universal_client", LogLevel::Warn, "The universal client is absent, reinstalling"),
                };

                out.log(format_args!("{api_id}_installing"))
                    .arg(reason_code)
                    .newline()  // Don't overwrite the failed line.
                    .line(log_level, reason_desc)
                    .info(format_args!("Installing in temporary directory: {}", tmp_dir.display()));
            }
            Event::InstallerFetching { game_version, loader_version } => {
                out.log(format_args!("{api_id}_installer_fetching"))
                    .arg(game_version)
                    .arg(loader_version)
                    .pending(format_args!("Fetching installer {loader_version} for {game_version}"));
            }
            Event::InstallerFetched { game_version, loader_version } => {
                out.log(format_args!("{api_id}_installer_fetched"))
                    .arg(game_version)
                    .arg(loader_version)
                    .success(format_args!("Fetched installer {loader_version} for {game_version}"));
            }
            Event::GameInstalling {  } => {
                out.log(format_args!("{api_id}_game_installing"))
                    .success("Installing the game version required by the installer");
            }
            Event::InstallerLibrariesFetching {  } => {
                out.log(format_args!("{api_id}_installer_libraries_fetching"))
                    .pending(format_args!("Fetching installer libraries"));
            }
            Event::InstallerLibrariesFetched {  } => {
                out.log(format_args!("{api_id}_installer_libraries_fetched"))
                    .success(format_args!("Fetched installer libraries"));
            }
            Event::InstallerProcessor { name, task } => {
                
                let desc = match (name.artifact(), task) {
                    ("installertools", Some("MCP_DATA")) => 
                        "Generating MCP data",
                    ("installertools", Some("DOWNLOAD_MOJMAPS")) => 
                        "Downloading Mojang mappings",
                    ("installertools", Some("MERGE_MAPPING")) => 
                        "Merging MCP and Mojang mappings",
                    ("jarsplitter", _) => 
                        "Splitting client with mappings",
                    ("ForgeAutoRenamingTool", _) => 
                        "Renaming client with mappings (Forge)",
                    ("AutoRenamingTool", _) if name.group() == "net.neoforged" =>
                        "Renaming client with mappings (NeoForge)",
                    ("vignette", _) => 
                        "Renaming client with mappings (Vignette)",
                    ("binarypatcher", _) => 
                        "Patching client",
                    ("SpecialSource", _) => 
                        "Renaming client with mappings (SpecialSource)",
                    _ => name.as_str()
                };

                out.log(format_args!("{api_id}_installer_processor"))
                    .arg(name.as_str())
                    .args(task.into_iter())
                    .success(format_args!("{desc}"));
                
            }
            Event::Installed {  } => {
                out.log(format_args!("{api_id}_installed"))
                    .success("Loader installed, retrying to start the game");
            }
            _ => todo!(),
        }

    }
}

/// Log a standard error on the given logger output.
pub fn log_standard_error(out: &mut Output, error: standard::Error) {
    
    use standard::Error;

    match error {
        Error::VersionNotFound { version: id } => {
            out.log("error_version_not_found")
                .arg(&id)
                .error(format_args!("Version {id} not found"));
        }
        Error::AssetsNotFound { version: id } => {
            out.log("error_assets_not_found")
                .arg(&id)
                .error(format_args!("Assets {id} not found although it is needed by the version"));
        }
        Error::ClientNotFound => {
            out.log("error_client_not_found")
                .error("Client JAR file not found and no download information is available");
        }
        Error::LibraryNotFound { gav } => {
            out.log("error_library_not_found")
                .error(format_args!("Library {gav} not found and no download information is available"));
        }
        Error::JvmNotFound { major_version } => {
            out.log("error_jvm_not_found")
                .error(format_args!("JVM version {major_version} not found"));
        }
        Error::MainClassNotFound {  } => {
            out.log("error_main_class_not_found")
                .error("No main class specified in version metadata");
        }
        Error::Io { error, origin } => {
            log_io_error(out, error, &origin);
        }
        Error::Json { error, origin } => {
            out.log("error_json")
                .arg(error.path())
                .arg(error.inner())
                .arg(&origin)
                .newline()
                .error(format_args!("JSON error: {error}"))
                .additional(format_args!("At {origin}"));
        }
        Error::Zip { error, origin } => {
            out.log("error_zip")
                .arg(&error)
                .arg(&origin)
                .newline()
                .error(format_args!("ZIP error: {error}"))
                .additional(format_args!("At {origin}"));
        }
        Error::Reqwest { error } => {
            let mut log = out.log("error_reqwest");
            log.args(error.url().into_iter());
            log.newline();
            log.error(format_args!("Reqwest error: {error}"));
            if let Some(source) = error.source() {
                log.additional(format_args!("At {source}"));
            }
        }
        Error::Download { batch } => {
            log_download_error(out, batch);
        }
        _ => todo!(),
    }

}

/// Log a mojang error on the given logger output.
pub fn log_mojang_error(out: &mut Output, error: mojang::Error) {

    use mojang::{Error, RootVersion};

    match error {
        Error::Standard(error) => log_standard_error(out, error),
        Error::AliasRootVersionNotFound { root_version } => {
            
            let alias_str = match &root_version {
                RootVersion::Release => "release",
                RootVersion::Snapshot => "snapshot",
                RootVersion::Name(_) => panic!()
            };

            out.log("error_mojang_alias_root_version_not_found")
                .arg(alias_str)
                .error(format_args!("Failed to resolve Mojang root version '{alias_str}'"))
                .additional("The alias might be missing from manifest, likely an issue on Mojang's side");

        }
        Error::LwjglFixNotFound { version } => {
            out.log("error_lwjgl_fix_not_found")
                .arg(&version)
                .error(format_args!("Failed to fix LWJGL to version '{version}' as requested with --lwjgl argument"))
                .additional("The version might be too old (< 3.2.3)")
                .additional("Your platform might not be supported for this version");
        }
        _ => todo!(),
    }

}

pub fn log_fabric_error(out: &mut Output, error: fabric::Error, api_id: &str, api_name: &str) {

    use fabric::{Error, GameVersion, LoaderVersion};

    match error {
        Error::Mojang(error) => log_mojang_error(out, error),
        Error::AliasGameVersionNotFound { game_version } => {

            let alias_str = match game_version {
                GameVersion::Stable => "stable",
                GameVersion::Unstable => "unstable",
                GameVersion::Name(_) => panic!()
            };

            let mut log = out.log(format_args!("error_{api_id}_alias_game_version_not_found"));
            log.arg(alias_str);
            log.error(format_args!("Failed to resolve {api_name} game version '{alias_str}'"));

            match game_version {
                GameVersion::Stable => log.additional("The loader might not yet support any stable game version"),
                GameVersion::Unstable => log.additional("The loader have zero game version supported, likely an issue on their side"),
                GameVersion::Name(_) => unreachable!()
            };

        }
        Error::AliasLoaderVersionNotFound { game_version, loader_version } => {

            let alias_str = match loader_version {
                LoaderVersion::Stable => "stable",
                LoaderVersion::Unstable => "unstable",
                LoaderVersion::Name(_) => panic!()
            };

            let mut log = out.log(format_args!("error_{api_id}_alias_loader_version_not_found"));
            log.arg(&game_version);
            log.arg(alias_str);
            log.error(format_args!("Failed to resolve {api_name} loader version '{alias_str}' for game version {game_version}"));

            match loader_version {
                LoaderVersion::Stable => log.additional("The loader might not yet support any stable version for this game version"),
                LoaderVersion::Unstable => log.additional("The loader have zero version supported for this game version, likely an issue on their side"),
                LoaderVersion::Name(_) => unreachable!()
            };

        }
        Error::GameVersionNotFound { game_version } => {
            out.log(format_args!("error_{api_id}_game_version_not_found"))
                .arg(&game_version)
                .error(format_args!("{api_name} loader has not support for {game_version} game version"));
        }
        Error::LoaderVersionNotFound { game_version, loader_version } => {
            out.log(format_args!("error_{api_id}_loader_version_not_found"))
                .arg(&game_version)
                .arg(&loader_version)
                .error(format_args!("{api_name} loader has no version {loader_version} for game version {game_version}"));
        }
        _ => todo!(),
    }

}

pub fn log_forge_error(out: &mut Output, error: forge::Error, api_id: &str, api_name: &str) {

    use forge::{Error, GameVersion, LoaderVersion};

    const CONTACT_DEV: &str = "This version of the loader might not be supported by PortableMC, please contact developers on https://github.com/mindstorm38/portablemc/issues";

    match error {
        Error::Mojang(error) => log_mojang_error(out, error),
        Error::AliasGameVersionNotFound { game_version } => {

            let alias_str = match game_version {
                GameVersion::Release => "release",
                GameVersion::Name(_) => panic!()
            };

            let mut log = out.log(format_args!("error_{api_id}_alias_game_version_not_found"));
            log.arg(alias_str);
            log.error(format_args!("Failed to resolve {api_name} game version '{alias_str}'"));
            log.additional("The alias might be missing from manifest, likely an issue on Mojang's side");

        }
        Error::AliasLoaderVersionNotFound { game_version, loader_version } => {
            
            let alias_str = match loader_version {
                LoaderVersion::Stable => "stable",
                LoaderVersion::Unstable => "unstable",
                LoaderVersion::Name(_) => panic!()
            };

            let mut log = out.log(format_args!("error_{api_id}_alias_loader_version_not_found"));
            log.arg(&game_version);
            log.arg(alias_str);
            log.error(format_args!("Failed to resolve {api_name} loader version '{alias_str}' for game version {game_version}"));

            match loader_version {
                LoaderVersion::Stable => log.additional("The loader might not yet support any stable version for this game version"),
                LoaderVersion::Unstable => log.additional("The loader have zero version supported for this game version"),
                LoaderVersion::Name(_) => unreachable!()
            };

        }
        Error::GameVersionNotFound { game_version } => {
            out.log(format_args!("error_{api_id}_game_version_not_found"))
                .arg(&game_version)
                .error(format_args!("{api_name} loader has not support for {game_version} game version"));
        }
        Error::LoaderVersionNotFound { game_version, loader_version } => {
            out.log(format_args!("error_{api_id}_loader_version_not_found"))
                .arg(&game_version)
                .arg(&loader_version)
                .error(format_args!("{api_name} loader has no version {loader_version} for game version {game_version}"))
                .additional("Note that really old versions have no installer and therefore are not supported by PortableMC");
        }
        Error::MavenMetadataMalformed {  } => {
            out.log(format_args!("error_{api_id}_maven_metadata_malformed"))
                .error(format_args!("{api_name} loader has an malformed maven metadata"))
                .additional("Likely an issue on the loader's API side");
        }
        Error::InstallerProfileNotFound {  } => {
            out.log(format_args!("error_{api_id}_installer_profile_not_found"))
                .error(format_args!("{api_name} installer has no installer profile"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerProfileIncoherent {  } => {
            out.log(format_args!("error_{api_id}_installer_profile_incoherent"))
                .error(format_args!("{api_name} installer profile is incoherent with what should've been downloaded"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerVersionMetadataNotFound {  } => {
            out.log(format_args!("error_{api_id}_installer_version_metadata_not_found"))
                .error(format_args!("{api_name} installer has no embedded version metadata"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerFileNotFound { entry } => {
            out.log(format_args!("error_{api_id}_installer_file_not_found"))
                .arg(&entry)
                .error(format_args!("{api_name} installer is missing a required file: {entry}"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerInvalidProcessor { name } => {
            out.log(format_args!("error_{api_id}_installer_invalid_processor"))
                .arg(&name)
                .error(format_args!("{api_name} installer has an invalid processor: {name}"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerProcessorFailed { name, output } => {

            let mut log = out.log(format_args!("error_{api_id}_installer_processor_failed"));
            log.arg(&name);

            if let Some(code) = output.status.code() {
                log.arg(code);
            } else {
                log.arg("");
            }
            
            log.error(format_args!("{api_name} installer processor failed ({}):", output.status));

            let stdout = std::str::from_utf8(&output.stdout).ok();
            let stderr = std::str::from_utf8(&output.stderr).ok();

            if let Some(stdout) = stdout {
                log.arg(stdout);
                log.additional(format_args!("stdout: {stdout}"));
            } else {
                log.arg(format_args!("{:?}", output.stdout));
                log.additional(format_args!("stdout: {}", output.stdout.escape_ascii()));
            }

            if let Some(stderr) = stderr {
                log.arg(stderr);
                log.additional(format_args!("stderr: {stderr}"));
            } else {
                log.arg(format_args!("{:?}", output.stderr));
                log.additional(format_args!("stderr: {}", output.stdout.escape_ascii()));
            }

            log.additional(CONTACT_DEV);

        }
        Error::InstallerProcessorInvalidOutput { name, file, expected_sha1 } => {
            out.log(format_args!("error_{api_id}_installer_processor_invalid_output"))
                .arg(&name)
                .arg(file.display())
                .error(format_args!("{api_name} installer processor {name} produced invalid output:"))
                .additional(format_args!("At: {}", file.display()))
                .additional(format_args!("Expected: {:x}", BytesFmt(&expected_sha1[..])))
                .additional(CONTACT_DEV);
        }
        _ => todo!(),
    }

}

/// Common function to log a download error.
pub fn log_download_error(out: &mut Output, batch: download::BatchResult) {

    use download::EntryErrorKind;

    if !batch.has_errors() {
        return;
    }

    // error_download <errors_count> <total_count>
    out.log("error_download")
        .arg(batch.errors_count())
        .arg(batch.len())
        .newline()
        .error(format_args!("Failed to download {} out of {} entries...", batch.errors_count(), batch.len()));

    // error_download_entry <url> <dest> <error> [error_data...]
    for error in batch.iter_errors() {

        let mut log = out.log("error_download_entry");
        log.arg(error.url());
        log.arg(error.file().display());

        log.additional(format_args!("{}", error.url()));
        log.additional(format_args!("-> {}", error.file().display()));
        
        match error.kind() {
            EntryErrorKind::Reqwest(error) => {
                log.arg("request");
                log.arg(&error);
                log.args(error.source().into_iter());
                if let Some(source) = error.source() {
                    log.additional(format_args!("   {error} (source: {source})"));
                } else {
                    log.additional(format_args!("   {error}"));
                }
            }
            EntryErrorKind::Io(error) => {
                log.arg("io");
                if let Some(error_kind_code) = io_error_kind_code(&error) {
                    log.arg(error_kind_code);
                } else {
                    log.arg(format_args!("unknown:{error}"));
                }
                log.additional(format_args!("   I/O error: {error}"));
            }
            EntryErrorKind::InvalidStatus(status) => {
                log.arg("invalid_status");
                log.arg(status);
                log.additional(format_args!("   Invalid status: {status}"));
            }
            EntryErrorKind::InvalidSize => {
                log.arg("invalid_size");
                log.additional(format_args!("   Invalid size"));
            }
            EntryErrorKind::InvalidSha1 => {
                log.arg("invalid_size");
                log.additional(format_args!("   Invalid SHA-1"));
            }
        }

    }

}

/// Common function to log an I/O error to the user.
pub fn log_io_error(out: &mut Output, error: io::Error, origin: &str) {

    let mut log = out.log("error_io");

    if let Some(error_kind_code) = io_error_kind_code(&error) {
        log.arg(error_kind_code);
    } else {
        log.arg(format_args!("unknown:{error}"));
    }

    log.arg(origin);

    // Newline because I/O errors are unexpected and we want to keep any previous context.
    log.newline()
        .error(format_args!("I/O error: {error}"))
        .additional(format_args!("At {origin}"));

}

fn io_error_kind_code(error: &io::Error) -> Option<&'static str> {
    use io::ErrorKind;
    Some(match error.kind() {
        ErrorKind::NotFound => "not_found",
        ErrorKind::PermissionDenied => "permission_denied",
        ErrorKind::ConnectionRefused => "connection_refused",
        ErrorKind::ConnectionReset => "connection_reset",
        ErrorKind::ConnectionAborted => "connection_aborted",
        ErrorKind::NotConnected => "not_connected",
        ErrorKind::AddrInUse => "addr_in_use",
        ErrorKind::AddrNotAvailable => "addr_not_available",
        ErrorKind::BrokenPipe => "broken_pipe",
        ErrorKind::AlreadyExists => "already_exists",
        ErrorKind::WouldBlock => "would_block",
        ErrorKind::InvalidInput => "invalid_input",
        ErrorKind::InvalidData => "invalid_data",
        ErrorKind::TimedOut => "timed_out",
        ErrorKind::WriteZero => "write_zero",
        ErrorKind::Interrupted => "interrupted",
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::UnexpectedEof => "unexpected_eof",
        ErrorKind::OutOfMemory => "out_of_memory",
        _ => return None,
    })
}
