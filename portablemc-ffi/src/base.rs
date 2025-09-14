//! Standard installer.

use std::ptr::{self, NonNull};
use std::path::PathBuf;
use std::ffi::c_char;

use portablemc::base::{Error, Event, Game, Handler, Installer, JvmPolicy, VersionChannel};

use crate::alloc::{extern_box, extern_cstr_from_fmt, extern_cstr_from_str};
use crate::err::{extern_err_catch, extern_err, IntoExternErr};
use crate::event::AdapterHandler;
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
        owned: Option<Box<[c_char]>>,
    }

    use raw::pmc_jvm_policy_tag::*;

    let tag = match inst.jvm_policy() {
        JvmPolicy::Static(_) => PMC_JVM_POLICY_STATIC,
        JvmPolicy::System => PMC_JVM_POLICY_SYSTEM,
        JvmPolicy::Mojang => PMC_JVM_POLICY_MOJANG,
        JvmPolicy::SystemThenMojang => PMC_JVM_POLICY_SYSTEM_THEN_MOJANG,
        JvmPolicy::MojangThenSystem => PMC_JVM_POLICY_MOJANG_THEN_SYSTEM,
    };

    let owned = if let JvmPolicy::Static(static_path) = inst.jvm_policy() {
        Some(cstr::from(static_path))
    } else {
        None
    };

    extern_box(ExternJvmPolicy {
        inner: raw::pmc_jvm_policy { 
            tag, 
            static_path: owned
                .as_deref()
                .map(|slice| slice.as_ptr())
                .unwrap_or(ptr::null()),
        },
        owned,
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
    extern_err_catch(err, || {
        inst.install(AdapterHandler(handler)).map(extern_box)
    })
}

impl Handler for AdapterHandler {
    fn on_event(&mut self, event: Event) {
        use raw::pmc_event_tag::*;
        use raw::pmc_version_channel::*;
        match event {
            Event::FilterFeatures { features: _ } => {
                self.forward(PMC_EVENT_BASE_FILTER_FEATURES, raw::pmc_event_data::default());
            }
            Event::LoadedFeatures { features } => {

                let mut buffers = Vec::with_capacity(features.len());
                let mut features_raw = Vec::with_capacity(features.len());

                for feature in features {
                    let feature = cstr::from(feature);
                    features_raw.push(feature.as_ptr());
                    buffers.push(feature);
                }

                self.forward(PMC_EVENT_BASE_LOADED_FEATURES, raw::pmc_event_base_loaded_features {
                    features_len: features_raw.len() as _,
                    features: features_raw.as_ptr(),
                });

            }
            Event::LoadHierarchy { root_version } => {
                let root_version = cstr::from(root_version);
                self.forward(PMC_EVENT_BASE_LOAD_HIERARCHY, raw::pmc_event_base_load_hierarchy {
                    root_version: root_version.as_ptr(),
                });
            }
            Event::LoadedHierarchy { hierarchy } => {

                let mut buffers = Vec::with_capacity(hierarchy.len());
                let mut hierarchy_raw = Vec::with_capacity(hierarchy.len());

                for version in hierarchy {

                    let (buffer, [name, dir]) = 
                        cstr::concat([cstr::from_ref(version.name()), cstr::from_ref(version.dir())]);

                    buffers.push(buffer);
                    hierarchy_raw.push(raw::pmc_loaded_version {
                        name,
                        dir,
                        channel: match version.channel() {
                            None => PMC_VERSION_CHANNEL_UNSPECIFIED,
                            Some(VersionChannel::Release) => PMC_VERSION_CHANNEL_RELEASE,
                            Some(VersionChannel::Snapshot) => PMC_VERSION_CHANNEL_SNAPSHOT,
                            Some(VersionChannel::Beta) => PMC_VERSION_CHANNEL_BETA,
                            Some(VersionChannel::Alpha) => PMC_VERSION_CHANNEL_ALPHA,
                        },
                    });

                }

                self.forward(PMC_EVENT_BASE_LOADED_HIERARCHY, raw::pmc_event_base_loaded_hierarchy {
                    hierarchy_len: hierarchy_raw.len() as _,
                    hierarchy: hierarchy_raw.as_ptr(),
                });

            }
            Event::LoadVersion { version, file } => {

                let (_buffer, [version, file]) = 
                    cstr::concat([cstr::from_ref(version), cstr::from_ref(file)]);

                self.forward(PMC_EVENT_BASE_LOAD_VERSION, raw::pmc_event_base_load_version {
                    version,
                    file,
                });

            }
            Event::NeedVersion { version, file, retry } => {

                let (_buffer, [version, file]) = 
                    cstr::concat([cstr::from_ref(version), cstr::from_ref(file)]);

                self.forward(PMC_EVENT_BASE_NEED_VERSION, raw::pmc_event_base_need_version {
                    version,
                    file,
                    retry,
                });

            }
            Event::LoadedVersion { version, file } => {

                let (_buffer, [version, file]) = 
                    cstr::concat([cstr::from_ref(version), cstr::from_ref(file)]);

                self.forward(PMC_EVENT_BASE_LOADED_VERSION, raw::pmc_event_base_load_version {
                    version,
                    file,
                });

            }
            Event::LoadClient => {
                self.forward(PMC_EVENT_BASE_LOAD_CLIENT, raw::pmc_event_data::default());
            }
            Event::LoadedClient { file } => {
                let file = cstr::from(file.to_path_buf());
                self.forward(PMC_EVENT_BASE_LOADED_CLIENT, raw::pmc_event_base_loaded_client {
                    file: file.as_ptr(),
                });
            }
            Event::LoadLibraries => {
                self.forward(PMC_EVENT_BASE_LOAD_LIBRARIES, raw::pmc_event_data::default());
            }
            Event::FilterLibraries { libraries: _ } => {
                self.forward(PMC_EVENT_BASE_FILTER_LIBRARIES, raw::pmc_event_data::default());
            }
            Event::LoadedLibraries { libraries } => {

                let mut buffers = Vec::with_capacity(libraries.len() * 3);
                let mut downloads_raw = Vec::with_capacity(libraries.len());
                let mut libraries_raw = Vec::with_capacity(libraries.len());

                for library in libraries {
                    
                    let mut library_raw = raw::pmc_loaded_library {
                        gav: {
                            let buf = cstr::from(library.name.as_str());
                            let ptr = buf.as_ptr();
                            buffers.push(buf);
                            ptr
                        },
                        path: ptr::null(),
                        download: ptr::null(),
                        natives: library.natives,
                    };

                    if let Some(path) = &library.path {
                        let buf = cstr::from(path);
                        library_raw.path = buf.as_ptr();
                        buffers.push(buf);
                    }
                    
                    if let Some(download) = &library.download {
                        
                        let download_raw = Box::new(raw::pmc_library_download {
                            url: {
                                let buf = cstr::from(&download.url);
                                let ptr = buf.as_ptr();
                                buffers.push(buf);
                                ptr
                            },
                            size: download.size.unwrap_or(u32::MAX),
                            sha1: download.sha1.as_ref().map(|sha1| sha1 as *const _).unwrap_or(ptr::null()),
                        });

                        library_raw.download = &*download_raw;
                        downloads_raw.push(download_raw);

                    }

                    libraries_raw.push(library_raw);

                }

                self.forward(PMC_EVENT_BASE_LOADED_LIBRARIES, raw::pmc_event_base_loaded_libraries {
                    libraries_len: libraries_raw.len() as _,
                    libraries: libraries_raw.as_ptr(),
                });

            }
            Event::FilterLibrariesFiles { class_files: _, natives_files: _ } => {
                self.forward(PMC_EVENT_BASE_FILTER_LIBRARIES_FILES, raw::pmc_event_data::default());
            },
            Event::LoadedLibrariesFiles { class_files, natives_files } => {
                
                let mut buffers = Vec::with_capacity(class_files.len() + natives_files.len());
                let mut class_files_raw = Vec::with_capacity(class_files.len());
                let mut natives_files_raw = Vec::with_capacity(natives_files.len());

                for class_file in class_files {
                    let buf = cstr::from(class_file);
                    class_files_raw.push(buf.as_ptr());
                    buffers.push(buf);
                }

                for natives_file in natives_files {
                    let buf = cstr::from(natives_file);
                    natives_files_raw.push(buf.as_ptr());
                    buffers.push(buf);
                }

                self.forward(PMC_EVENT_BASE_LOADED_LIBRARIES_FILES, raw::pmc_event_base_loaded_libraries_files {
                    class_files_len: class_files_raw.len() as _,
                    class_files: class_files_raw.as_ptr(),
                    natives_files_len: natives_files_raw.len() as _,
                    natives_files: natives_files_raw.as_ptr(),
                });

            }
            Event::NoLogger => {
                self.forward(PMC_EVENT_BASE_NO_LOGGER, raw::pmc_event_data::default());
            }
            Event::LoadLogger { id } => {
                let id = cstr::from(id.to_string());
                self.forward(PMC_EVENT_BASE_LOAD_LOGGER, raw::pmc_event_base_load_logger {
                    id: id.as_ptr(),
                });
            }
            Event::LoadedLogger { id } => {
                let id = cstr::from(id.to_string());
                self.forward(PMC_EVENT_BASE_LOADED_LOGGER, raw::pmc_event_base_loaded_logger {
                    id: id.as_ptr(),
                });
            }
            Event::NoAssets => {
                self.forward(PMC_EVENT_BASE_NO_ASSETS, raw::pmc_event_data::default());
            }
            Event::LoadAssets { id } => {
                let id = cstr::from(id.to_string());
                self.forward(PMC_EVENT_BASE_LOAD_ASSETS, raw::pmc_event_base_load_assets {
                    id: id.as_ptr(),
                });
            }
            Event::LoadedAssets { id, count } => {
                let id = cstr::from(id.to_string());
                self.forward(PMC_EVENT_BASE_LOADED_ASSETS, raw::pmc_event_base_loaded_assets {
                    id: id.as_ptr(),
                    count: count as _,
                });
            }
            Event::VerifiedAssets { id, count } => {
                let id = cstr::from(id.to_string());
                self.forward(PMC_EVENT_BASE_VERIFIED_ASSETS, raw::pmc_event_base_loaded_assets {
                    id: id.as_ptr(),
                    count: count as _,
                });
            }
            Event::LoadJvm { major_version } => {
                self.forward(PMC_EVENT_BASE_LOAD_JVM, raw::pmc_event_base_load_jvm {
                    major_version,
                });
            }
            Event::FoundJvmSystemVersion { file, version, compatible } => {
                
                let (_buffer, [file, version]) = 
                    cstr::concat([cstr::from_ref(file), cstr::from_ref(version)]);

                self.forward(PMC_EVENT_BASE_FOUND_JVM_VERSION, raw::pmc_event_base_found_jvm_system_version {
                    file,
                    version,
                    compatible,
                });

            }
            Event::WarnJvmUnsupportedDynamicCrt => {
                self.forward(PMC_EVENT_BASE_WARN_JVM_UNSUPPORTED_DYNAMIC_CTR, raw::pmc_event_data::default());
            }
            Event::WarnJvmUnsupportedPlatform => {
                self.forward(PMC_EVENT_BASE_WARN_JVM_UNSUPPORTED_PLATFORM, raw::pmc_event_data::default());
            }
            Event::WarnJvmMissingDistribution => {
                self.forward(PMC_EVENT_BASE_WARN_JVM_MISSING_DISTRIBUTION, raw::pmc_event_data::default());
            }
            Event::LoadedJvm { file, version, compatible } => {

                let no_version = version.is_none();
                let (_buffer, [file, version]) = 
                    cstr::concat([cstr::from_ref(file), cstr::from_ref(version.unwrap_or(""))]);

                self.forward(PMC_EVENT_BASE_FOUND_JVM_VERSION, raw::pmc_event_base_found_jvm_system_version {
                    file,
                    version: if no_version { ptr::null() } else { version },
                    compatible,
                });

            }
            Event::DownloadResources { cancel } => {
                self.forward(PMC_EVENT_BASE_DOWNLOAD_RESOURCES, raw::pmc_event_base_download_resources {
                    cancel,
                });
            }
            Event::DownloadedResources => {
                self.forward(PMC_EVENT_BASE_DOWNLOADED_RESOURCES, raw::pmc_event_data::default());
            }
            Event::DownloadProgress { count, total_count, size, total_size } => {
                self.forward(PMC_EVENT_BASE_DOWNLOAD_PROGRESS, raw::pmc_event_base_download_progress {
                    count,
                    total_count,
                    size,
                    total_size,
                });
            }
            Event::ExtractedBinaries { dir } => {
                let dir = cstr::from(dir.to_path_buf());
                self.forward(PMC_EVENT_BASE_EXTRACTED_BINARIES, raw::pmc_event_base_extracted_binaries {
                    dir: dir.as_ptr(),
                });
            }
            _ => todo!(),
        }
    }
}

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

#[no_mangle]
pub unsafe extern "C" fn pmc_game_jvm_args(game: &Game) -> NonNull<raw::pmc_game_args> {
    extern_game_args(&game.jvm_args)
}

#[no_mangle]
pub unsafe extern "C" fn pmc_game_game_args(game: &Game) -> NonNull<raw::pmc_game_args> {
    extern_game_args(&game.game_args)
}

fn extern_game_args(args: &[String]) -> NonNull<raw::pmc_game_args> {

    #[repr(C)]
    struct ExternGameArgs {
        inner: raw::pmc_game_args,
        args_raw: Box<[*const c_char]>,
        buffers: Box<[Box<[c_char]>]>,
    }

    let mut buffers = Vec::with_capacity(args.len());
    let mut args_raw = Vec::with_capacity(args.len());

    for arg in args {
        let buffer = cstr::from(arg.clone());
        args_raw.push(buffer.as_ptr());
        buffers.push(buffer);
    }

    extern_box(ExternGameArgs {
        inner: raw::pmc_game_args {
            len: args_raw.len() as _,
            args: args_raw.as_ptr(),
        },
        args_raw: args_raw.into_boxed_slice(),
        buffers: buffers.into_boxed_slice(),
    }).cast::<raw::pmc_game_args>()

}

#[no_mangle]
pub unsafe extern "C" fn pmc_game_spawn(game: &Game, err: *mut *mut raw::pmc_err) -> u32 {
    extern_err_catch(err, || {
        game.spawn().map(|child| child.id())
    }).unwrap_or(0)
}
