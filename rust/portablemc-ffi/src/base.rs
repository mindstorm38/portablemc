//! Standard installer.

use std::path::PathBuf;
use std::ffi::c_char;
use std::pin::Pin;
use std::ptr::{self, NonNull};

use portablemc::base::{Installer, JvmPolicy, Game, Error, Handler, Event};

use crate::alloc::{extern_box, extern_cstr_from_fmt, extern_cstr_from_str};
use crate::err::{extern_err_catch, extern_err, IntoExternErr};
use crate::{cstr, raw};


// =======
// Module errors
// =======

impl IntoExternErr for Error {

    fn into(self) -> NonNull<raw::pmc_err> {
        use raw::pmc_err_tag::*;
        match self {
            Error::HierarchyLoop { version } => extern_err!(
                PMC_ERR_BASE_HIERARCHY_LOOP, 
                format!("Version hierarchy loop caused by: {version}"),
                raw::pmc_err_base_hierarchy_loop {
                    version: version => cstr
                }),
            Error::VersionNotFound { version } => extern_err!(
                PMC_ERR_BASE_VERSION_NOT_FOUND, 
                format!("Version not found: {version}"),
                raw::pmc_err_base_version_not_found {
                    version: version => cstr
                }),
            Error::AssetsNotFound { id } => extern_err!(
                PMC_ERR_BASE_ASSETS_NOT_FOUND, 
                format!("Assets not found: {id}"),
                raw::pmc_err_base_assets_not_found {
                    id: id => cstr
                }),
            Error::ClientNotFound {  } => extern_err!(
                PMC_ERR_BASE_CLIENT_NOT_FOUND,
                c"Client not found"),
            Error::LibraryNotFound { name } => extern_err!(
                PMC_ERR_BASE_LIBRARY_NOT_FOUND,
                format!("Library not found: {name}"),
                raw::pmc_err_base_library_not_found {
                    name: name.to_string() => cstr
                }),
            Error::JvmNotFound { major_version } => extern_err!(
                PMC_ERR_BASE_JVM_NOT_FOUND,
                format!("JVM not found for major version: {major_version}"),
                raw::pmc_err_base_jvm_not_found {
                    major_version: major_version
                }),
            Error::MainClassNotFound {  } => extern_err!(
                PMC_ERR_BASE_MAIN_CLASS_NOT_FOUND,
                c"Main class not found"),
            Error::DownloadResourcesCancelled {  } => extern_err!(
                PMC_ERR_BASE_DOWNLOAD_RESOURCES_CANCELLED,
                c"Download resources cancelled"),
            Error::Download { batch: _ } => extern_err!(
                PMC_ERR_BASE_DOWNLOAD,
                c"Download error"),
            Error::Internal { error, origin } => extern_err!(
                PMC_ERR_INTERNAL, 
                error.to_string(),
                raw::pmc_err_data_internal {
                    origin: origin => cstr
                }),
            _ => todo!(),
        }
    }

}

// =======
// Binding for Installer
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_new(version: *const c_char) -> NonNull<Installer> {
    extern_box(Installer::new(unsafe { cstr::to_str_lossy(version) }))
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_version(inst: &Installer) -> NonNull<c_char> {
    extern_cstr_from_str(inst.version())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_version(inst: &mut Installer, version: *const c_char) {
    inst.set_version(unsafe { cstr::to_str_lossy(version) });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_versions_dir(inst: &Installer) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", inst.versions_dir().display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_versions_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_versions_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_libraries_dir(inst: &Installer) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", inst.libraries_dir().display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_libraries_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_libraries_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_assets_dir(inst: &Installer) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", inst.assets_dir().display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_assets_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_assets_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_jvm_dir(inst: &Installer) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", inst.jvm_dir().display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_jvm_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_jvm_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_bin_dir(inst: &Installer) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", inst.bin_dir().display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_bin_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_bin_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_mc_dir(inst: &Installer) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", inst.mc_dir().display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_mc_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_mc_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_main_dir(inst: &mut Installer, dir: *const c_char) {
    inst.set_main_dir(unsafe { cstr::to_str_lossy(dir).to_string() });
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
pub unsafe extern "C" fn pmc_standard_jvm_policy(inst: &Installer) -> NonNull<raw::pmc_jvm_policy> {
   
    /// This wrapper type is used to return the JVM policy allocated and, if static,
    /// pointing to the inner buffer. The inner type that we return the pointer for
    /// must be placed first.
    #[repr(C)]
    struct ExternJvmPolicy {
        inner: raw::pmc_jvm_policy,
        owned_static_path: Option<Pin<Box<[c_char]>>>,
    }

    use raw::pmc_jvm_policy_tag::*;

    let tag = match inst.jvm_policy() {
        JvmPolicy::Static(_) => PMC_JVM_POLICY_STATIC,
        JvmPolicy::System => PMC_JVM_POLICY_SYSTEM,
        JvmPolicy::Mojang => PMC_JVM_POLICY_MOJANG,
        JvmPolicy::SystemThenMojang => PMC_JVM_POLICY_SYSTEM_THEN_MOJANG,
        JvmPolicy::MojangThenSystem => PMC_JVM_POLICY_MOJANG_THEN_SYSTEM,
    };

    let owned_static_path = if let JvmPolicy::Static(static_path) = inst.jvm_policy() {
        Some(Pin::new(cstr::from_string(format!("{}", static_path.display()))))
    } else {
        None
    };

    extern_box(ExternJvmPolicy {
        inner: raw::pmc_jvm_policy { 
            tag, 
            static_path: owned_static_path
                .as_deref()
                .map(|slice| slice.as_ptr())
                .unwrap_or(ptr::null()),
        },
        owned_static_path,
    }).cast::<raw::pmc_jvm_policy>()

}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_jvm_policy(inst: &mut Installer, policy: &raw::pmc_jvm_policy) {
    
    use raw::pmc_jvm_policy_tag::*;

    inst.set_jvm_policy(match policy.tag {
        PMC_JVM_POLICY_STATIC =>
            JvmPolicy::Static(PathBuf::from(unsafe { cstr::to_str_lossy(policy.static_path).to_string() })),
        PMC_JVM_POLICY_SYSTEM => JvmPolicy::System,
        PMC_JVM_POLICY_MOJANG => JvmPolicy::Mojang,
        PMC_JVM_POLICY_SYSTEM_THEN_MOJANG => JvmPolicy::SystemThenMojang,
        PMC_JVM_POLICY_MOJANG_THEN_SYSTEM => JvmPolicy::MojangThenSystem,
    });

}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_launcher_name(inst: &Installer) -> NonNull<c_char> {
    extern_cstr_from_str(inst.launcher_name())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_launcher_name(inst: &mut Installer, name: *const c_char) {
    inst.set_launcher_name(unsafe { cstr::to_str_lossy(name) });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_launcher_version(inst: &Installer) -> NonNull<c_char> {
    extern_cstr_from_str(inst.launcher_version())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_set_launcher_version(inst: &mut Installer, version: *const c_char) {
    inst.set_launcher_version(unsafe { cstr::to_str_lossy(version) });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_install(inst: &mut Installer, handler: raw::pmc_handler, err: *mut *mut raw::pmc_err) -> Option<NonNull<Game>> {
    
    struct AdapterHandler(raw::pmc_handler);

    impl Handler for AdapterHandler {
        fn on_event(&mut self, event: Event) {
            
            let extern_event = match event {
                Event::FilterFeatures { features: _ } => todo!(),
                Event::LoadedFeatures { features } => {

                    let mut owned_owned_features = Vec::with_capacity(features.len());
                    for feature in features {
                        owned_owned_features.push(Pin::new(cstr::from_string(feature.clone())));
                    }
                    let owned_owned_features = Pin::new(owned_owned_features.into_boxed_slice());

                    let mut owned_features = Vec::with_capacity(features.len());
                    for owned_owned_feature in &*owned_owned_features {
                        owned_features.push(owned_owned_feature.as_ptr());
                    }
                    let owned_features = Pin::new(owned_features.into_boxed_slice());

                    raw::pmc_event_base_loaded_features {
                        features: todo!(),
                        features_len: todo!(),
                    }

                }
                Event::LoadHierarchy { root_version } => todo!(),
                Event::LoadedHierarchy { hierarchy } => todo!(),
                Event::LoadVersion { version, file } => todo!(),
                Event::NeedVersion { version, file, retry } => todo!(),
                Event::LoadedVersion { version, file } => todo!(),
                Event::LoadClient => todo!(),
                Event::LoadedClient { file } => todo!(),
                Event::LoadLibraries => todo!(),
                Event::FilterLibraries { libraries } => todo!(),
                Event::LoadedLibraries { libraries } => todo!(),
                Event::FilterLibrariesFiles { class_files, natives_files } => todo!(),
                Event::LoadedLibrariesFiles { class_files, natives_files } => todo!(),
                Event::NoLogger => todo!(),
                Event::LoadLogger { id } => todo!(),
                Event::LoadedLogger { id } => todo!(),
                Event::NoAssets => todo!(),
                Event::LoadAssets { id } => todo!(),
                Event::LoadedAssets { id, count } => todo!(),
                Event::VerifiedAssets { id, count } => todo!(),
                Event::LoadJvm { major_version } => todo!(),
                Event::FoundJvmSystemVersion { file, version, compatible } => todo!(),
                Event::WarnJvmUnsupportedDynamicCrt => todo!(),
                Event::WarnJvmUnsupportedPlatform => todo!(),
                Event::WarnJvmMissingDistribution => todo!(),
                Event::LoadedJvm { file, version, compatible } => todo!(),
                Event::DownloadResources { cancel } => todo!(),
                Event::DownloadProgress { count, total_count, size, total_size } => todo!(),
                Event::DownloadedResources => todo!(),
                Event::ExtractedBinaries { dir } => todo!(),
                _ => todo!(),
            };

        }
    }
    
    extern_err_catch(err, || {
        inst.install(AdapterHandler(handler)).map(extern_box)
    })
}

// TODO:

// =======
// Binding for Game
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_game_jvm_file(game: &Game) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", game.jvm_file.display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_game_mc_dir(game: &Game) -> Option<NonNull<c_char>> {
    extern_cstr_from_fmt(format_args!("{}", game.mc_dir.display())).ok()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_game_main_class(game: &Game) -> NonNull<c_char> {
    extern_cstr_from_str(&game.main_class)
}

// TODO: