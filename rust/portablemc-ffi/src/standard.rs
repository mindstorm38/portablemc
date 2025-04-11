//! Standard installer.

use std::ffi::{c_char, CStr};
use std::ptr;

use portablemc::standard::Installer;


#[no_mangle]
extern "C" fn pmc_standard_new(version: *const c_char) -> *mut Installer {

    // SAFETY: The caller must ensure that it's a valid C string.
    let version = unsafe { CStr::from_ptr(version) };
    let version = match version.to_str() {
        Ok(s) => s.to_string(),
        Err(_e) => {
            return ptr::null_mut();
        }
    };

    let inst = Box::new(Installer::new(version));
    let inst = Box::into_raw(inst);
    inst

}

#[no_mangle]
extern "C" fn pmc_standard_set_version(inst: *mut InstallerStore, version: *const c_char) {

    // SAFETY: The caller guarantees that it's a valid, non shared, pointer.
    let inst = unsafe { &mut *inst };

    inst.

}
