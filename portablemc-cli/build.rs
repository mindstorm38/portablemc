//! This build script is only used to provide compile-time rustc version and git revision
//! for the CLI to show with long version.

use std::process::Command;
use std::path::Path;
use std::fs;

fn main() {

    let git_dir = Path::new("..").join(".git");
    let head_file = git_dir.join("HEAD");

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={}", head_file.display());

    if let Ok(head_ref) = fs::read_to_string(&head_file) 
    && let Some(head_ref) = head_ref.trim_end().strip_prefix("ref: ") {
        let ref_file = git_dir.join(&head_ref);
        if ref_file.is_file() {
            println!("cargo::rerun-if-changed={}", ref_file.display());
        }
    }

    if let Ok(meta) = rustc_version::version_meta() {
        println!("cargo::rustc-env=PMC_RUSTC_VERSION={} {}", meta.semver, meta.host);
    } else {
        println!("cargo::rustc-env=PMC_RUSTC_VERSION=?");
    }

    if let Some(rev) = get_git_revision() {
        println!("cargo::rustc-env=PMC_GIT_REVISION={rev}");
    } else {
        println!("cargo::rustc-env=PMC_GIT_REVISION=?");
    }

}

fn get_git_revision() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output().ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
}
