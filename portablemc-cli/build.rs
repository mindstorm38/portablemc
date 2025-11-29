//! This build script is only used to provide compile-time rustc version and git revision
//! for the CLI to show with long version.

use std::process::Command;
use std::path::Path;
use std::{env, fs};

fn main() {

    println!("cargo::rerun-if-changed=build.rs");

    // Fetch git revision...
    let git_dir = Path::new("..").join(".git");
    let head_file = git_dir.join("HEAD");
    println!("cargo::rerun-if-changed={}", head_file.display());
    if let Ok(head_ref) = fs::read_to_string(&head_file) 
    && let Some(head_ref) = head_ref.trim_end().strip_prefix("ref: ") {
        let ref_file = git_dir.join(&head_ref);
        if ref_file.is_file() {
            println!("cargo::rerun-if-changed={}", ref_file.display());
        }
    }

    if let Ok(out) = Command::new("git").args(["show", "--no-patch", "--format=%h (%cs)", "HEAD"]).output()
    && let Ok(rev) = String::from_utf8(out.stdout) {
        println!("cargo::rustc-env=PMC_GIT_COMMIT={}", rev.trim_end());
    } else {
        println!("cargo::rustc-env=PMC_GIT_COMMIT=?");
    }

    // Fetch rustc version...
    if let Ok(meta) = rustc_version::version_meta() {
        println!("cargo::rustc-env=PMC_RUSTC_VERSION={} ({})", meta.semver, meta.host);
    } else {
        println!("cargo::rustc-env=PMC_RUSTC_VERSION=?");
    }
    
    // Forward the optional version more...
    println!("cargo::rerun-if-env-changed=PMC_VERSION_MORE");
    println!("cargo::rustc-env=PMC_VERSION_MORE={}", env::var("PMC_VERSION_MORE").as_deref().unwrap_or(""));

}
