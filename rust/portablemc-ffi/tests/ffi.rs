//! This test tries to compile with the local C compiler the test file, and then execute
//! it to check if everything works.

use std::process::Command;
use std::path::PathBuf;
use std::fs;


#[test]
fn compile_link_exec() {

    static TEST_FILES: [&str; 1] = ["main.c"];

    fs::create_dir_all(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let tmp_dir = tempfile::Builder::new()
        .prefix("")
        .suffix(".ffi")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap()
        .into_path();

    println!("tmp_dir: {tmp_dir:?}");
    
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let include_dir = manifest_dir.join("include");
    let ffi_dir = manifest_dir.join("tests").join("ffi");

    let mut compilation_database = Vec::new();

    for test_file in TEST_FILES {

        let src_file = ffi_dir.join(test_file);
        println!("src_file: {src_file:?}");

        let mut compile_cmd;
        let mut out_file;
        
        if let Some(test_name) = test_file.strip_suffix(".c") {
            
            out_file = tmp_dir.join(test_name);

            if cfg!(target_family = "windows") && cfg!(target_env = "msvc") {

                compile_cmd = Command::new("cl.exe");
                compile_cmd.arg("/nologo");
                compile_cmd.arg(format!("/I{}", include_dir.display()));
                compile_cmd.arg("/W4");
                compile_cmd.arg(format!("/Fo:{}", out_file.display()));
                compile_cmd.arg(format!("/Fe:{}", out_file.display()));
                compile_cmd.arg(src_file);

                out_file.as_mut_os_string().push(".exe");

            } else {
                panic!("No compiler found for your target!");
            }

            compilation_database.push(CommandObject {
                directory: format!("{}", ffi_dir.display()),
                arguments: compile_cmd.get_args().map(|arg| arg.to_string_lossy().to_string()).collect(),
                file: test_name.to_string(),
            });

        } else {
            panic!("This type of test file is not supported!");
        }

        println!("cmd: {compile_cmd:?}");
        println!("out_file: {out_file:?}");

        let status = compile_cmd.spawn()
            .unwrap()
            .wait()
            .unwrap();

        println!("status: {status:?}");

    }

    panic!();
    
    // Only remove it here so when the test did not panic.
    fs::remove_dir_all(&tmp_dir).unwrap();

}


#[derive(Debug, serde::Serialize)]
struct CommandObject {
    directory: String,
    arguments: Vec<String>,
    file: String,
}
