//! Automated installation tests with verification of the events ordering.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use assert_fs::TempDir;
use regex::Regex;

use portablemc::standard::{self, JvmPolicy, LoadedLibrary, LoadedVersion};
use portablemc::download;


/// This macro is used to easily define various version installation tests.
macro_rules! def_install_tests {
    ( $fn_name:ident = $name:expr, $( $rem:tt )* ) => {
        #[test]
        fn $fn_name () {
            install_version( $name );
        }
        def_install_tests!( $($rem)* );
    };
    ( $fn_name:ident, $( $rem:tt )* ) => {
        #[test]
        fn $fn_name () {
            install_version( stringify!($fn_name) );
        }
        def_install_tests!( $($rem)* );
    };
    () => {};
}

def_install_tests![
    recurse, 
    client_not_found,
    libraries,
];

/// Create a path to the data directory.
fn data_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("tests");
    dir.push("data");
    dir
}

/// Replace macro of the form `name!(<content>)` by giving the content to the closure
/// and replacing the whole macro by the returned content.
fn replace_macro<F>(s: &mut String, name: &str, mut func: F)
where
    F: FnMut(&str) -> String,
{

    let open_pat = format!("${name}(");
    let mut cursor = 0;

    while let Some(open_idx) = s[cursor..].find(&open_pat) {

        let open_idx = cursor + open_idx;
        let Some(close_idx) = s[open_idx + open_pat.len()..].find(')') else { break };
        let close_idx = open_idx + open_pat.len() + close_idx + 1;
        cursor = close_idx;

        let value = func(&s[open_idx + open_pat.len()..close_idx - 1]);
        s.replace_range(open_idx..close_idx, &value);

        let repl_len = close_idx - open_idx;
        let repl_diff = value.len() as isize - repl_len as isize;
        cursor = cursor.checked_add_signed(repl_diff).unwrap();

    }

}

/// Compare expected logs and actual logs, also checking for macros in expected string.
fn assert_logs_eq(
    expected_logs: Vec<String>, 
    actual_logs: Vec<String>,
    tmp_main_dir: &Path,
) {

    let mut expected_logs_it = expected_logs.into_iter().peekable();
    let mut actual_logs_it = actual_logs.into_iter().peekable();

    // Check line by line.
    let mut valid = true;
    let mut regex_cache = None::<Regex>;

    loop {
        
        let Some(expected_log) = expected_logs_it.peek_mut() else {
            while let Some(actual_log) = actual_logs_it.next() {
                eprintln!("== Expected less line");
                eprintln!("{actual_log}");
                valid = false;
            }
            break;
        };

        // Replace any unprocessed path macro.
        replace_macro(&mut *expected_log, "os", |_| env::consts::OS.to_string());
        replace_macro(&mut *expected_log, "path", |path| {
            let mut buf = tmp_main_dir.to_path_buf();
            buf.extend(path.split('/'));
            format!("{buf:?}")
        });

        let Some(actual_log) = actual_logs_it.peek() else {
            eprintln!("== Expected more lines");
            valid = false;
            break;
        };
        
        let expected_log = &*expected_log;
        let actual_log = &*actual_log;

        eprintln!("==");
        eprintln!("    {expected_log}");
        eprintln!("    {actual_log}");

        if let Some(regex_str) = expected_log.strip_prefix("$ignore_many ") {
            
            let regex = match &regex_cache {
                Some(regex) if regex.as_str() == regex_str => regex,
                _ => {
                    let regex = Regex::new(regex_str).expect("failed to compile regex for $ignore_many");
                    regex_cache.insert(regex)
                }
            };

            if regex.is_match(&actual_log) {
                actual_logs_it.next();
            } else {
                expected_logs_it.next();
                eprintln!("== Retrying...");
            }

        } else if let Some(regex_str) = expected_log.strip_prefix("$ignore_once ") {
            
            let regex = Regex::new(regex_str).expect("failed to compile regex for $ignore_once");

            if regex.is_match(&actual_log) {
                expected_logs_it.next();
                actual_logs_it.next();
            } else {
                valid = false;
                break;
            }

        } else if expected_log != actual_log {
            valid = false;
            break;
        } else {
            expected_logs_it.next();
            actual_logs_it.next();
        }

    }

    if !valid {
        panic!("Incoherent, read above!");
    }

}

/// Common function to run a test for the given function.
fn install_version(version: &str) {
    
    let data_dir = data_dir();
    let versions_dir = data_dir.join("versions");
    let metadata_file = versions_dir.join(format!("{version}.json"));

    let expected_log = {
        match fs::read_to_string(versions_dir.join(format!("{version}.{}.log", env::consts::OS))) {
            Ok(log) => log,
            Err(e) if e.kind() == io::ErrorKind::NotFound =>
                fs::read_to_string(versions_dir.join(format!("{version}.log"))).unwrap(),
            Err(e) => Err(e).unwrap(),
        }
    };
    let expected_logs = expected_log.lines().map(str::to_string).collect::<Vec<_>>();
    drop(expected_log);
    
    let tmp_main_dir = TempDir::new().unwrap();
    let tmp_versions_dir = tmp_main_dir.join("versions");
    let tmp_version_dir = tmp_versions_dir.join(version);
    let tmp_metadata_file = tmp_version_dir.join(format!("{version}.json"));

    fs::create_dir_all(&tmp_version_dir).unwrap();
    fs::copy(&metadata_file, &tmp_metadata_file).unwrap();

    // Now run the installer and store its actual logs...
    let mut actual_logs = Vec::new();
    let mut installer = standard::Installer::new(version);
    installer.set_main_dir(tmp_main_dir.to_path_buf());
    installer.set_jvm_policy(JvmPolicy::Static(PathBuf::new()));
    match installer.install(TestHandler { logs: &mut actual_logs }) {
        Ok(_game) => {}
        Err(standard::Error::DownloadResourcesCancelled {  }) => {}
        Err(e) => {
            actual_logs.push(format!("standard::{e:?}"));
        }
    }

    assert_logs_eq(expected_logs, actual_logs, &tmp_main_dir);

}

/// The handler used to debug event when testing version installation. This handler stores
/// every method invocation as a debug string that can later be matched against an 
/// expected trace.
#[derive(Debug)]
struct TestHandler<'a> {
    logs: &'a mut Vec<String>,
}

macro_rules! impl_test_handler {
    (
        $prefix:literal :
        $( fn $func:ident ( $( $arg:ident : $arg_ty:ty ),* ) $( -> $ret_ty:ty = $ret_value:expr )?; )*
    ) => {
        $(
            fn $func ( &mut self $(, $arg : $arg_ty )* ) $( -> $ret_ty )? {
                self.logs.push(format!(
                    concat!($prefix, "::", stringify!($func), "(", $( "{", stringify!($arg), ":?}, ", )* ")")
                    $( , $arg = $arg )*
                ));
                $( $ret_value )?
            }
        )*
    };
}

impl download::Handler for TestHandler<'_> {
    impl_test_handler! {
        "download":
        fn progress(count: u32, total_count: u32, size: u32, total_size: u32);
    }
}

impl standard::Handler for TestHandler<'_> {

    impl_test_handler! {
        "standard":
        fn filter_features(features: &mut HashSet<String>);
        fn loaded_features(features: &HashSet<String>);
        fn load_hierarchy(root_version: &str);
        fn loaded_hierarchy(hierarchy: &[LoadedVersion]);
        fn load_version(version: &str, file: &Path);
        fn need_version(version: &str, file: &Path) -> bool = false;
        fn loaded_version(version: &str, file: &Path);
        fn load_client();
        fn loaded_client(file: &Path);
        fn load_libraries();
        fn filter_libraries(libraries: &mut Vec<LoadedLibrary>);
        fn loaded_libraries(libraries: &[LoadedLibrary]);
        fn filter_libraries_files(class_files: &mut Vec<PathBuf>, natives_files: &mut Vec<PathBuf>);
        fn loaded_libraries_files(class_files: &[PathBuf], natives_files: &[PathBuf]);
        fn no_logger();
        fn load_logger(id: &str);
        fn loaded_logger(id: &str);
        fn no_assets();
        fn load_assets(id: &str);
        fn loaded_assets(id: &str, count: usize);
        fn verified_assets(id: &str, count: usize);
        fn load_jvm(major_version: u32);
        fn found_jvm_system_version(file: &Path, version: &str, compatible: bool);
        fn warn_jvm_unsupported_dynamic_crt();
        fn warn_jvm_unsupported_platform();
        fn warn_jvm_missing_distribution();
        fn loaded_jvm(file: &Path, version: Option<&str>, compatible: bool);
    }

    fn download_resources(&mut self) -> bool {
        // Just skip download resources.
        false
    }

    fn downloaded_resources(&mut self) {
        // Ignore.
    }

    fn extracted_binaries(&mut self, _dir: &Path) {
        // Ignore.
    }

}
