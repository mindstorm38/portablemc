//! PortableMC CLI.

pub mod parse;
pub mod output;

use std::time::Instant;

use clap::Parser;

use portablemc::{download, standard, mojang};

use parse::{CliArgs, CliCmd, CliOutput, LoginArgs, LogoutArgs, SearchArgs, SearchKind, ShowArgs, StartArgs, StartResolution, StartVersion};
use output::{Output, LogOutput, LogLevel};


fn main() {
    
    let args = CliArgs::parse();
    let mut out = match args.output {
        CliOutput::Human => Output::human(match args.verbose {
            0 => LogLevel::Progress,
            1.. => LogLevel::Info,
        }),
        CliOutput::Machine => Output::tab_separated(),
    };

    println!("{args:?}");
    cmd_cli(&mut out, &args);

}

fn cmd_cli(out: &mut Output, args: &CliArgs) {
    match &args.cmd {
        CliCmd::Search(search_args) => cmd_search(out, args, search_args),
        CliCmd::Start(start_args) => cmd_start(out, args, start_args),
        CliCmd::Login(login_args) => cmd_login(out, args, login_args),
        CliCmd::Logout(logout_args) => cmd_logout(out, args, logout_args),
        CliCmd::Show(show_args) => cmd_show(out, args, show_args),
    }
}

fn cmd_search(out: &mut Output, cli_args: &CliArgs, args: &SearchArgs) {
    
    let _ = (out, cli_args);

    match args.kind {
        SearchKind::Mojang => cmd_search_mojang(&args.query),
        SearchKind::Local => todo!(),
        SearchKind::Forge => todo!(),
        SearchKind::Fabric => todo!(),
        SearchKind::Quilt => todo!(),
        SearchKind::LegacyFabric => todo!(),
    }

}

fn cmd_search_mojang(_query: &str) {
    mojang::request_manifest(()).unwrap();
}

fn cmd_start(out: &mut Output, cli_args: &CliArgs, args: &StartArgs) {
    
    // Internal function to apply args to the standard installer.
    fn apply_standard_args<'a>(
        installer: &'a mut standard::Installer, 
        cli_args: &CliArgs, 
        _args: &StartArgs,
    ) -> &'a mut standard::Installer {
        
        if let Some(dir) = &cli_args.main_dir {
            installer.main_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.versions_dir {
            installer.versions_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.libraries_dir {
            installer.libraries_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.assets_dir {
            installer.assets_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.jvm_dir {
            installer.jvm_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.bin_dir {
            installer.bin_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.work_dir {
            installer.work_dir(dir.clone());
        }

        installer
        
    }

    // Internal function to apply args to the mojang installer.
    fn apply_mojang_args<'a>(
        installer: &'a mut mojang::Installer,
        cli_args: &CliArgs, 
        args: &StartArgs,
    ) -> &'a mut mojang::Installer {

        installer.with_standard(|i| apply_standard_args(i, cli_args, args));
        installer.disable_multiplayer(args.disable_multiplayer);
        installer.disable_chat(args.disable_chat);
        installer.demo(args.demo);

        if let Some(StartResolution { width, height }) = args.resolution {
            installer.resolution(width, height);
        }

        if let Some(lwjgl) = &args.lwjgl {
            installer.fix_lwjgl(lwjgl.to_string());
        }

        for exclude_id in &args.exclude_fetch {
            if exclude_id == "*" {
                installer.fetch(false);
            } else {
                installer.fetch_exclude(exclude_id.clone());
            }
        }

        match (&args.username, &args.uuid) {
            (Some(username), None) => 
                installer.auth_offline_username_authlib(username.clone()),
            (None, Some(uuid)) =>
                installer.auth_offline_uuid(*uuid),
            (Some(username), Some(uuid)) =>
                installer.auth_offline(*uuid, username.clone()),
            (None, None) => installer, // nothing
        };

        if let Some(server) = &args.server {
            installer.quick_play(mojang::QuickPlay::Multiplayer { 
                host: server.clone(), 
                port: args.server_port,
            });
        }
        
        installer

    }

    let mut handler = InstallHandler::new(out.log());

    let game;

    match &args.version {
        StartVersion::Mojang { 
            root,
        } => {
            
            let mut inst = mojang::Installer::new();
            apply_mojang_args(&mut inst, cli_args, args);
            inst.root(root.clone());
            game = inst.install(&mut handler).unwrap();

        }
        StartVersion::Loader {
            root, 
            loader, 
            kind,
        } => {
            let _ = (root, loader, kind);
            todo!("start loader");
        }
    }

    if args.dry {
        return;
    }
    
    let _ = game;

    todo!()

}

fn cmd_login(out: &mut Output, cli_args: &CliArgs, args: &LoginArgs) {
    let _ = (out, cli_args, args);
}

fn cmd_logout(out: &mut Output, cli_args: &CliArgs, args: &LogoutArgs) {
    let _ = (out, cli_args, args);
}

fn cmd_show(out: &mut Output, cli_args: &CliArgs, args: &ShowArgs) {
    let _ = (out, cli_args, args);
}


/// Generic install handler.
#[derive(Debug)]
pub struct InstallHandler<'a> {
    /// Handle to the output.
    out: LogOutput<'a>,
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
}

impl<'a> InstallHandler<'a> {

    pub fn new(out: LogOutput<'a>) -> Self {
        Self {
            out,
            download_start: None,
        }
    }

}

impl download::Handler for InstallHandler<'_> {
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
        let (speed_fmt, speed_suffix) = output::number_si_unit(speed);
        let (size_fmt, size_suffix) = output::number_si_unit(size as f32);

        let mut log = self.out.background_log("download");
        if count == total_count {
            log.message(format_args!(" -- {speed_fmt:.1} {speed_suffix}B/s {size_fmt:.0} {size_suffix}B ({count})"));
        } else {
            log.message(format_args!(" -- {speed_fmt:.1} {speed_suffix}B/s {progress:.1}% ({count}/{total_count})"));
        }
        
        log.arg(format_args!("{count}/{total_count}"));
        log.arg(format_args!("{size}/{total_size}"));
        log.arg(format_args!("{}", elapsed.as_secs_f32()));
        log.arg(format_args!("{speed}"));
        
    }
}

impl standard::Handler for InstallHandler<'_> {
    fn handle_standard_event(&mut self, event: standard::Event) {
        
        use standard::Event;

        let out = &mut self.out;
        
        match event {
            Event::FeaturesFilter { .. } => {}
            Event::FeaturesLoaded { features } => {
                out.log("features_loaded")
                    .args(features.iter())
                    .info(format_args!("Features loaded: {features:?}"));
            }
            Event::HierarchyLoading { root_id } => {
                out.log("hierarchy_loading")
                    .arg(root_id)
                    .info(format_args!("Hierarchy loading from {root_id}"));
            }
            Event::HierarchyFilter { .. } => {}
            Event::HierarchyLoaded { hierarchy } => {
                out.log("hierarchy_loaded")
                    .args(hierarchy.iter().map(|v| &*v.id))
                    .info(format_args!("Hierarchy loaded: {hierarchy:?}"));
            }
            Event::VersionLoading { id, .. } => {
                out.log("version_loading")
                    .arg(id)
                    .progress(format_args!("Loading version {id}"));
            }
            Event::VersionNotFound { id, .. } => {
                out.log("version_not_found")
                    .arg(id)
                    .error(format_args!("Version {id} not found"));
            }
            Event::VersionLoaded { id, .. } => {
                out.log("version_loaded")
                    .arg(id)
                    .success(format_args!("Loaded version {id}"));
            }
            Event::ClientLoading {  } => {
                out.log("client_loading")
                    .progress("Loading client");
            }
            Event::ClientLoaded { file } => {
                out.log("client_loaded")
                    .arg(file.display())
                    .success("Loaded client");
            }
            Event::LibrariesLoading {  } => {
                out.log("libraries_loading")
                    .progress("Loading libraries");
            }
            Event::LibrariesFilter { .. } => {}
            Event::LibrariesLoaded { libraries } => {
                out.log("libraries_loaded")
                    .args(libraries.iter().map(|lib| &lib.gav))
                    .progress(format_args!("Loaded {} libraries", libraries.len()));
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
                    .progress(format_args!("Loading logger {id}"));
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
                    .progress(format_args!("Loading assets {id}"));
            }
            Event::AssetsLoaded { id, index } => {
                out.log("assets_loaded")
                    .arg(id)
                    .arg(index.objects.len())
                    .progress(format_args!("Loaded {} assets {id}", index.objects.len()));
            }
            Event::AssetsVerified { id, index } => {
                out.log("assets_verified")
                    .arg(id)
                    .arg(index.objects.len())
                    .success(format_args!("Loaded and verified {} assets {id}", index.objects.len()));
            }
            Event::ResourcesDownloading {  } => {
                out.log("resources_downloading")
                    .progress("Downloading");
            }
            Event::ResourcesDownloaded {  } => {
                out.log("resources_downloaded")
                    .success("Downloaded");
            }
            Event::JvmLoading { major_version } => {
                out.log("jvm_loading")
                    .arg(major_version)
                    .progress(format_args!("Loading JVM (preferred: {major_version:?}"));
            }
            Event::JvmVersionRejected { file, version } => {
                out.log("jvm_version_rejected")
                    .arg(file.display())
                    .args(version.into_iter())
                    .info(format_args!("Rejected JVM version {version:?} at {}", file.display()));
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
                out.log("jvm_loaded")
                    .arg(file.display())
                    .args(version.into_iter())
                    .success(format_args!("Loaded JVM version {version:?} at {}", file.display()));
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

impl mojang::Handler for InstallHandler<'_> {
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        
        use mojang::Event;

        let out = &mut self.out;

        match event {
            Event::MojangVersionInvalidated { id } => {
                out.log("mojang_version_invalidated")
                    .arg(id)
                    .info(format_args!("Mojang version {id} invalidated"));
            }
            Event::MojangVersionFetching { id } => {
                out.log("mojang_version_fetching")
                    .arg(id)
                    .progress(format_args!("Fetching Mojang version {id}"));
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

// #[allow(unused)]
// fn test_auth() -> msa::Result<()> {

//     let auth = msa::Auth::new("708e91b5-99f8-4a1d-80ec-e746cbb24771".to_string());
//     let device_code_auth = auth.request_device_code()?;
//     println!("{}", device_code_auth.message());

//     let account = device_code_auth.wait()?;
//     println!("account: {account:#?}");

//     Ok(())

// }

