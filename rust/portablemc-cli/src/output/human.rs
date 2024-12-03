use std::fmt::{self, Write as _};
use std::io::Write as _;

use portablemc::{download, mojang, standard};

use super::DownloadTracker;


/// Find the SI unit of a given number and return the number scaled down to that unit.
pub fn number_si_unit(num: f32) -> (f32, char) {
    match num {
        ..=999.0 => (num, ' '),
        ..=999_999.0 => (num / 1_000.0, 'k'),
        ..=999_999_999.0 => (num / 1_000_000.0, 'M'),
        _ => (num / 1_000_000_000.0, 'G'),
    }
}

/// A utility output for printing state
#[derive(Debug)]
pub struct Output {
    /// Enable color for known states.
    color: bool,
    /// The buffer containing the whole rendered line.
    line_buf: String,
    /// The buffer containing the full rendered suffix.
    suffix_buf: String,
}

impl Output {

    const STATE_COLOR: &'static [(&'static str, &'static str)] = &[
        ("OK", "\x1b[92m"),
        ("FAILED", "\x1b[31m"),
        ("WARN", "\x1b[33m"),
        ("INFO", "\x1b[34m"),
    ];

    pub fn new(color: bool) -> Self {
        Self {
            color,
            line_buf: String::new(),
            suffix_buf: String::new(),
        }
    }

    /// Return the current line length, not count the suffix.
    pub fn line_len(&self) -> usize {
        self.line_buf.len() - self.suffix_buf.len()
    }

    /// Return the current suffix length.
    pub fn suffix_len(&self) -> usize {
        self.suffix_buf.len()
    }

    /// Update the current line.
    pub fn line(&mut self, message: fmt::Arguments) -> &mut Self {

        let last_line_len = self.line_buf.len();
        self.line_buf.clear();
        write!(self.line_buf, "{}{}", message, self.suffix_buf).unwrap();
        
        let mut stdout = std::io::stdout().lock();
        let _ = write!(stdout, "\r{:last_line_len$}", self.line_buf);
        let _ = stdout.flush();

        self
        
    }

    /// Set the suffix to be displayed systematically after the line.
    pub fn suffix(&mut self, message: fmt::Arguments) -> &mut Self {

        let last_suffix_len = self.suffix_buf.len();
        self.suffix_buf.clear();
        self.suffix_buf.write_fmt(message).unwrap();

        let last_line_len = self.line_buf.len();
        self.line_buf.truncate(last_line_len - last_suffix_len);
        self.line_buf.push_str(&self.suffix_buf);

        let mut stdout = std::io::stdout().lock();
        let _ = write!(stdout, "\r{:last_line_len$}", self.line_buf);
        let _ = stdout.flush();

        self

    }

    /// Update the current state.
    pub fn state(&mut self, state: &str, message: fmt::Arguments) -> &mut Self {
        if self.color {

            let color_code = Self::STATE_COLOR.iter()
                .find(|&&(s, _)| s == state)
                .map(|&(_, code)| code)
                .unwrap_or_default();
            
            self.line(format_args!("[{color_code}{state:^6}\x1b[0m] {message}"))

        } else {
            self.line(format_args!("[{state:^6}] {message}"))
        }
    }

    /// Add a newline and reset the buffer, only if there was a preview.
    pub fn newline(&mut self) -> &mut Self {
        if self.line_buf.is_empty() {
            return self;
        }
        self.line_buf.clear();
        self.suffix_buf.clear();
        println!();
        self
    }

}


/// Installation handler for human-readable output.
#[derive(Debug)]
pub struct HumanHandler {
    /// Internal state output writer.
    state: Output,
    /// Internal download handler.
    download: DownloadTracker,
}

impl HumanHandler {

    pub fn new(color: bool) -> Self {
        Self {
            state: Output::new(color),
            download: DownloadTracker::new(),
        }
    }

}

impl download::Handler for HumanHandler {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        
        let Some(metrics) = self.download.handle(count, total_count, size, total_size) else {
            return;
        };

        let progress = size as f32 / total_size as f32 * 100.0;
        let (speed, speed_suffix) = number_si_unit(metrics.speed);
        let (size, size_suffix) = number_si_unit(size as f32);

        if count == total_count {
            self.state.suffix(format_args!(" -- {speed:.1} {speed_suffix}B/s {size:.0} {size_suffix}B ({count})"));
        } else {
            self.state.suffix(format_args!(" -- {speed:.1} {speed_suffix}B/s {progress:.1}% ({count}/{total_count})"));
        }
        
    }
}

impl standard::Handler for HumanHandler {
    fn handle_standard_event(&mut self, event: standard::Event) {
        
        use standard::Event;

        let state = &mut self.state;
        
        match event {
            Event::FeaturesFilter { .. } => state,
            Event::FeaturesLoaded { .. } => state,
            Event::HierarchyLoading { .. } => state,
            Event::HierarchyFilter { .. } => state,
            Event::HierarchyLoaded { .. } => state,
            Event::VersionLoading { id, .. } => 
                state.state("..", format_args!("Loading version {id}")),
            Event::VersionNotFound { id, .. } =>
                state.state("FAILED", format_args!("Version {id} not found"))
                    .newline(),
            Event::VersionLoaded { id, .. } => 
                state.state("OK", format_args!("Loaded version {id}"))
                    .newline(),
            Event::ClientLoading {  } => 
                state.state("..", format_args!("Loading client")),
            Event::ClientLoaded { .. } => 
                state.state("OK", format_args!("Loaded client"))
                    .newline(),
            Event::LibrariesLoading {  } => 
                state.state("..", format_args!("Loading libraries")),
            Event::LibrariesFilter { .. } => state,
            Event::LibrariesLoaded { libraries } => 
                state.state("..", format_args!("Loaded {} libraries", libraries.len())),
            Event::LibrariesFilesFilter { .. } => state,
            Event::LibrariesFilesLoaded { class_files, natives_files } => 
                state.state("OK", format_args!("Loaded and verified {}+{} libraries", class_files.len(), natives_files.len()))
                    .newline(),
            Event::LoggerAbsent {  } => 
                state.state("OK", format_args!("No logger"))
                    .newline(),
            Event::LoggerLoading { id } => 
                state.state("..", format_args!("Loading logger {id}")),
            Event::LoggerLoaded { id } => 
                state.state("OK", format_args!("Loaded logger {id}"))
                    .newline(),
            Event::AssetsAbsent {  } => 
                state.state("OK", format_args!("No assets"))
                    .newline(),
            Event::AssetsLoading { id } => 
                state.state("..", format_args!("Loading assets {id}")),
            Event::AssetsLoaded { id, index } => 
                state.state("..", format_args!("Loaded {} assets {id}", index.objects.len())),
            Event::AssetsVerified { id, index } => 
                state.state("OK", format_args!("Loaded and verified {} assets {id}", index.objects.len()))
                    .newline(),
            Event::ResourcesDownloading {  } =>
                state.state("..", format_args!("Downloading")),
            Event::ResourcesDownloaded {  } =>
                state.state("OK", format_args!("Downloaded"))
                    .newline(),
            Event::JvmLoading { major_version } => 
                state.state("..", format_args!("Loading JVM (preferred: {major_version:?}")),
            Event::JvmVersionRejected { file, version } =>
                state.state("INFO", format_args!("Rejected JVM version {version:?} at {}", file.display()))
                    .newline(),
            Event::JvmDynamicCrtUnsupported {  } => 
                state.state("INFO", format_args!("Couldn't find a Mojang JVM because your launcher is compiled with a static C runtime"))
                    .newline(),
            Event::JvmPlatformUnsupported {  } => 
                state.state("INFO", format_args!("Couldn't find a Mojang JVM because your platform is not supported"))
                    .newline(),
            Event::JvmDistributionNotFound {  } => 
                state.state("INFO", format_args!("Couldn't find a Mojang JVM because the required distribution was not found"))
                    .newline(),
            Event::JvmLoaded { file, version } => 
                state.state("OK", format_args!("Loaded JVM version {version:?} at {}", file.display()))
                    .newline(),
            Event::BinariesExtracted { dir } =>
                state.state("INFO", format_args!("Binaries extracted to {}", dir.display()))
                    .newline(),
            _ => todo!("{event:?}")
        };

    }
}

impl mojang::Handler for HumanHandler {
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        
        use mojang::Event;

        let state = &mut self.state;

        match event {
            Event::MojangVersionInvalidated { id } => 
                state.state("OK", format_args!("Mojang version {id} invalidated"))
                    .newline(),
            Event::MojangVersionFetching { id } => 
                state.state("..", format_args!("Fetching Mojang version {id}")),
            Event::MojangVersionFetched { id } =>
                state.state("OK", format_args!("Fetched Mojang version {id}"))
                    .newline(),
            Event::FixLegacyQuickPlay {  } => 
                state.state("INFO", format_args!("Fixed: legacy quick play"))
                    .newline(),
            Event::FixLegacyProxy { host, port } => 
                state.state("INFO", format_args!("Fixed: legacy proxy ({host}:{port})"))
                    .newline(),
            Event::FixLegacyMergeSort {  } => 
                state.state("INFO", format_args!("Fixed: legacy merge sort"))
                    .newline(),
            Event::FixLegacyResolution {  } => 
                state.state("INFO", format_args!("Fixed: legacy resolution"))
                    .newline(),
            Event::FixBrokenAuthlib {  } => 
                state.state("INFO", format_args!("Fixed: broken authlib"))
                    .newline(),
            Event::QuickPlayNotSupported {  } => 
                state.state("WARN", format_args!("Quick play has been requested but is not supported"))
                    .newline(),
            Event::ResolutionNotSupported {  } => 
                state.state("WARN", format_args!("Resolution has been requested but is not supported"))
                    .newline(),
            _ => todo!("{event:?}")
        };

    }
}
