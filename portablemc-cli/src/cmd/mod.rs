//! Implementing the logic for the different CLI commands.

mod start;
mod search;
mod auth;

use std::process::{self, ExitCode};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::io;

use portablemc::{base, download, moj, fabric, forge, msa};

use crate::parse::{CliArgs, CliCmd, CliOutput};
use crate::output::{Output, LogLevel};
use crate::format::{self, BytesFmt};


const DEFAULT_AZURE_APP_ID: &str = "708e91b5-99f8-4a1d-80ec-e746cbb24771";
const DEFAULT_MSA_DB_FILE: &str = "portablemc_msa.json";


pub fn main(args: &CliArgs) -> ExitCode {
    
    // We can set only one Ctrl-C handler for the whole CLI, so we set it here and access
    // the various known resources that we should shutdown.
    ctrlc::set_handler(|| {

        // No unwrap to avoid panicking if poisoned.
        if let Ok(mut guard) = start::GAME_CHILD.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
            }
        }

        process::exit(0);
        
    }).unwrap();

    // Create the adequate output handle depending on the output and verbose options.
    let mut out = match args.output {
        CliOutput::Human => Output::human(match args.verbose {
            0 => LogLevel::Pending,
            1.. => LogLevel::Info,
        }),
        CliOutput::Machine => Output::tab_separated(),
    };

    // Ensure that we can have a main directory for Minecraft, needed for all commands.
    let Some(main_dir) = args.main_dir.as_deref()
        .or_else(|| base::default_main_dir())
        .map(Path::to_path_buf) else {
        
        out.log("error_missing_main_dir")
            .error("There is no default main directory for your platform, please specify it using --main-dir")
            .additional("This directory is used to define derived directories for the various commands");
        
        return ExitCode::FAILURE;

    };

    let msa_db_file = args.msa_db_file.clone()
        .unwrap_or_else(|| main_dir.join(DEFAULT_MSA_DB_FILE));
    let msa_azure_app_id = args.msa_azure_app_id.clone()
        .unwrap_or_else(|| DEFAULT_AZURE_APP_ID.to_string());

    let mut cli = Cli {
        out,
        main_dir,
        msa_db: msa::Database::new(msa_db_file),
        msa_azure_app_id,
    };

    legacy_check(&mut cli);

    match &args.cmd {
        CliCmd::Start(start_args) => start::start(&mut cli, start_args),
        CliCmd::Search(search_args) => search::search(&mut cli, search_args),
        CliCmd::Auth(auth_args) => auth::auth(&mut cli, auth_args),
    }

}

fn legacy_check(cli: &mut Cli) {

    const LEGACY_FILES: [&str; 2] = ["portablemc_auth.json", "portablemc_version_manifest.json"];

    // Cleanup any legacy files from the older Python version.
    let mut files = Vec::new();
    for file_name in LEGACY_FILES {
        let file = cli.main_dir.join(file_name);
        if file.exists() {
            files.push(file);
        }
    }

    if files.is_empty() {
        return;
    }

    let mut log = cli.out.log("warn_legacy_file");
    log.args(files.iter().map(|file| file.display()));
    log.warning("The following files were used in older versions of the launcher and you can safely delete them:");
    for file in files {
        log.additional(file.display());
    }
    
}


/// Shared CLI data.
#[derive(Debug)]
pub struct Cli {
    pub out: Output,
    pub main_dir: PathBuf,
    pub msa_db: msa::Database,
    pub msa_azure_app_id: String,
}

/// Generic handler for various event handlers type (download and installers).
#[derive(Debug)]
pub struct LogHandler<'a> {
    /// Handle to the output.
    out: &'a mut Output,
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
    /// When an installer with different supported APIs (for finding game or loader 
    /// versions) is used, this defines the id used for log messages.
    api_id: &'static str,
    /// For the same reason as above, this field is used for human-readable messages.
    api_name: &'static str,
    /// The LWJGL version loaded.
    loaded_lwjgl_version: Option<String>,
    /// The JVM major version being loaded.
    jvm_major_version: u32,
}

impl<'a> LogHandler<'a> {

    pub fn new(out: &'a mut Output) -> Self {
        Self {
            out,
            download_start: None,
            api_id: "",
            api_name: "",
            loaded_lwjgl_version: None,
            jvm_major_version: 0,
        }
    }
    
    fn set_api(&mut self, api_id: &'static str, api_name: &'static str) {
        self.api_id = api_id;
        self.api_name = api_name;
    }

    pub fn set_fabric_loader(&mut self, loader: fabric::Loader) {
        let (api_id, api_name) = fabric_id_name(loader);
        self.set_api(api_id, api_name);
    }

    pub fn set_forge_loader(&mut self, loader: forge::Loader) {
        let (api_id, api_name) = forge_id_name(loader);
        self.set_api(api_id, api_name);
    }

}

impl download::Handler for LogHandler<'_> {
    fn on_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        
        if self.download_start.is_none() {
            self.download_start = Some(Instant::now());
        }

        let elapsed = self.download_start.unwrap().elapsed();
        let speed = size as f32 / elapsed.as_secs_f32();

        if count == total_count {
            self.download_start = None;
        }

        // No logging when no size is actually downloaded, for example when downloading
        // already cached files. But if these files needs to be re-downloaded, then the
        // download will be shown.
        if size == 0 {
            return;
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

impl base::Handler for LogHandler<'_> {

    fn on_event(&mut self, event: base::Event) {
        match event {
            base::Event::FilterFeatures { .. } => {}
            base::Event::LoadedFeatures { features } => {
                
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

                self.out.log("loaded_features")
                    .args(features.iter())
                    .info(format_args!("Features loaded: {buffer}"));

            }
            base::Event::LoadHierarchy { root_version } => {
                
                self.out.log("load_hierarchy")
                    .arg(root_version)
                    .info(format_args!("Hierarchy loading from {root_version}"));

            }
            base::Event::LoadedHierarchy { hierarchy } => {

                let mut buffer = String::new();
                for version in hierarchy {
                    if !buffer.is_empty() {
                        buffer.push_str(" -> ");
                    }
                    buffer.push_str(&version.name());
                }

                self.out.log("loaded_hierarchy")
                    .args(hierarchy.iter().map(|v| v.name()))
                    .info(format_args!("Hierarchy loaded: {buffer}"));

            }
            base::Event::LoadVersion { version, file } => {
                
                self.out.log("load_version")
                    .arg(version)
                    .pending(format_args!("Loading version {version}"))
                    .info(format_args!("Loading version metadata: {}", file.display()));
                
            }
            base::Event::NeedVersion { .. } => {}
            base::Event::LoadedVersion { version, .. } => {

                self.out.log("loaded_version")
                    .arg(version)
                    .success(format_args!("Loaded version {version}"));

            }
            base::Event::LoadClient => {
                self.out.log("load_client")
                    .pending("Loading client");
            }
            base::Event::LoadedClient { file } => {
                self.out.log("loaded_client")
                    .arg(file.display())
                    .success("Loaded client");
            }
            base::Event::LoadLibraries => {
                self.out.log("load_libraries")
                    .pending("Loading libraries");
            }
            base::Event::FilterLibraries { .. } => {}
            base::Event::LoadedLibraries { libraries } => {

                self.out.log("loaded_libraries")
                    .args(libraries.iter().map(|lib| &lib.name))
                    .pending(format_args!("Loaded {} libraries, now verifying", libraries.len()));
                
                self.loaded_lwjgl_version = libraries.iter()
                    .find(|lib| ("org.lwjgl", "lwjgl") == (lib.name.group(), lib.name.artifact()))
                    .map(|lib| lib.name.version().to_string());

            }
            base::Event::FilterLibrariesFiles { .. } => {}
            base::Event::LoadedLibrariesFiles { class_files, natives_files } => {

                self.out.log("loaded_libraries_files")
                    .success(format_args!("Loaded and verified {}+{} libraries", class_files.len(), natives_files.len()));

                self.out.log("loaded_class_files")
                    .args(class_files.iter().map(|p| p.display()));
                self.out.log("loaded_natives_files")
                    .args(natives_files.iter().map(|p| p.display()));
                
                // Just an information for debug.
                if let Some(lwjgl_version) = self.loaded_lwjgl_version.as_deref() {
                    self.out.log("loaded_lwjgl_version")
                        .arg(lwjgl_version)
                        .info(format_args!("Loaded LWJGL version: {lwjgl_version}"));
                }
                
            }
            base::Event::NoLogger => {
                self.out.log("no_logger")
                    .success("No logger");
            }
            base::Event::LoadLogger { id } => {
                self.out.log("load_logger")
                    .arg(id)
                    .pending(format_args!("Loading logger {id}"));
            }
            base::Event::LoadedLogger { id } => {
                self.out.log("loaded_logger")
                    .arg(id)
                    .success(format_args!("Loaded logger {id}"));
            }
            base::Event::NoAssets => {
                self.out.log("no_assets")
                    .success("No assets");
            }
            base::Event::LoadAssets { id } => {
                self.out.log("assets_loading")
                    .arg(id)
                    .pending(format_args!("Loading assets {id}"));
            }
            base::Event::LoadedAssets { id, count } => {
                self.out.log("assets_loaded")
                    .arg(id)
                    .arg(count)
                    .pending(format_args!("Loaded {count} assets {id}"));
            }
            base::Event::VerifiedAssets { id, count } => {
                self.out.log("verified_assets")
                    .arg(id)
                    .arg(count)
                    .success(format_args!("Loaded and verified {count} assets {id}"));
            }
            base::Event::LoadJvm { major_version } => {
                self.jvm_major_version = major_version;
                self.out.log("load_jvm")
                    .arg(major_version)
                    .pending(format_args!("Loading JVM (major version {major_version})"));
            }
            base::Event::FoundJvmSystemVersion { file, version, compatible } => {

                let compatible_str = if compatible { "compatible" } else { "incompatible" };

                self.out.log("found_jvm_system_version")
                    .arg(file.display())
                    .arg(version)
                    .arg(compatible)
                    .info(format_args!("Found system JVM at {}, version {version}, {compatible_str}", file.display()));
                
            }
            base::Event::WarnJvmUnsupportedDynamicCrt => {
                self.out.log("warn_jvm_unsupported_dynamic_crt")
                    .info("Couldn't find a Mojang JVM because your launcher is compiled with a static C runtime");
            }
            base::Event::WarnJvmUnsupportedPlatform => {
                self.out.log("warn_jvm_unsupported_platform")
                    .info("Couldn't find a Mojang JVM because your platform is not supported");
            }
            base::Event::WarnJvmMissingDistribution => {
                self.out.log("warn_jvm_missing_distribution")
                    .info("Couldn't find a Mojang JVM because the required distribution was not found");
            }
            base::Event::LoadedJvm { file, version, compatible } => {

                {
                    let mut log = self.out.log("loaded_jvm");
                    log.arg(file.display());
                    log.args(version);
                    
                    if let Some(version) = version {
                        log.success(format_args!("Loaded JVM ({version})"));
                    } else {
                        log.success(format_args!("Loaded JVM (unknown version)"));
                    }

                    log.info(format_args!("Loaded JVM at {}", file.display()));

                }

                if !compatible {
                    
                    self.out.log("warn_jvm_likely_incompatible")
                        .warning(format_args!("Loaded JVM is likely incompatible with the game version, which requires major version {}", 
                            self.jvm_major_version));
                    
                }
                
            }
            base::Event::DownloadResources { .. } => {
                self.out.log("download_resources")
                    .pending("Downloading");
            }
            base::Event::DownloadProgress { count, total_count, size, total_size } => {
                download::Handler::on_progress(self, count, total_count, size, total_size);
            }
            base::Event::DownloadedResources => {
                self.out.log("resources_downloaded")
                    .success("Downloaded");
            }
            base::Event::ExtractedBinaries { dir } => {
                self.out.log("binaries_extracted")
                    .arg(dir.display())
                    .info(format_args!("Binaries extracted to {}", dir.display()));
            }
            _ => todo!(),
        }
    }

}

impl moj::Handler for LogHandler<'_> {

    
    fn on_event(&mut self, event: moj::Event) {
        match event {
            moj::Event::Base(event) => {
                base::Handler::on_event(self, event);
            }
            moj::Event::InvalidatedVersion { version } => {
                self.out.log("invalidated_version")
                    .arg(version)
                    .info(format_args!("Version {version} invalidated"));
            }
            moj::Event::FetchVersion { version } => {
                self.out.log("fetch_version")
                    .arg(version)
                    .pending(format_args!("Fetching version {version}"));
            }
            moj::Event::FetchedVersion { version } => {
                self.out.log("fetched_version")
                    .arg(version)
                    .success(format_args!("Fetched version {version}"));
            }
            moj::Event::FixedLegacyQuickPlay => {
                self.out.log("fixed_legacy_quick_play")
                    .info("Fixed: legacy quick play");
            }
            moj::Event::FixedLegacyProxy { host, port } => {
                self.out.log("fixed_legacy_proxy")
                    .arg(host)
                    .arg(port)
                    .info(format_args!("Fixed: legacy proxy ({host}:{port})"));
            }
            moj::Event::FixedLegacyMergeSort => {
                self.out.log("fixed_legacy_merge_sort")
                    .info("Fixed: legacy merge sort");
            }
            moj::Event::FixedLegacyResolution => {
                self.out.log("fixed_legacy_resolution")
                    .info("Fixed: legacy resolution");
            }
            moj::Event::FixedBrokenAuthlib => {
                self.out.log("fixed_broken_authlib")
                    .info("Fixed: broken authlib");
            }
            moj::Event::WarnUnsupportedQuickPlay => {
                self.out.log("warn_unsupported_quick_play")
                    .warning("Quick play has been requested but is not supported");
            }
            moj::Event::WarnUnsupportedResolution => {
                self.out.log("warn_unsupported_resolution")
                    .warning("Resolution has been requested but is not supported");
            }
            _ => todo!(),
        }
    }

}

impl fabric::Handler for LogHandler<'_> {

    fn on_event(&mut self, event: fabric::Event) {
        let (api_id, api_name) = (self.api_id, self.api_name);
        match event {
            fabric::Event::Mojang(event) => {
                moj::Handler::on_event(self, event);
            }
            fabric::Event::FetchVersion { game_version, loader_version } => {
                self.out.log(format_args!("{api_id}_fetch_loader"))
                    .arg(game_version)
                    .arg(loader_version)
                    .pending(format_args!("Fetching {api_name} loader {loader_version} for {game_version}"));
            }
            fabric::Event::FetchedVersion { game_version, loader_version } => {
                self.out.log(format_args!("{api_id}_fetched_loader"))
                    .arg(game_version)
                    .arg(loader_version)
                    .info(format_args!("Fetched {api_name} loader {loader_version} for {game_version}"));
            }
            _ => todo!(),
        }
    }

}

impl forge::Handler for LogHandler<'_> {

    fn on_event(&mut self, event: forge::Event) {
        let api_id = self.api_id;
        match event {
            forge::Event::Mojang(event) => {
                moj::Handler::on_event(self, event);
            }
            forge::Event::Installing { tmp_dir, reason } => {

                let (reason_code, log_level, reason_desc) = match reason {
                    forge::InstallReason::MissingVersionMetadata => 
                        ("missing_version_metadata", LogLevel::Success, "The version metadata is absent, installing"),
                    forge::InstallReason::MissingCoreLibrary => 
                        ("missing_core_library", LogLevel::Warn, "The core loader library is absent, reinstalling"),
                    forge::InstallReason::MissingClientExtra => 
                        ("missing_client_extra", LogLevel::Warn, "The client extra is absent, reinstalling"),
                    forge::InstallReason::MissingClientSrg => 
                        ("missing_client_srg", LogLevel::Warn, "The client srg is absent, reinstalling"),
                    forge::InstallReason::MissingPatchedClient => 
                        ("missing_patched_client", LogLevel::Warn, "The patched client is absent, reinstalling"),
                    forge::InstallReason::MissingUniversalClient => 
                        ("missing_universal_client", LogLevel::Warn, "The universal client is absent, reinstalling"),
                };

                self.out.log(format_args!("{api_id}_installing"))
                    .arg(reason_code)
                    .newline()  // Don't overwrite the failed line.
                    .line(log_level, reason_desc);
                
                self.out.log_indent_inc();

                self.out.log(format_args!("{api_id}_install_dir"))
                    .arg(tmp_dir.display())
                    .info(format_args!("Installing in temporary directory: {}", tmp_dir.display()));
                
            }
            forge::Event::FetchInstaller { version } => {
                self.out.log(format_args!("{api_id}_fetch_installer"))
                    .arg(version)
                    .pending(format_args!("Fetching installer {version}"));
            }
            forge::Event::FetchedInstaller { version } => {
                self.out.log(format_args!("{api_id}_fetched_installer"))
                    .arg(version)
                    .success(format_args!("Fetched installer {version}"));
            }
            forge::Event::InstallingGame => {
                self.out.log(format_args!("{api_id}_game_installing"))
                    .success("Installing the game version required by the installer");
            }
            forge::Event::FetchInstallerLibraries => {
                self.out.log(format_args!("{api_id}_installer_libraries_fetching"))
                    .pending(format_args!("Fetching installer libraries"));
            }
            forge::Event::FetchedInstallerLibraries => {
                self.out.log(format_args!("{api_id}_installer_libraries_fetched"))
                    .success(format_args!("Fetched installer libraries"));
            }
            forge::Event::RunInstallerProcessor { name, task } => {

                let desc = match (name.artifact(), task) {
                    ("installertools", Some("MCP_DATA")) => 
                        "Generating MCP data",
                    ("installertools", Some("DOWNLOAD_MOJMAPS")) => 
                        "Downloading Mojang mappings",
                    ("installertools", Some("MERGE_MAPPING")) => 
                        "Merging MCP and Mojang mappings",
                    ("installertools", Some("PROCESS_MINECRAFT_JAR")) => 
                        "Process client (NeoForge)",
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

                self.out.log(format_args!("{api_id}_installer_processor"))
                    .arg(name.as_str())
                    .args(task)
                    .success(format_args!("{desc}"))
                    .info(format_args!("{name} ({})", task.unwrap_or("<no param>")));

            }
            forge::Event::Installed => {
                self.out.log_indent_dec();
                self.out.log(format_args!("{api_id}_installed"))
                    .success("Loader installed, retrying to start the game");
            }
            _ => todo!(),
        }
    }

}

/// Log a base error on the given logger output.
pub fn log_base_error(cli: &mut Cli, error: &base::Error) {
    
    use base::Error;

    let out = &mut cli.out;

    match error {
        Error::HierarchyLoop { version } => {
            out.log("error_hierarchy_loop")
                .arg(&version)
                .error(format_args!("Version {version} appears twice in the hierarchy, causing an infinite hierarchy loop"));
        }
        Error::VersionNotFound { version } => {
            out.log("error_version_not_found")
                .arg(&version)
                .error(format_args!("Version {version} not found"));
        }
        Error::AssetsNotFound { id } => {
            out.log("error_assets_not_found")
                .arg(&id)
                .error(format_args!("Assets {id} not found although it is needed by the version"));
        }
        Error::ClientNotFound {  } => {
            out.log("error_client_not_found")
                .error("Client JAR file not found and no download information is available");
        }
        Error::LibraryNotFound { name: gav } => {
            out.log("error_library_not_found")
                .error(format_args!("Library {gav} not found and no download information is available"));
        }
        Error::JvmNotFound { major_version } => {
            let mut log = out.log("error_jvm_not_found");
            log.error(format_args!("No compatible JVM found for the game version, which requires major version {major_version}"));
            log.additional("You can enable verbose mode to learn more about potential JVM rejections");
            if *major_version <= 8 {
                log.additional("Note that JVM version 8 and prior versions are not compatible with other versions");
            }
        }
        Error::MainClassNotFound {  } => {
            out.log("error_main_class_not_found")
                .error("No main class specified in version metadata");
        }
        Error::DownloadResourcesCancelled {  } => {
            panic!("should not happen because the handler does not cancel downloading");
        }
        Error::Download { batch } => {
            log_download_error(cli, batch);
        }
        Error::Internal { error, origin } => {

            cli.out.log("error_base")
                .arg(origin)
                .error("The following error(s) originates from:")
                .additional(origin);

            log_any_error(cli, &**error, false, true);

        }
        _ => todo!(),
    }

}

/// Log a mojang error on the given logger output.
pub fn log_mojang_error(cli: &mut Cli, error: &moj::Error) {

    use moj::Error;

    let out = &mut cli.out;

    match error {
        Error::Base(error) => log_base_error(cli, error),
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

pub fn log_fabric_error(cli: &mut Cli, error: &fabric::Error, loader: fabric::Loader) {

    use fabric::Error;

    let out = &mut cli.out;
    let (api_id, api_name) = fabric_id_name(loader);

    match *error {
        Error::Mojang(ref error) => log_mojang_error(cli, error),
        Error::LatestVersionNotFound { ref game_version, stable } => {

            let stable_str = if stable { "stable" } else { "unstable" };
            let mut log = out.log(format_args!("error_{api_id}_latest_version_not_found"));
            log.arg(stable_str);
            log.args(game_version.as_ref());

            if let Some(game_version) = game_version {
                log.error(format_args!("Failed to find {api_name} latest {stable_str} loader version for {game_version}"));
                if stable {
                    log.additional("The loader might not yet support any stable version for this game version");
                } else {
                    log.additional("The loader have zero version supported for this game version, likely an issue on their side");
                }
            } else {
                log.error(format_args!("Failed to find {api_name} latest {stable_str} game version"));
                if stable {
                    log.additional("The loader might not yet support any stable game version");
                } else {
                    log.additional("The loader have zero game version supported, likely an issue on their side");
                }
            }

        }
        Error::GameVersionNotFound { ref game_version } => {
            out.log(format_args!("error_{api_id}_game_version_not_found"))
                .arg(&game_version)
                .error(format_args!("{api_name} loader does not support {game_version} game version"));
        }
        Error::LoaderVersionNotFound { ref game_version, ref loader_version } => {
            out.log(format_args!("error_{api_id}_loader_version_not_found"))
                .arg(&game_version)
                .arg(&loader_version)
                .error(format_args!("{api_name} loader has no version {loader_version} for game version {game_version}"));
        }
        _ => todo!(),
    }

}

pub fn log_forge_error(cli: &mut Cli, error: &forge::Error, loader: forge::Loader) {

    use forge::Error;

    let out = &mut cli.out;
    let (api_id, api_name) = forge_id_name(loader);

    const CONTACT_DEV: &str = "This version of the loader might not be supported by PortableMC, please contact developers on https://github.com/mindstorm38/portablemc/issues";

    match *error {
        Error::Mojang(ref error) => log_mojang_error(cli, error),
        Error::LatestVersionNotFound { ref game_version, stable } => {

            let stable_str = if stable { "stable" } else { "unstable" };
            let mut log = out.log(format_args!("error_{api_id}_latest_version_not_found"));
            log.arg(stable_str);
            log.arg(&game_version);
            log.error(format_args!("Failed to find {api_name} latest {stable_str} loader version for {game_version}"));
            log.additional("This game version might not yet be supported by the loader");
            if stable {
                log.additional(format_args!("You can try to relax this by also targeting unstable loader versions with {api_id}:{game_version}:unstable"));
            }

        }
        Error::InstallerNotFound { ref version } => {
            out.log(format_args!("error_{api_id}_installer_not_found"))
                .arg(&version)
                .error(format_args!("{api_name} loader has no installer for {version}"))
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
        Error::InstallerFileNotFound { ref entry } => {
            out.log(format_args!("error_{api_id}_installer_file_not_found"))
                .arg(&entry)
                .error(format_args!("{api_name} installer is missing a required file: {entry}"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerProcessorNotFound { ref name } => {
            out.log(format_args!("error_{api_id}_installer_processor_not_found"))
                .arg(&name)
                .error(format_args!("{api_name} installer could not find a processor {name}"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerProcessorMainClassNotFound { ref name } => {
            out.log(format_args!("error_{api_id}_installer_processor_main_class_not_found"))
                .arg(&name)
                .error(format_args!("{api_name} installer could not find the main class for processor {name}"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerProcessDependencyNotFound { ref name, ref dependency } => {
            out.log(format_args!("error_{api_id}_installer_processor_dependency_not_found"))
                .arg(&name)
                .error(format_args!("{api_name} installer could not find the dependency {dependency} for processor {name}"))
                .additional(CONTACT_DEV);
        }
        Error::InstallerProcessorFailed { ref name, ref output } => {

            let mut log = out.log(format_args!("error_{api_id}_installer_processor_failed"));
            log.arg(&name);

            if let Some(code) = output.status.code() {
                log.arg(code);
            } else {
                log.arg("");
            }
            
            log.error(format_args!("{api_name} installer processor {name} failed ({}):", output.status));

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
        Error::InstallerProcessorCorrupted { ref name, ref file, ref expected_sha1 } => {
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
pub fn log_download_error(cli: &mut Cli, batch: &download::BatchResult) {

    use download::EntryErrorKind;

    if !batch.has_errors() {
        return;
    }

    // error_download <errors_count> <total_count>
    cli.out.log("error_download")
        .arg(batch.errors_count())
        .arg(batch.len())
        .newline()
        .error(format_args!("Failed to download {} out of {} entries...", batch.errors_count(), batch.len()));

    // error_download_entry <url> <dest> <error> [error_data...]
    // ... possibly followed by a stack of standard errors
    for error in batch.iter_errors() {

        let mut log = cli.out.log("error_download_entry");
        log.arg(error.url());
        log.arg(error.file().display());

        log.error(format_args!("{}", error.url()));
        log.additional(format_args!("-> {}", error.file().display()));
        
        match error.kind() {
            EntryErrorKind::InvalidSize => {
                log.arg("invalid_size");
                log.additional(format_args!("   Invalid size"));
            }
            EntryErrorKind::InvalidSha1 => {
                log.arg("invalid_size");
                log.additional(format_args!("   Invalid SHA-1"));
            }
            EntryErrorKind::InvalidStatus(status) => {
                log.arg("invalid_status");
                log.arg(status);
                log.additional(format_args!("   Invalid status: {status}"));
            }
            EntryErrorKind::Internal(error) => {
                log.arg("any");  // Indicates that the real error comes next
                drop(log);
                log_any_error(&mut *cli, &**error, true, true);
            }
        }

    }

}

/// Common function to log an internal and generic error.
pub fn log_any_error(cli: &mut Cli, error: &(dyn std::error::Error + 'static), from_download: bool, additional: bool) {

    let indent = if from_download { "   " } else { "" };
    let level = if additional { LogLevel::Additional } else { LogLevel::Error };

    if let Some(error) = error.downcast_ref::<io::Error>() {
        
        let mut log = cli.out.log("error_io");

        if let Some(error_kind_code) = io_error_kind_code(&error) {
            log.arg(error_kind_code);
        } else {
            log.arg(format_args!("unknown:{error}"));
        }

        // Newline because I/O errors are unexpected and we want to keep any previous context.
        log.newline()
            .line(level, format_args!("{indent}I/O error: {error}"));

    } else if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        cli.out.log("error_reqwest")
            .args(error.status().map(|status| format!("status:{:03}", status.as_u16())))
            .args(error.url().map(|url| format!("url:{url}")))
            .newline()
            .line(level, format_args!("{indent}Reqwest error: {error}"));

    } else if let Some(error) = error.downcast_ref::<serde_json::Error>() {
        cli.out.log("error_json")
            .arg(error)
            .arg("")
            .newline()
            .line(level, format_args!("{indent}JSON error: {error}"));
        
    } else if let Some(error) = error.downcast_ref::<serde_path_to_error::Error<serde_json::Error>>() {
        cli.out.log("error_json")
            .arg(error.inner())
            .arg(error.path())
            .newline()
            .line(level, format_args!("{indent}JSON error: {error}"))
            .additional(format_args!("{indent}At {}", error.path()));

    } else if let Some(error) = error.downcast_ref::<zip::result::ZipError>() {
        cli.out.log("error_zip")
            .arg(error)
            .newline()
            .line(level, format_args!("{indent}ZIP error: {error}"));

    } else if let Some(error) = error.downcast_ref::<jsonwebtoken::errors::Error>() {
        cli.out.log("error_jwt")
            .line(level, format_args!("{indent}JWT error: {error}"));

    } else {
        cli.out.log("error_unknown")
            .arg(error)
            .newline()
            .line(level, format_args!("{indent}Unknown error: {error}"));

    }

    if let Some(source) = error.source() {
        log_any_error(cli, source, from_download, true);
    }

}

/// Log a database error.
pub fn log_msa_auth_error(cli: &mut Cli, error: &msa::AuthError) {
    match error {
        msa::AuthError::Declined => {
            cli.out.log("error_auth_declined")
                .error("Authorization request has been declined");
        }
        msa::AuthError::TimedOut => {
            cli.out.log("error_auth_timed_out")
                .error("Authorization timed out");
        }
        msa::AuthError::OutdatedToken => {
            cli.out.log("error_auth_outdated_token")
                .error("Outdated authentication token");
        }
        msa::AuthError::DoesNotOwnGame => {
            cli.out.log("error_auth_does_not_own_game")
                .error("The account you logged in doesn't own Minecraft");
        }
        msa::AuthError::InvalidStatus(status) => {
            cli.out.log("error_auth_invalid_status")
                .arg(status)
                .error(format_args!("Invalid status while authenticating: {status}"));
        }
        msa::AuthError::Unknown(error) => {
            cli.out.log("error_auth_unknown")
                .arg(error)
                .error(format_args!("Unknown authentication error: {error}"));
        }
        msa::AuthError::Internal(error) => {
            log_any_error(cli, &**error, false, false);
        }
        _ => todo!()
    }
}

/// Log a database error.
pub fn log_msa_database_error(cli: &mut Cli, error: &msa::DatabaseError) {
    match error {
        msa::DatabaseError::Io(error) => {
            log_any_error(cli, error, false, false);
        },
        msa::DatabaseError::Corrupted => {
            cli.out.log("error_msa_database_corrupted")
                .arg(cli.msa_db.file().display())
                .error("The authentication database is corrupted and cannot be recovered automatically")
                .additional(format_args!("At {}", cli.msa_db.file().display()));
        }
        msa::DatabaseError::WriteFailed => {
            cli.out.log("error_msa_database_write_failed")
                .arg(cli.msa_db.file().display())
                .error("Unknown error while writing the authentication database, operation cancelled")
                .additional(format_args!("At {}", cli.msa_db.file().display()));
        }
        _ => todo!()
    }
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

fn fabric_id_name(loader: fabric::Loader) -> (&'static str, &'static str) {
    match loader {
        fabric::Loader::Fabric => ("fabric", "Fabric"),
        fabric::Loader::Quilt => ("quilt", "Quilt"),
        fabric::Loader::LegacyFabric => ("legacyfabric", "LegacyFabric"),
        fabric::Loader::Babric => ("babric", "Babric"),
    }
}

fn forge_id_name(loader: forge::Loader) -> (&'static str, &'static str) {
    match loader {
        forge::Loader::Forge => ("forge", "Forge"),
        forge::Loader::NeoForge => ("neoforge", "NeoForge"),
    }
}
