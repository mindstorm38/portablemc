//! PortableMC CLI.

use std::time::Instant;
use std::io::Write;

use portablemc::standard::*;


fn main() {
    
    let mut installer = Installer::with_dirs(r"C:\Users\Theo\AppData\Roaming\.minecraft_test".into(), r"C:\Users\Theo\AppData\Roaming\.minecraft_test".into());
    installer.strict_libraries_checking = false;
    installer.strict_assets_checking = false;

    installer.install(&mut DebugHandler::default(), "1.21").unwrap();

}

#[derive(Debug, Default)]
struct DebugHandler {
    download_start: Option<Instant>,
}

impl Handler for DebugHandler {
    
    fn handle(&mut self, _installer: &Installer, event: Event) -> Result<()> {

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
            Event::DownloadProgress { count, total_count, size, total_size  } => {

                if self.download_start.is_none() {
                    self.download_start = Some(Instant::now());
                }

                if size == 0 {
                    return Ok(());
                }

                let elapsed = self.download_start.unwrap().elapsed();

                let speed = size as f32 / elapsed.as_secs_f32() / 1_000_000.0;
                let progress = size as f32 / total_size as f32 * 100.0;

                print!("\r[  ..  ] Downloading... {speed:.0} MB/s {progress:5.1}% ({count}/{total_count})");

                if count == total_count {
                    self.download_start = None;
                    print!("\r[  OK  ]\n");
                }
            }
            _ => todo!(),
        }

        std::io::stdout().flush().unwrap();

        Ok(())

    }

}
