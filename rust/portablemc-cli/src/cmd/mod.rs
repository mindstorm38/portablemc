//! Implementing the logic for the different CLI commands.

pub mod start;
pub mod search;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;
use std::error::Error;
use std::io;

use portablemc::{download, mojang, standard};

use crate::parse::{CliArgs, CliCmd, CliOutput};
use crate::output::{Output, LogLevel};
use crate::format;


pub fn main(args: CliArgs) -> ExitCode {

    let mut out = match args.output {
        CliOutput::Human => Output::human(match args.verbose {
            0 => LogLevel::Pending,
            1.. => LogLevel::Info,
        }),
        CliOutput::Machine => Output::tab_separated(),
    };

    let Some(main_dir) = args.main_dir.or_else(standard::default_main_dir) else {
        
        out.log("error_missing_main_dir")
            .error("There is no default main directory for your platform, please specify it using --main-dir.");
        
        return ExitCode::FAILURE;

    };

    let mut cli = Cli {
        out,
        versions_dir: main_dir.join("versions"),
        libraries_dir: main_dir.join("libraries"),
        assets_dir: main_dir.join("assets"),
        jvm_dir: main_dir.join("jvm"),
        bin_dir: main_dir.join("bin"),
        work_dir: main_dir.clone(),
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
    pub work_dir: PathBuf,
}


/// Generic handler for various event handlers type (download and installers).
#[derive(Debug)]
pub struct CommonHandler<'a> {
    /// Handle to the output.
    out: &'a mut Output,
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
}

impl<'a> CommonHandler<'a> {

    pub fn new(out: &'a mut Output) -> Self {
        Self {
            out,
            download_start: None,
        }
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

        let progress = size as f32 / total_size as f32 * 100.0;
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
            Event::HierarchyLoading { root_id } => {
                out.log("hierarchy_loading")
                    .arg(root_id)
                    .info(format_args!("Hierarchy loading from {root_id}"));
            }
            Event::HierarchyFilter { .. } => {}
            Event::HierarchyLoaded { hierarchy } => {

                let mut buffer = String::new();
                for version in hierarchy {
                    if !buffer.is_empty() {
                        buffer.push_str(", ");
                    } else {
                        buffer.push_str(&version.id);
                    }
                }

                out.log("hierarchy_loaded")
                    .args(hierarchy.iter().map(|v| &*v.id))
                    .info(format_args!("Hierarchy loaded: {buffer}"));

            }
            Event::VersionLoading { id, .. } => {
                out.log("version_loading")
                    .arg(id)
                    .pending(format_args!("Loading version {id}"));
            }
            Event::VersionNotFound { .. } => {}
            Event::VersionLoaded { id, .. } => {
                out.log("version_loaded")
                    .arg(id)
                    .success(format_args!("Loaded version {id}"));
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
                    .pending(format_args!("Loaded {} libraries", libraries.len()));
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
            Event::MojangVersionInvalidated { id } => {
                out.log("mojang_version_invalidated")
                    .arg(id)
                    .info(format_args!("Mojang version {id} invalidated"));
            }
            Event::MojangVersionFetching { id } => {
                out.log("mojang_version_fetching")
                    .arg(id)
                    .pending(format_args!("Fetching Mojang version {id}"));
            }
            Event::MojangVersionFetched { id } => {
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

/// Log a standard error on the given logger output.
pub fn log_standard_error(out: &mut Output, error: standard::Error) {
    
    use standard::Error;

    match error {
        Error::VersionNotFound { id } => {
            out.log("error_version_not_found")
                .arg(&id)
                .error(format_args!("Version {id} not found"));
        }
        Error::AssetsNotFound { id } => {
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
        Error::Io { error, file } => {
            log_io_error(out, error, file.as_deref());
        }
        Error::Json { error, file } => {
            out.log("error_json")
                .arg(file.display())
                .arg(error.path())
                .arg(error.inner())
                .error(format_args!("JSON error: {error}"))
                .additional(format_args!("Related to {}", file.display()));
        }
        Error::Zip { error, file } => {
            out.log("error_zip")
                .arg(file.display())
                .arg(&error)
                .error(format_args!("ZIP error: {error}"))
                .additional(format_args!("Related to {}", file.display()));
        }
        Error::Download(error) => {
            log_download_error(out, error);
        }
        _ => todo!(),
    }

}

/// Log a mojang error on the given logger output.
pub fn log_mojang_error(out: &mut Output, error: mojang::Error) {

    use mojang::{Error, Root};

    match error {
        Error::Standard(error) => log_standard_error(out, error),
        Error::AliasVersionNotFound { root } => {
            
            let root_code = match &root {
                Root::Release => "release",
                Root::Snapshot => "snapshot",
                Root::Id(id) => id.as_str(),
            };

            out.log("error_alias_version_not_found")
                .arg(root_code)
                .error(format_args!("Failed to resolve root version '{root_code}'"))
                .additional("Version fetching might be disabled with --exclude-fetch argument set to '*'")
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

/// Common function to log a download error.
pub fn log_download_error(out: &mut Output, error: download::Error) {

    use download::{Error, EntryError, EntryMode};

    match error {
        Error::Reqwest(error) => {
            // error_download_init [error_source]
            let mut log = out.log("error_download_init");
            log.args(error.source().into_iter());
            log.error("Failed to initialize download client");
            if let Some(source) = error.source() {
                log.additional(format_args!("Source: {source}"));
            }
        }
        Error::Entries(entries) => {
            
            // error_download_entries <entries_count>
            out.log("error_download_entries")
                .arg(entries.len())
                .error(format_args!("Failed to download {} entries...", entries.len()));

            // error_download_entry <url> <dest> <mode> <error> [error_data...]
            for (entry, error) in entries {
            
                let mode_code = match entry.mode {
                    EntryMode::Force => "force",
                    EntryMode::Cache => "cache",
                };
                
                let mut log = out.log("error_download_entry");
                log.arg(&entry.source.url);
                log.arg(entry.file.display());
                log.arg(mode_code);

                log.additional(format_args!("{} -> {} ({mode_code})", entry.source.url, entry.file.display()));
                
                match error {
                    EntryError::Reqwest(error) => {
                        log.arg("request");
                        log.arg(&error);
                        log.args(error.source().into_iter());
                        if let Some(source) = error.source() {
                            log.additional(format_args!("  {error} (source: {source})"));
                        } else {
                            log.additional(format_args!("  {error}"));
                        }
                    }
                    EntryError::Io(error) => {
                        log.arg("io");
                        if let Some(error_kind_code) = io_error_kind_code(&error) {
                            log.arg(error_kind_code);
                        } else {
                            log.arg(format_args!("unknown:{error}"));
                        }
                        log.additional(format_args!("  I/O error: {error}"));
                    }
                    EntryError::InvalidStatus(status) => {
                        log.arg("invalid_status");
                        log.arg(status);
                        log.additional(format_args!("  Invalid status: {status}"));
                    }
                    EntryError::InvalidSize => {
                        log.arg("invalid_size");
                        log.additional(format_args!("  Invalid size"));
                    }
                    EntryError::InvalidSha1 => {
                        log.arg("invalid_size");
                        log.additional(format_args!("  Invalid SHA-1"));
                    }
                }

            }

        }
    }

}

/// Common function to log an I/O error to the user.
pub fn log_io_error(out: &mut Output, error: io::Error, file: Option<&Path>) {

    let mut log = out.log("error_io");

    if let Some(error_kind_code) = io_error_kind_code(&error) {
        log.arg(error_kind_code);
    } else {
        log.arg(format_args!("unknown:{error}"));
    }

    log.error(format_args!("I/O error: {error}"));

    if let Some(file) = file {
        log.arg(file.display());
        log.additional(format_args!("Related to {}", file.display()));
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
