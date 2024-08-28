//! PortableMC CLI.

use std::fmt::{self, Write as _};
use std::time::Instant;
use std::io::Write;

use portablemc::{download, standard, mojang};

// mod output;


fn main() {

    let mut handler = CliHandler::default();

    let mut installer = standard::Installer::with_dir(r"C:\Users\theor\AppData\Roaming\.minecraft_test".into());
    installer.strict_libraries_check = false;
    installer.strict_assets_check = false;

    let installer = mojang::Installer::from(installer);
    let game = match installer.install(&mut handler, "1.21") {
        Ok(v) => v,
        Err(e) => {
            handler.newline();
            handler.state("FAILED", format_args!("{e}"));
            return;
        }
    };

    let _ = game;
    // let _ = game.finalize();

}

#[derive(Debug, Default)]
struct CliHandler {
    /// The buffer containing the whole rendered line.
    line_buf: String,
    /// The buffer containing the full rendered suffix.
    suffix_buf: String,
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
}

impl CliHandler {

    /// Update the current line.
    fn line(&mut self, message: fmt::Arguments) -> &mut Self {

        let last_line_len = self.line_buf.len();
        self.line_buf.clear();
        write!(self.line_buf, "{} {}", message, self.suffix_buf).unwrap();
        
        let mut stdout = std::io::stdout().lock();
        let _ = write!(stdout, "\r{:last_line_len$}", self.line_buf);
        let _ = stdout.flush();

        self
        
    }

    /// Set the suffix to be displayed systematically after
    fn suffix(&mut self, message: fmt::Arguments) -> &mut Self {

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
    fn state(&mut self, state: &str, message: fmt::Arguments) -> &mut Self {
        self.line(format_args!("[{state:^6}] {message}"))
    }

    /// Add a newline and reset the buffer, only if there was a preview.
    fn newline(&mut self) -> &mut Self {
        if self.line_buf.is_empty() {
            return self;
        }
        self.line_buf.clear();
        self.suffix_buf.clear();
        println!();
        self
    }

}

impl download::Handler for CliHandler {
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
        let progress = size as f32 / total_size as f32 * 100.0;

        let (speed, speed_suffix) = number_si_unit(speed);
        let (size, size_suffix) = number_si_unit(size as f32);

        let sep = self.line_buf.is_empty().then_some("").unwrap_or("-- ");
        
        if count == total_count {
            self.download_start = None;
            self.suffix(format_args!("{sep}{speed:.1} {speed_suffix}B/s {size:.0} {size_suffix}B ({count})"));
        } else {
            self.suffix(format_args!("{sep}{speed:.1} {speed_suffix}B/s {progress:.1}% ({count}/{total_count})"));
        }
        
    }
}

impl standard::Handler for CliHandler {
    fn handle_standard_event(&mut self, event: standard::Event) {
        
        use standard::Event;

        match event {
            Event::FeaturesLoaded { .. } => self,
            Event::HierarchyLoading { .. } => self,
            Event::HierarchyLoaded { .. } => self,
            Event::VersionLoading { id, .. } => 
                self.state("..", format_args!("Loading version {id}")),
            Event::VersionNotFound { id, .. } =>
                self.state("FAILED", format_args!("Version {id} not found"))
                    .newline(),
            Event::VersionLoaded { id, .. } => 
                self.state("OK", format_args!("Loaded version {id}"))
                    .newline(),
            Event::ClientLoading {  } => 
                self.state("..", format_args!("Loading client")),
            Event::ClientLoaded { .. } => 
                self.state("OK", format_args!("Loaded client"))
                    .newline(),
            Event::LibrariesLoading {  } => 
                self.state("..", format_args!("Loading libraries")),
            Event::LibrariesLoaded { libraries } => 
                self.state("..", format_args!("Loaded {} libraries", libraries.len())),
            Event::LibrariesVerified { class_files, natives_files } => 
                self.state("OK", format_args!("Loaded and verified {}+{} libraries", class_files.len(), natives_files.len()))
                    .newline(),
            Event::LoggerAbsent {  } => 
                self.state("OK", format_args!("No logger"))
                    .newline(),
            Event::LoggerLoading { id } => 
                self.state("..", format_args!("Loading logger {id}")),
            Event::LoggerLoaded { id } => 
                self.state("OK", format_args!("Loaded logger {id}"))
                    .newline(),
            Event::AssetsAbsent {  } => 
                self.state("OK", format_args!("No assets"))
                    .newline(),
            Event::AssetsLoading { id } => 
                self.state("..", format_args!("Loading assets {id}")),
            Event::AssetsLoaded { id, index } => 
                self.state("..", format_args!("Loaded {} assets {id}", index.objects.len())),
            Event::AssetsVerified { id, index } => 
                self.state("OK", format_args!("Loaded and verified {} assets {id}", index.objects.len()))
                    .newline(),
            Event::ResourcesDownloading {  } =>
                self.state("..", format_args!("Downloading")),
            Event::ResourcesDownloaded {  } =>
                self.state("OK", format_args!("Downloaded"))
                    .newline(),
            Event::JvmLoading { major_version } => 
                self.state("..", format_args!("Loading JVM (preferred: {major_version:?}")),
            Event::JvmVersionRejected { file, version } =>
                self.state("INFO", format_args!("Rejected JVM version {version:?} at {file:?}")),
            Event::JvmLoaded { file, version } => 
                self.state("OK", format_args!("Loaded JVM version {version:?} at {file:?}"))
                    .newline(),
            _ => todo!(),
            
        };

    }
}

impl mojang::Handler for CliHandler {
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        
        use mojang::Event;

        match event {
            Event::MojangVersionInvalidated { id } => 
                self.state("OK", format_args!("Mojang version {id} invalidated"))
                    .newline(),
            Event::MojangVersionFetching { id } => 
                self.state("..", format_args!("Fetching Mojang version {id}")),
            Event::MojangVersionFetched { id } =>
                self.state("OK", format_args!("Fetched Mojang version {id}"))
                    .newline(),
            _ => todo!(),
        };

    }
}

fn number_si_unit(num: f32) -> (f32, char) {
    match num {
        ..=999.0 => (num, ' '),
        ..=999_999.0 => (num / 1_000.0, 'k'),
        ..=999_999_999.0 => (num / 1_000_000.0, 'M'),
        _ => (num / 1_000_000_000.0, 'G'),
    }
}
