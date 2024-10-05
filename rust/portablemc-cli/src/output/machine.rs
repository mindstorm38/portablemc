use portablemc::{download, mojang, standard};

use super::DownloadTracker;


/// Installation handler for machine-readable output.
#[derive(Debug)]
pub struct MachineHandler {
    /// Internal download handler.
    download: DownloadTracker,
}

impl MachineHandler {

    pub fn new() -> Self {
        Self {
            download: DownloadTracker::new(),
        }
    }

}

impl download::Handler for MachineHandler {

    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        
        let Some(metrics) = self.download.handle(count, total_count, size, total_size) else {
            return;
        };

        println!("download\tcount:{count}/{total_count}\tsize:{size}/{total_size}\telapsed:{}\tspeed:{}", 
            metrics.elapsed.as_secs_f32(),
            metrics.speed);

    }

}

impl standard::Handler for MachineHandler {

    fn handle_standard_event(&mut self, event: standard::Event) { 
        
        use standard::Event;

        match event {
            Event::FeaturesFilter { .. } => {}
            Event::FeaturesLoaded { features } => {
                print!("features_loaded");
                for feature in features {
                    print!("\t{feature}");
                }
                println!();
            }
            Event::HierarchyLoading { root_id } => {
                println!("hierarchy_loading\t{root_id}");
            }
            Event::HierarchyFilter { .. } => {}
            Event::HierarchyLoaded { hierarchy } => {
                print!("hierarchy_loaded");
                for version in hierarchy {
                    print!("\t{}", version.id);
                }
                println!();
            }
            Event::VersionLoading { id, .. } => {
                println!("version_loading\t{id}");
            }
            Event::VersionNotFound { .. } => {}
            Event::VersionLoaded { id, .. } => {
                println!("version_loaded\t{id}");
            }
            Event::ClientLoading {  } => {}
            Event::ClientLoaded { .. } => {}
            Event::LibrariesLoading {  } => {
                println!("libraries_loading\t");
            }
            Event::LibrariesFilter { .. } => {}
            Event::LibrariesLoaded { libraries } => {
                print!("libraries_loaded");
                for lib in libraries {
                    print!("\t{}", lib.gav);
                }
                println!();
            }
            Event::LibrariesFilesFilter { .. } => {}
            Event::LibrariesFilesLoaded { class_files, natives_files } => {
                print!("class_files_loaded\t");
                for file in class_files {
                    print!("\t{}", file.display());
                }
                println!();
                print!("natives_files_loaded\t");
                for file in natives_files {
                    print!("\t{}", file.display());
                }
                println!();
            }
            Event::LoggerAbsent {  } =>
                println!("logger_absent"),
            Event::LoggerLoading { id } =>
                println!("logger_loading\t{id}"),
            Event::LoggerLoaded { id } =>
                println!("logger_loaded\t{id}"),
            Event::AssetsAbsent {  } => 
                println!("assets_absent"),
            Event::AssetsLoading { id } => 
                println!("assets_loading\t{id}"),
            Event::AssetsLoaded { id, .. } => 
                println!("assets_loaded\t{id}"),
            Event::AssetsVerified { id, .. } => 
                println!("assets_verified\t{id}"),
            Event::JvmLoading { major_version } => 
                println!("jvm_loading\t{major_version}"),
            Event::JvmVersionRejected { file, version } => {
                print!("jvm_version_rejected\t{}", file.display());
                if let Some(version) = version {
                    print!("\t{version}");
                }
                println!();
            }
            Event::JvmDynamicCrtUnsupported {  } => 
                println!("jvm_dynamic_crt_unsupported"),
            Event::JvmPlatformUnsupported {  } => 
                println!("jvm_platform_unsupported"),
            Event::JvmDistributionNotFound {  } => 
                println!("jvm_distribution_not_found"),
            Event::JvmLoaded { file, version } => {
                print!("jvm_loaded\t{}", file.display());
                if let Some(version) = version {
                    print!("\t{version}");
                }
                println!();
            }
            Event::ResourcesDownloading {  } => 
                println!("resources_downloading"),
            Event::ResourcesDownloaded {  } => 
                println!("resources_downloaded"),
            Event::BinariesExtracted { dir } => 
                println!("binaries_extracted\t{}", dir.display()),
            _ => todo!("{event:?}")
        };

    }

}

impl mojang::Handler for MachineHandler {
    
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        
        use mojang::Event;

        match event {
            Event::MojangVersionInvalidated { id } => 
                println!("mojang_version_invalidated\t{id}"),
            Event::MojangVersionFetching { id } => 
                println!("mojang_version_fetching\t{id}"),
            Event::MojangVersionFetched { id } => 
                println!("mojang_version_fetched\t{id}"),
            Event::FixLegacyQuickPlay {  } => 
                println!("fix_legacy_quick_play"),
            Event::FixLegacyProxy { host, port } => 
                println!("fix_legacy_proxy\t{host}\t{port}"),
            Event::FixLegacyMergeSort {  } => 
                println!("fix_legacy_merge_sort"),
            Event::FixLegacyResolution {  } => 
                println!("fix_legacy_resolution"),
            Event::FixBrokenAuthlib {  } => 
                println!("fix_broken_authlib"),
            Event::QuickPlayNotSupported {  } => 
                println!("quick_play_not_supported"),
            Event::ResolutionNotSupported {  } => 
                println!("resolution_not_supported"),
            _ => todo!("{event:?}")
        }

    }

}
