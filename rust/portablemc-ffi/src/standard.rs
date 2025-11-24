//! Standard installer.

use std::ffi::c_char;
use std::ptr;

use portablemc::standard::{Installer, JvmPolicy};

use crate::alloc::{extern_box, extern_box_cstr_from_fmt, extern_box_drop_unchecked};
use crate::str_from_cstr_ptr;


// =======
// Binding
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
    
    let Some(version) = (unsafe { str_from_cstr_ptr(version) }) else {
        return ptr::null_mut();
    };

    extern_box(Installer::new(version))

}

#[no_mangle]
pub unsafe extern "C" fn pmc_standard_jvm_policy(inst: *const Installer) -> *mut pmc_jvm_policy {
   
    /// This wrapper type is used to return the JVM policy allocated and, if static,
    /// pointing to the inner buffer. The inner type that we return the pointer for
    /// must be placed first.
    #[repr(C)]
    struct ExternJvmPolicy {
        inner: pmc_jvm_policy,
        owned_static_path: *mut c_char,
    }

    impl Drop for ExternJvmPolicy {
        fn drop(&mut self) {
            if !self.owned_static_path.is_null() {
                unsafe { extern_box_drop_unchecked(self.owned_static_path); }
                self.owned_static_path = ptr::null_mut();
                self.inner.static_path = ptr::null();
            }
        }
    }

    let jvm_policy = unsafe { &*inst };
    
    let tag = match jvm_policy.jvm_policy() {
        JvmPolicy::Static(_) => pmc_jvm_policy_tag::Static,
        JvmPolicy::System => pmc_jvm_policy_tag::System,
        JvmPolicy::Mojang => pmc_jvm_policy_tag::Mojang,
        JvmPolicy::SystemThenMojang => pmc_jvm_policy_tag::SystemThenMojang,
        JvmPolicy::MojangThenSystem => pmc_jvm_policy_tag::MojangThenSystem,
    };

    let static_path = if let JvmPolicy::Static(static_path) = jvm_policy.jvm_policy() {
        extern_box_cstr_from_fmt(format_args!("{}", static_path.display()))
    } else {
        ptr::null_mut()
    };

    extern_box(ExternJvmPolicy {
        inner: pmc_jvm_policy { 
            tag, 
            static_path,
        },
        owned_static_path: static_path,
    });

    todo!()

}
