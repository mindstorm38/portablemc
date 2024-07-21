//! PortableMC CLI.

use std::io::Write;

use portablemc::standard::*;


fn main() {
    
    let mut installer = Installer::new();
    installer.strict_libraries_checking = false;
    installer.strict_assets_checking = false;

    installer.install(&mut DebugHandler, "fabric-1.21-0.15.11").unwrap();

}


struct DebugHandler;
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
            Event::DownloadProgress { index, count, size, total_size } => 
                print!("download progress..."),
            _ => todo!(),
        }

        std::io::stdout().flush().unwrap();

        Ok(())

    }

}
