use std::process::{Command, ExitCode};
use std::fmt::Write as _;
use std::path::Path;
use std::io::Write;
use std::fs::File;
use std::{env, fs};

use flate2::Compression;
use flate2::write::GzEncoder;


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

    let root_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
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

    // Reducing codegen units count have reduced by 20% the size of the final binary.
    let mut cmd = Command::new("cargo");
    cmd.env("PMC_VERSION_LONG", version_long);
    cmd.env("RUSTFLAGS", "-Copt-level=3 -Cstrip=debuginfo -Ccodegen-units=1");
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
    
    let mut archive_readme_write = File::create(archive_dir.join("readme.txt")).unwrap();
    archive_readme_write.write_all(include_bytes!("../data/readme.txt")).unwrap();
    writeln!(archive_readme_write, "Version: {version}").unwrap();
    writeln!(archive_readme_write, "Target: {target_llvm}").unwrap();
    writeln!(archive_readme_write, "Platform: {target_platform}").unwrap();
    drop(archive_readme_write);
    
    println!("Building archive...");
    let archive_file = dist_dir.join(format!("{archive_name}.tar.gz"));
    let archive_write = File::create(&archive_file).unwrap();
    let archive_write = GzEncoder::new(archive_write, Compression::default());
    let mut archive_write = tar::Builder::new(archive_write);
    archive_write.append_dir_all(archive_name, &archive_dir).unwrap();

    ExitCode::SUCCESS

}
