//! PortableMC CLI.

use std::fmt::{self, Write as _};
use std::time::Instant;
use std::io::Write;

use portablemc::{download, standard, mojang};
use portablemc::msa;

// mod output;


fn main() {

    let mut handler = CliHandler::default();
    
    // match test_auth() {
    //     Ok(()) => handler
    //         .state("OK", format_args!("Authenticated"))
    //         .newline(),
    //     Err(e) => handler
    //         .state("FAILED", format_args!("Error: {e}"))
    //         .newline(),
    // };

    let res = mojang::Installer::new()
        // .root("1.16.4")
        .root("1.6.4")
        // .quick_play(QuickPlay::Multiplayer { host: "mc.hypixel.net".to_string(), port: 25565 })
        // .resolution(900, 900)
        // .demo(true)
        .auth_offline_username_authlib("Mindstorm38")
        .with_standard(|i| i
            .main_dir(r".minecraft_test")
            .strict_libraries_check(false)
            .strict_assets_check(false)
            .strict_jvm_check(false))
        .install(&mut handler);
    
    let game = match res {
        Ok(game) => game,
        Err(e) => {
            handler.newline();
            handler.state("FAILED", format_args!("{e}"));
            return;
        }
    };

    println!("game: {game:?}");
    match game.launch() {
        Ok(()) => (),
        Err(e) => {
            handler.newline();
            handler.state("FAILED", format_args!("{e}"));
        }
    }

}

fn test_auth() -> msa::Result<()> {

    let auth = msa::Auth::new("708e91b5-99f8-4a1d-80ec-e746cbb24771".to_string());
    let device_code_auth = auth.request_device_code()?;
    println!("{}", device_code_auth.message());

    let account = device_code_auth.wait()?;
    println!("account: {account:#?}");

    Ok(())

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

    /// Set the suffix to be displayed systematically after the line.
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
                self.state("INFO", format_args!("Rejected JVM version {version:?} at {}", file.display()))
                    .newline(),
            Event::JvmDynamicCrtUnsupported {  } => 
                self.state("INFO", format_args!("Couldn't find a Mojang JVM because your launcher is compiled with a static C runtime"))
                    .newline(),
            Event::JvmPlatformUnsupported {  } => 
                self.state("INFO", format_args!("Couldn't find a Mojang JVM because your platform is not supported"))
                    .newline(),
            Event::JvmDistributionNotFound {  } => 
                self.state("INFO", format_args!("Couldn't find a Mojang JVM because the required distribution was not found"))
                    .newline(),
            Event::JvmLoaded { file, version } => 
                self.state("OK", format_args!("Loaded JVM version {version:?} at {}", file.display()))
                    .newline(),
            Event::BinariesExtracted { dir } =>
                self.state("INFO", format_args!("Binaries extracted to {}", dir.display()))
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
            Event::FixLegacyQuickPlay {  } => 
                self.state("INFO", format_args!("Fixed: legacy quick play"))
                    .newline(),
            Event::FixLegacyProxy { host, port } => 
                self.state("INFO", format_args!("Fixed: legacy proxy ({host}:{port})"))
                    .newline(),
            Event::FixLegacyMergeSort {  } => 
                self.state("INFO", format_args!("Fixed: legacy merge sort"))
                    .newline(),
            Event::FixLegacyResolution {  } => 
                self.state("INFO", format_args!("Fixed: legacy resolution"))
                    .newline(),
            Event::FixBrokenAuthlib {  } => 
                self.state("INFO", format_args!("Fixed: broken authlib"))
                    .newline(),
            Event::QuickPlayNotSupported {  } => 
                self.state("WARN", format_args!("Quick play has been requested but is not supported"))
                    .newline(),
            Event::ResolutionNotSupported {  } => 
                self.state("WARN", format_args!("Resolution has been requested but is not supported"))
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
