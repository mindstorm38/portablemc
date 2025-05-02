//! Standard installer.

use std::path::PathBuf;
use std::ffi::c_char;
use std::pin::Pin;
use std::ptr;

use portablemc::standard::{Installer, JvmPolicy, Game, Error};

use crate::alloc::{extern_box, extern_box_cstr_from_fmt, extern_box_cstr_from_str};
use crate::{str_lossy_from_cstr_ptr, cstr_bytes_from_string};
use crate::err::{self, extern_err_with, Err, ExposedError};


// =======
// Module errors
// =======

impl ExposedError for Error {

    fn code(&self) -> u8 {
        match self {
            Error::HierarchyLoop { .. } => err::code::STANDARD_HIERARCHY_LOOP,
            Error::VersionNotFound { .. } => err::code::STANDARD_VERSION_NOT_FOUND,
            Error::AssetsNotFound { .. } => err::code::STANDARD_ASSETS_NOT_FOUND,
            Error::ClientNotFound { .. } => err::code::STANDARD_CLIENT_NOT_FOUND,
            Error::LibraryNotFound { .. } => err::code::STANDARD_LIBRARY_NOT_FOUND,
            Error::JvmNotFound { .. } => err::code::STANDARD_JVM_NOT_FOUND,
            Error::MainClassNotFound { .. } => err::code::STANDARD_MAIN_CLASS_NOT_FOUND,
            Error::DownloadResourcesCancelled { .. } => err::code::STANDARD_DOWNLOAD_RESOURCES_CANCELLED,
            Error::Download { .. } => err::code::STANDARD_DOWNLOAD,
            Error::Internal { .. } => err::code::INTERNAL,
            _ => todo!(),
        }
    }
    
    fn extern_data(&self) -> *mut () {
        match *self {
            Error::HierarchyLoop { ref version } => extern_box_cstr_from_str(version).cast(),
            Error::VersionNotFound { ref version } => extern_box_cstr_from_str(version).cast(),
            Error::AssetsNotFound { ref id } => extern_box_cstr_from_str(id).cast(),
            Error::LibraryNotFound { ref gav } => extern_box_cstr_from_str(gav.as_str()).cast(),
            Error::JvmNotFound { major_version } => extern_box(major_version).cast(),
            // TODO: Error::Download { .. }
            Error::Internal { ref origin, .. } => extern_box_cstr_from_str(origin).cast(),
            _ => ptr::null_mut(),
        }
    }

}

// =======
// Binding for Installer
// =======

/// The external type defined in the header.
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum pmc_jvm_policy_tag {
    Static,
    System,
    Mojang,
    SystemThenMojang,
    MojangThenSystem,
}

/// The external type defined in the header.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct pmc_jvm_policy {
    tag: pmc_jvm_policy_tag,
    static_path: *const c_char,
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_new(version: *const c_char) -> *mut Installer {
    extern_box(Installer::new(unsafe { str_lossy_from_cstr_ptr(version) }))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_version(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_str(inst.version())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_version(inst: &mut Installer, version: *const c_char) {
    inst.set_version(unsafe { str_lossy_from_cstr_ptr(version) });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_versions_dir(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", inst.versions_dir().display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_versions_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_versions_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_libraries_dir(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", inst.libraries_dir().display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_libraries_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_libraries_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_assets_dir(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", inst.assets_dir().display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_assets_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_assets_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_jvm_dir(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", inst.jvm_dir().display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_jvm_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_jvm_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_bin_dir(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", inst.bin_dir().display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_bin_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_bin_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_mc_dir(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", inst.mc_dir().display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_mc_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_mc_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_main_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_main_dir(unsafe { str_lossy_from_cstr_ptr(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_strict_assets_check(inst: &Installer) -> bool {
    inst.strict_assets_check()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_strict_assets_check(inst: &mut Installer, strict: bool) {
    inst.set_strict_assets_check(strict);
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_strict_libraries_check(inst: &Installer) -> bool {
    inst.strict_libraries_check()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_strict_libraries_check(inst: &mut Installer, strict: bool) {
    inst.set_strict_libraries_check(strict);
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_strict_jvm_check(inst: &Installer) -> bool {
    inst.strict_jvm_check()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_strict_jvm_check(inst: &mut Installer, strict: bool) {
    inst.set_strict_jvm_check(strict);
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_jvm_policy(inst: &Installer) -> *mut pmc_jvm_policy {
   
    /// This wrapper type is used to return the JVM policy allocated and, if static,
    /// pointing to the inner buffer. The inner type that we return the pointer for
    /// must be placed first.
    #[repr(C)]
    struct ExternJvmPolicy {
        inner: pmc_jvm_policy,
        owned_static_path: Option<Pin<Box<[u8]>>>,
    }

    let tag = match inst.jvm_policy() {
        JvmPolicy::Static(_) => pmc_jvm_policy_tag::Static,
        JvmPolicy::System => pmc_jvm_policy_tag::System,
        JvmPolicy::Mojang => pmc_jvm_policy_tag::Mojang,
        JvmPolicy::SystemThenMojang => pmc_jvm_policy_tag::SystemThenMojang,
        JvmPolicy::MojangThenSystem => pmc_jvm_policy_tag::MojangThenSystem,
    };

    let owned_static_path = if let JvmPolicy::Static(static_path) = inst.jvm_policy() {
        Some(Pin::new(cstr_bytes_from_string(format!("{}", static_path.display()))))
    } else {
        None
    };

    extern_box(ExternJvmPolicy {
        inner: pmc_jvm_policy { 
            tag, 
            static_path: owned_static_path
                .as_deref()
                .map(|slice| slice.as_ptr().cast::<c_char>())
                .unwrap_or(ptr::null()),
        },
        owned_static_path,
    }).cast()

}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_jvm_policy(inst: &mut Installer, policy: &pmc_jvm_policy) {
    inst.set_jvm_policy(match policy.tag {
        pmc_jvm_policy_tag::Static =>
            JvmPolicy::Static(PathBuf::from(unsafe { str_lossy_from_cstr_ptr(policy.static_path).to_string() })),
        pmc_jvm_policy_tag::System => JvmPolicy::System,
        pmc_jvm_policy_tag::Mojang => JvmPolicy::Mojang,
        pmc_jvm_policy_tag::SystemThenMojang => JvmPolicy::SystemThenMojang,
        pmc_jvm_policy_tag::MojangThenSystem => JvmPolicy::MojangThenSystem,
    });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_launcher_name(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_str(inst.launcher_name())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_launcher_name(inst: &mut Installer, name: *const c_char) {
    inst.set_launcher_name(unsafe { str_lossy_from_cstr_ptr(name) });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_launcher_version(inst: &Installer) -> *mut c_char {
    extern_box_cstr_from_str(inst.launcher_version())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_launcher_version(inst: &mut Installer, version: *const c_char) {
    inst.set_launcher_version(unsafe { str_lossy_from_cstr_ptr(version) });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_install(inst: &mut Installer, err: *mut *mut Err) -> *mut Game {
    extern_err_with(err, || {
        inst.install(()).map(extern_box)
    }).unwrap_or(ptr::null_mut())
}

// TODO:

// =======
// Binding for Game
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_game_jvm_file(game: &Game) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", game.jvm_file.display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_game_mc_dir(game: &Game) -> *mut c_char {
    extern_box_cstr_from_fmt(format_args!("{}", game.mc_dir.display()))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_game_main_class(game: &Game) -> *mut c_char {
    extern_box_cstr_from_str(&game.main_class)
}

// TODO: