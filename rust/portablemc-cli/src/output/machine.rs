use std::io::{self, StderrLock, StdoutLock, Write};
use std::fmt::Display;

use portablemc::{download, mojang, standard};

use super::DownloadTracker;


/// A handle to writing a single of tab-separated values on stdout.
#[derive(Debug)]
pub struct TabWriter<W: Write> {
    inner: W,
}

impl<W: Write> Drop for TabWriter<W> {
    fn drop(&mut self) {
        let _ = writeln!(self.inner);
        let _ = self.inner.flush();
    }
}

impl TabWriter<StdoutLock<'static>> {

    pub fn stdout() -> Self {
        Self {
            inner: io::stdout().lock(),
        }
    }
    
}

impl TabWriter<StderrLock<'static>> {

    pub fn stderr() -> Self {
        Self {
            inner: io::stderr().lock(),
        }
    }

}

impl<W: Write> TabWriter<W> {

    pub fn arg(&mut self, value: impl Display) -> &mut Self {
        write!(self.inner, "\t{value}").unwrap();
        self
    }

    pub fn some_arg(&mut self, value: Option<impl Display>) -> &mut Self {
        if let Some(value) = value {
            self.arg(value);
        }
        self
    }

    #[inline]
    pub fn args<D, I>(&mut self, values: I) -> &mut Self 
    where
        D: Display,
        I: Iterator<Item = D>,
    {
        for value in values {
            self.arg(value);
        }
        self
    }

}







/// Installation handler for machine-readable output.
#[derive(Debug)]
pub struct MachineHandler {
    /// Internal machine writer.
    writer: MachineWriter,
    /// Internal download handler.
    download: DownloadTracker,
}

impl MachineHandler {

    pub fn new() -> Self {
        Self {
            writer: MachineWriter::new(),
            download: DownloadTracker::new(),
        }
    }

}

impl download::Handler for MachineHandler {

    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        
        let Some(metrics) = self.download.handle(count, total_count, size, total_size) else {
            return;
        };

        self.writer.line(format_args!("download"))
            .arg(format_args!("count:{count}/{total_count}"))
            .arg(format_args!("size:{size}/{total_size}"))
            .arg(format_args!("elapsed:{}", metrics.elapsed.as_secs_f32()))
            .arg(format_args!("tspeed:{}", metrics.speed));

    }

}

impl standard::Handler for MachineHandler {

    fn handle_standard_event(&mut self, event: standard::Event) { 
        
        use standard::Event;

        let writer = &mut self.writer;

        match event {
            Event::FeaturesFilter { .. } => {}
            Event::FeaturesLoaded { features } => {
                writer.line("features_loaded")
                    .args(features.iter());
            }
            Event::HierarchyLoading { root_id } => {
                writer.line("hierarchy_loading").arg(root_id);
            }
            Event::HierarchyFilter { .. } => {}
            Event::HierarchyLoaded { hierarchy } => {
                writer.line("features_loaded")
                    .args(hierarchy.iter().map(|v| &*v.id));
            }
            Event::VersionLoading { id, .. } => {
                writer.line("version_loading").arg(id);
            }
            Event::VersionNotFound { .. } => {}
            Event::VersionLoaded { id, .. } => {
                writer.line("version_loaded").arg(id);
            }
            Event::ClientLoading {  } => {}
            Event::ClientLoaded { .. } => {}
            Event::LibrariesLoading {  } => {
                writer.line("libraries_loading");
            }
            Event::LibrariesFilter { .. } => {}
            Event::LibrariesLoaded { libraries } => {
                writer.line("libraries_loaded")
                    .args(libraries.iter().map(|l| &l.gav));
            }
            Event::LibrariesFilesFilter { .. } => {}
            Event::LibrariesFilesLoaded { class_files, natives_files } => {
                writer.line("class_files_loaded")
                    .args(class_files.iter().map(|p| p.display()));
                writer.line("natives_files")
                    .args(natives_files.iter().map(|p| p.display()));
            }
            Event::LoggerAbsent {  } => {
                writer.line("logger_absent");
            }
            Event::LoggerLoading { id } => {
                writer.line("logger_loading").arg(id);
            }
            Event::LoggerLoaded { id } => {
                writer.line("logger_loaded").arg(id);
            }
            Event::AssetsAbsent {  } => {
                writer.line("assets_absent");
            }
            Event::AssetsLoading { id } => {
                writer.line("assets_loading").arg(id);
            }
            Event::AssetsLoaded { id, .. } => {
                writer.line("assets_loaded").arg(id);
            }
            Event::AssetsVerified { id, .. } => {
                writer.line("assets_verified").arg(id);
            }
            Event::JvmLoading { major_version } => {
                writer.line("jvm_loading").arg(major_version);
            }
            Event::JvmVersionRejected { file, version } => {
                writer.line("jvm_version_rejected")
                    .arg(file.display())
                    .some_arg(version);
            }
            Event::JvmDynamicCrtUnsupported {  } => {
                writer.line("jvm_dynamic_crt_unsupported");
            }
            Event::JvmPlatformUnsupported {  } => {
                writer.line("jvm_platform_unsupported");
            }
            Event::JvmDistributionNotFound {  } => {
                writer.line("jvm_distribution_not_found");
            }
            Event::JvmLoaded { file, version } => {
                writer.line("jvm_loaded")
                    .arg(file.display())
                    .some_arg(version);
            }
            Event::ResourcesDownloading {  } => {
                writer.line("resources_downloading");
            }
            Event::ResourcesDownloaded {  } => {
                writer.line("resources_downloaded");
            }
            Event::BinariesExtracted { dir } => {
                writer.line("binaries_extracted").arg(dir.display());
            }
            _ => todo!("{event:?}")
        };

    }

}

impl mojang::Handler for MachineHandler {
    
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        
        use mojang::Event;

        let writer = &mut self.writer;

        match event {
            Event::MojangVersionInvalidated { id } => {
                writer.line("mojang_version_invalidated").arg(id);
            }
            Event::MojangVersionFetching { id } => {
                writer.line("mojang_version_fetching").arg(id);
            }
            Event::MojangVersionFetched { id } => {
                writer.line("mojang_version_fetched").arg(id);
            }
            Event::FixLegacyQuickPlay {  } => {
                writer.line("fix_legacy_quick_play");
            }
            Event::FixLegacyProxy { host, port } => {
                writer.line("fix_legacy_proxy").arg(host).arg(port);
            }
            Event::FixLegacyMergeSort {  } => {
                writer.line("fix_legacy_merge_sort");
            }
            Event::FixLegacyResolution {  } => {
                writer.line("fix_legacy_resolution");
            }
            Event::FixBrokenAuthlib {  } => {
                writer.line("fix_broken_authlib");
            }
            Event::QuickPlayNotSupported {  } => {
                writer.line("quick_play_not_supported");
            }
            Event::ResolutionNotSupported {  } => {
                writer.line("resolution_not_supported");
            }
            _ => todo!("{event:?}")
        }

    }

}
