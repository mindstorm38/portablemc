//! PortableMC CLI.

use std::time::Instant;
use std::io::Write;

use portablemc::{download, standard};


fn main() {
    
    let mut installer = standard::Installer::with_dirs(r"C:\Users\Theo\AppData\Roaming\.minecraft_test".into(), r"C:\Users\Theo\AppData\Roaming\.minecraft_test".into());
    installer.strict_libraries_checking = false;
    installer.strict_assets_checking = false;

    installer.install(&mut DebugHandler::default(), "1.21").unwrap();

}

#[derive(Debug, Default)]
struct DebugHandler {
    download_start: Option<Instant>,
}

impl download::Handler for DebugHandler {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        
        if self.download_start.is_none() {
            self.download_start = Some(Instant::now());
        }

        if size == 0 {
            return;
        }

        let elapsed = self.download_start.unwrap().elapsed();

        let speed = size as f32 / elapsed.as_secs_f32() / 1_000_000.0;
        let progress = size as f32 / total_size as f32 * 100.0;

        print!("\r[  ..  ] Downloading... {speed:.0} MB/s {progress:5.1}% ({count}/{total_count})");

        if count == total_count {
            self.download_start = None;
            print!("\r[  OK  ]\n");
        }
        
        std::io::stdout().flush().unwrap();

    }
}

impl standard::Handler for DebugHandler {
    
    fn handle_standard_event(&mut self, event: standard::Event) {
        
        use standard::Event;

        match event {
            Event::HierarchyLoading { .. } => (),
            Event::HierarchyLoaded { .. } => (),
            Event::VersionLoading { id, .. } => 
                print!(  "[  ..  ] Loading version {id}"),
            Event::VersionNotFound { .. } =>
                print!("\r[FAILED] \n"),
            Event::VersionLoaded { id, .. } => 
                print!("\r[  OK  ] Loaded version {id} \n"),
            Event::ClientLoading {  } => 
                print!(  "[  ..  ] Loading client"),
            Event::ClientLoaded {  } => 
                print!("\r[  OK  ] Loaded client \n"),
            Event::LibrariesLoading {  } => 
                print!(  "[  ..  ] Loading libraries"),
            Event::LibrariesLoaded { libraries } => 
                print!("\r[  ..  ] Loaded {} libraries", libraries.len()),
            Event::LibrariesVerified { class_files, natives_files } => 
                print!("\r[  OK  ] Loaded and verified {}+{} libraries\n", class_files.len(), natives_files.len()),
            Event::LoggerAbsent {  } => 
                print!(  "[  OK  ] No logger"),
            Event::LoggerLoading { id } => 
                print!(  "[  ..  ] Loading logger {id}"),
            Event::LoggerLoaded { id } => 
                print!("\r[  OK  ] Loaded logger {id} \n"),
            Event::AssetsAbsent {  } => 
                print!(  "[  OK  ] No assets\n"),
            Event::AssetsLoading { id } => 
                print!(  "[  ..  ] Loading assets {id}"),
            Event::AssetsLoaded { id, index } => 
                print!("\r[  ..  ] Loaded {} assets {id}", index.objects.len()),
            Event::AssetsVerified { id, index } => 
                print!("\r[  OK  ] Loaded and verified {} assets {id}\n", index.objects.len()),
            _ => todo!(),
        }

        std::io::stdout().flush().unwrap();

    }

}
