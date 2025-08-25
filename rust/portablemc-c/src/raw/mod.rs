//! The C language type bindings.

/// The header file contains the bindings to the C part of the header file. This file
/// should be generated with bindgen using the following command in order to only generate
/// layouts for structures, and not functions. Functions should be manually defined.
/// 
/// ```
/// $ bindgen include/portablemc.h -o src/raw/generated.rs --generate types --allowlist-type "pmc_.*" --default-enum-style rust --ctypes-prefix ::std::ffi --no-layout-tests --no-size_t-is-usize
/// ```
#[allow(non_camel_case_types)]
#[allow(unsafe_op_in_unsafe_fn)]
mod generated;
pub use generated::*;


/// Internal macro to implement the [`From`] trait for each field to their union.
macro_rules! impl_union_from_field {
    ( 
        for $union_type:ident,
        $( $field:ident : $field_type:ident ),* $(,)?
    ) => {
        $(
        impl From<$field_type> for $union_type {
            #[inline]
            fn from($field: $field_type) -> Self {
                Self { $field }
            }
        }
        )*
    };
}

impl Default for pmc_err_data {
    fn default() -> Self {
        pmc_err_data { _none: 0 }
    }
}

impl_union_from_field! {
    for pmc_err_data,
    internal: pmc_err_data_internal,
    msa_auth_invalid_status: pmc_err_data_msa_auth_invalid_status,
    msa_auth_unknown: pmc_err_data_msa_auth_unknown,
    base_hierarchy_loop: pmc_err_base_hierarchy_loop,
    base_version_not_found: pmc_err_base_version_not_found,
    base_assets_not_found: pmc_err_base_assets_not_found,
    base_library_not_found: pmc_err_base_library_not_found,
    base_jvm_not_found: pmc_err_base_jvm_not_found,
    moj_lwjgl_fix_not_found: pmc_err_moj_lwjgl_fix_not_found,
    fabric_latest_version_not_found: pmc_err_fabric_latest_version_not_found,
    fabric_game_version_not_found: pmc_err_fabric_game_version_not_found,
    fabric_loader_version_not_found: pmc_err_fabric_loader_version_not_found,
    forge_latest_version_not_found: pmc_err_forge_latest_version_not_found,
    forge_installer_not_found: pmc_err_forge_installer_not_found,
    forge_installer_file_not_found: pmc_err_forge_installer_file_not_found,
    forge_installer_processor_not_found: pmc_err_forge_installer_processor_not_found,
    forge_installer_processor_failed: pmc_err_forge_installer_processor_failed,
    forge_installer_processor_corrupted: pmc_err_forge_installer_processor_corrupted,
}

impl Default for pmc_event_data {
    fn default() -> Self {
        pmc_event_data { _none: 0 }
    }
}

impl_union_from_field! {
    for pmc_event_data,
    base_loaded_features: pmc_event_base_loaded_features,
    base_load_hierarchy: pmc_event_base_load_hierarchy,
    base_loaded_hierarchy: pmc_event_base_loaded_hierarchy,
    base_load_version: pmc_event_base_load_version,
    base_loaded_version: pmc_event_base_loaded_version,
    base_need_version: pmc_event_base_need_version,
    base_loaded_client: pmc_event_base_loaded_client,
    base_loaded_libraries: pmc_event_base_loaded_libraries,
    base_loaded_libraries_files: pmc_event_base_loaded_libraries_files,
    base_load_logger: pmc_event_base_load_logger,
    base_loaded_logger: pmc_event_base_loaded_logger,
    base_load_assets: pmc_event_base_load_assets,
    base_loaded_assets: pmc_event_base_loaded_assets,
    base_verified_assets: pmc_event_base_verified_assets,
    base_load_jvm: pmc_event_base_load_jvm,
    base_found_jvm_system_version: pmc_event_base_found_jvm_system_version,
    base_loaded_jvm: pmc_event_base_loaded_jvm,
    base_download_resources: pmc_event_base_download_resources,
    base_download_progress: pmc_event_base_download_progress,
    base_extracted_binaries: pmc_event_base_extracted_binaries,
    moj_invalidated_version: pmc_event_moj_invalidated_version,
    moj_fetch_version: pmc_event_moj_fetch_version,
    moj_fetched_version: pmc_event_moj_fetched_version,
    moj_fixed_legacy_proxy: pmc_event_moj_fixed_legacy_proxy,
    fabric_fetch_version: pmc_event_fabric_fetch_version,
    fabric_fetched_version: pmc_event_fabric_fetched_version,
    forge_installing: pmc_event_forge_installing,
    forge_fetch_installer: pmc_event_forge_fetch_installer,
    forge_fetched_installer: pmc_event_forge_fetched_installer,
    forge_run_installer_processor: pmc_event_forge_run_installer_processor,
}
