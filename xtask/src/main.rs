use std::ffi::OsStr;
use std::process::{Command, ExitCode};
use std::fmt::Write as _;
use std::path::Path;
use std::fs::File;
use std::{env, fs};

use flate2::Compression;
use flate2::write::GzEncoder;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};
use zip_extensions::default_entry_handler::DefaultEntryHandler;
use zip_extensions::zip_writer_extensions::ZipWriterExtensions;


fn main() -> ExitCode {
    
    let args = env::args().collect::<Vec<_>>();
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();

    match args[1..] {
        ["dist", target] => dist(Some(target)),
        ["dist"] => dist(None),
        _ => {
            eprintln!("usage: {} dist [target]", args[0]);
            ExitCode::FAILURE
        }
    }

}

fn dist(target: Option<&str>) -> ExitCode {

    let cargo_env = env::vars_os()
        .map(|(name, _val)| name)
        .filter(|name| 
            name.as_encoded_bytes().starts_with(b"CARGO") || 
            name.as_encoded_bytes() == b"OUT_DIR")
        .collect::<Vec<_>>();

    let xtask_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_dir = xtask_dir.parent().unwrap();
    env::set_current_dir(&root_dir).unwrap();
    println!("Root dir: {}", root_dir.display());

    let dist_dir = root_dir.join("dist");
    fs::create_dir_all(&dist_dir).unwrap();
    println!("Dist dir: {}", dist_dir.display());

    let target_dir = {
        let mut buf = root_dir.join("target");
        if let Some(target) = target {
            buf.push(target);
        }
        buf.push("release");
        buf
    };
    println!("Target dir: {}", target_dir.display());

    let version = env!("CARGO_PKG_VERSION");
    let mut version_long = version.to_string();

    println!("Finding git commit...");
    if let Ok(out) = Command::new("git").args(["show", "--no-patch", "--format=%h (%cs)", "HEAD"]).output()
    && let Ok(rev) = String::from_utf8(out.stdout) {
        let rev = rev.trim_end();
        println!("   Found: {rev}");
        write!(version_long, "\ncommit: {rev}").unwrap();
    } else {
        println!("   Not found, skipped");
    }

    println!("Finding rustc version...");
    let rustc_vv_output = Command::new("rustc")
        .arg("-vV")
        .output()
        .unwrap();
    let rustc_vv = String::from_utf8(rustc_vv_output.stdout).unwrap();
    let mut it = rustc_vv.split_whitespace();
    let mut rustc_release = None;
    let mut rustc_host = None;
    while let Some(part) = it.next() {
        match part {
            "host:" => rustc_host = Some(it.next().unwrap()),
            "release:" => rustc_release = Some(it.next().unwrap()),
            _ => (),
        }
    }

    if let (Some(rustc_release), Some(rustc_host)) = (rustc_release, rustc_host) {
        println!("   Found: {rustc_release} ({rustc_host})");
        write!(version_long, "\nrustc: {rustc_release} ({rustc_host})").unwrap();
    } else {
        println!("   Not found, skipped");
    }

    println!("Finding target spec...");
    let mut cmd = Command::new("rustc");
    cmd.args(["+nightly", "-Z", "unstable-options", "--print", "target-spec-json"]);
    if let Some(target) = target {
        cmd.args(["--target", target]);
    }
    let target_spec_output = cmd.output().unwrap();
    let target_spec = serde_json::from_slice::<serde_json::Value>(&target_spec_output.stdout).unwrap();
    let target_llvm = target_spec
        .get("llvm-target").unwrap()
        .as_str().unwrap();
    let target_platform = target_spec
        .get("metadata").unwrap()
        .get("description").unwrap()
        .as_str().unwrap();
    let target_os = target_spec
        .get("os").unwrap()
        .as_str().unwrap();
    let target_arch = target_spec
        .get("arch").unwrap()
        .as_str().unwrap();

    println!("   Found: {target_llvm} ({target_platform})");
    write!(version_long, "\ntarget: {target_llvm}").unwrap();
    write!(version_long, "\nplatform: {target_platform}").unwrap();
    
    if let Ok(more) = env::var("PMC_VERSION_LONG") {
        version_long.push('\n');
        version_long.push_str(&more);
    }

    println!("Building release artifacts...");
    let mut cmd = Command::new("cargo");
    // We remove all the cargo env variables that are forwarded by "cargo run", so
    // that no build script could mention a change in environment.
    for cargo_var in cargo_env {
        cmd.env_remove(&cargo_var);
    }
    cmd.env("PMC_VERSION_LONG", version_long);
    cmd.args(["build", "--release"]);
    if let Some(target) = target {
        cmd.args(["--target", target]);
    }

    if !cmd.spawn().expect("Cannot spawn").wait().expect("Cannot wait").success() {
        return ExitCode::FAILURE;
    }

    println!("Building archive directory...");
    let archive_name = format!("portablemc-{version}-{target_os}-{target_arch}");
    let archive_dir = dist_dir.join(&archive_name);
    if archive_dir.exists() {
        fs::remove_dir_all(&archive_dir).unwrap();
    }
    fs::create_dir_all(&archive_dir).unwrap();
    
    if cfg!(windows) {
        fs::copy(target_dir.join("portablemc.exe"), archive_dir.join("portablemc.exe")).unwrap();
        fs::copy(target_dir.join("portablemc.pdb"), archive_dir.join("portablemc.exe.pdb")).unwrap();
    } else {
        fs::copy(target_dir.join("portablemc"), archive_dir.join("portablemc")).unwrap();
    }
    
    fs::copy(root_dir.join("LICENSE"), archive_dir.join("LICENSE")).unwrap();
    
    let mut readme = fs::read_to_string(xtask_dir.join("data/README")).unwrap();
    writeln!(readme, "Version: {version}").unwrap();
    writeln!(readme, "Target: {target_llvm}").unwrap();
    writeln!(readme, "Platform: {target_platform}").unwrap();
    fs::write(archive_dir.join("README"), &readme).unwrap();

    if has_non_empty_var("PMC_NO_ARCHIVE") {
        println!("Not building archive because PMC_NO_ARCHIVE is not empty.");
    } else {
        println!("Building archive...");
        if cfg!(windows) {
            
            let archive_file = dist_dir.join(format!("{archive_name}.zip"));
            let archive_write = File::create(&archive_file).unwrap();
            let mut archive_write = ZipWriter::new(archive_write);
            archive_write.set_comment(readme);

            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated);

            archive_write.create_from_directory_with_options(&archive_dir, |_| options, &DefaultEntryHandler).unwrap();

        } else {

            let archive_file = dist_dir.join(format!("{archive_name}.tar.gz"));
            let archive_write = File::create(&archive_file).unwrap();
            let archive_write = GzEncoder::new(archive_write, Compression::default());
            let mut archive_write = tar::Builder::new(archive_write);
            archive_write.append_dir_all(archive_name, &archive_dir).unwrap();

        }
    }

    ExitCode::SUCCESS

}

fn has_non_empty_var<S: AsRef<OsStr>>(name: S) -> bool {
    env::var_os(name).is_some_and(|val| val.as_encoded_bytes() != b"")
}
