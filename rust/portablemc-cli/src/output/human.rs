use std::{env, io};
use std::fmt::{self, Write as _};
use std::io::{IsTerminal, Write as _};

use portablemc::{download, mojang, standard};

use super::DownloadTracker;


/// A utility output for printing state message that can be updated dynamically.
#[derive(Debug)]
pub struct LogWriter {
    /// Enable color for known states.
    color: bool,
    /// The buffer containing the whole rendered line.
    line_buf: String,
    /// Current terminal width of the line.
    line_term_width: usize,
    /// The buffer containing the full rendered suffix.
    suffix_buf: String,
}

impl LogWriter {

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
            line_term_width: 0,
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

    /// Update the printed line after the buffer has been updated.
    fn print_line(&mut self) -> &mut Self {

        let term_width = terminal_width(&self.line_buf);
        let padding_width = self.line_term_width.saturating_sub(term_width);
        self.line_term_width = term_width;
        
        let mut stdout = std::io::stdout().lock();
        let _ = write!(stdout, "\r{}{:padding_width$}", self.line_buf, "");
        let _ = stdout.flush();

        self

    }

    /// Update the current line.
    pub fn line(&mut self, message: fmt::Arguments) -> &mut Self {
        self.line_buf.clear();
        write!(self.line_buf, "{}{}", message, self.suffix_buf).unwrap();
        self.print_line()
    }

    /// Set the suffix to be displayed systematically after the line.
    pub fn suffix(&mut self, message: fmt::Arguments) -> &mut Self {

        let last_suffix_len = self.suffix_buf.len();
        self.suffix_buf.clear();
        self.suffix_buf.write_fmt(message).unwrap();

        let last_line_len = self.line_buf.len();
        self.line_buf.truncate(last_line_len - last_suffix_len);
        self.line_buf.push_str(&self.suffix_buf);
        
        self.print_line()

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


/// A utility to write table to stdout.
#[derive(Debug)]
pub struct TableWriter {
    
}

impl TableWriter {

    pub fn new() -> Self {
        Self {

        }
    }

}


/// Installation handler for human-readable output.
#[derive(Debug)]
pub struct HumanHandler {
    /// Internal state output writer.
    state: LogWriter,
    /// Internal download handler.
    download: DownloadTracker,
}

impl HumanHandler {

    pub fn new(color: bool) -> Self {
        Self {
            state: LogWriter::new(color),
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

/// Return true if color should be used on terminal.
/// 
/// Supporting `NO_COLOR` (https://no-color.org/) and `TERM=dumb`.
pub fn has_color() -> bool {
    if cfg!(unix) && env::var_os("TERM").map(|term| term == "dumb").unwrap_or_default() {
        false
    } else if env::var_os("NO_COLOR").map(|s| !s.is_empty()).unwrap_or_default() {
        false
    } else {
        true
    }
}

/// Return true if color can be printed to stdout.
/// 
/// See [`has_color()`].
pub fn has_stdout_color() -> bool {
    if !io::stdout().is_terminal() {
        false
    } else {
        has_color()
    }
}

/// Find the SI unit of a given number and return the number scaled down to that unit.
pub fn number_si_unit(num: f32) -> (f32, char) {
    match num {
        ..=999.0 => (num, ' '),
        ..=999_999.0 => (num / 1_000.0, 'k'),
        ..=999_999_999.0 => (num / 1_000_000.0, 'M'),
        _ => (num / 1_000_000_000.0, 'G'),
    }
}

/// Compute terminal display length of a given string.
pub fn terminal_width(s: &str) -> usize {

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Control {
        None,
        Escape,
        Csi,
    }

    let mut len = 0;
    let mut control = Control::None;

    for ch in s.chars() {
        match (control, ch) {
            (Control::None, '\x1b') => {
                control = Control::Escape;
            }
            (Control::None, c) if !c.is_control() => {
                len += 1;
            }
            (Control::Escape, '[') => {
                control = Control::Csi;
            }
            (Control::Escape, _) => {
                control = Control::None;
            }
            (Control::Csi, c) if c.is_alphabetic() => {
                // After a CSI control any alphabetic char is terminating the sequence.
                control = Control::None;
            }
            _ => {}
        }
    }

    len

}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn check_terminal_width() {
        assert_eq!(terminal_width(""), 0);
        assert_eq!(terminal_width("\x1b"), 0);
        assert_eq!(terminal_width("\x1b[92m"), 0);
        assert_eq!(terminal_width("\x1b[92mOK"), 2);
        assert_eq!(terminal_width("[  \x1b[92mOK"), 5);
        assert_eq!(terminal_width("[  \x1b[92mOK  ]"), 8);
        assert_eq!(terminal_width("[  \x1b[92mOK  \x1b[0m]"), 8);
    }

}
