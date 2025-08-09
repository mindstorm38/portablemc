//! The C language type bindings.

/// The header file contains the bindings to the C part of the header file. This file
/// should be generated with bindgen using the following command in order to only generate
/// layouts for structures, and not functions. Functions should be manually defined.
/// 
/// ```
/// $ bindgen include/portablemc.h -o src/raw/generated.rs --generate types --allowlist-type "pmc_.*" --default-enum-style rust --ctypes-prefix ::std::ffi --no-layout-tests --no-size_t-is-usize
/// ```
#[allow(non_camel_case_types)]
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
    std_hierarchy_loop: pmc_err_std_hierarchy_loop,
    std_version_not_found: pmc_err_std_version_not_found,
    std_assets_not_found: pmc_err_std_assets_not_found,
    std_jvm_not_found: pmc_err_std_jvm_not_found,
}
