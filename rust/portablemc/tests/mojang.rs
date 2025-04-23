//! Integration test to ensure that all versions can be installed, without their 
//! resources.

use std::fs;
use std::path::PathBuf;

use portablemc::download;
use portablemc::mojang::{self, Manifest};
use portablemc::standard::{self, JvmPolicy, VersionChannel};


/// This test tries to parse all versions (except snapshots).
#[test]
#[ignore = "long, use internet"]
fn all() {

    fs::create_dir_all(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let tmp_main_dir = tempfile::Builder::new()
        .prefix("")
        .suffix(".all")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap()
        .into_path();

    let mut inst = mojang::Installer::new(mojang::Version::Release);
    inst.standard_mut().set_main_dir(tmp_main_dir.clone());
    inst.standard_mut().set_jvm_policy(JvmPolicy::Static(PathBuf::new()));

    let manifest = Manifest::request(()).unwrap();
    for version in manifest.iter() {

        if let VersionChannel::Snapshot = version.channel() {
            continue;
        }

        inst.set_version(version.name());
        match inst.install(NoResourceHandler) {
            Ok(_game) => {}
            Err(mojang::Error::Standard(standard::Error::DownloadResourcesCancelled {  })) => {}
            Err(e) => Err(e).unwrap(),
        }

    }

    // Only remove it here so when the test did not panic.
    fs::remove_dir_all(&tmp_main_dir).unwrap();

}


struct NoResourceHandler;
impl download::Handler for NoResourceHandler { }
impl standard::Handler for NoResourceHandler {
    
    fn download_resources(&mut self) -> bool {
        false
    }

}
impl mojang::Handler for NoResourceHandler { }
