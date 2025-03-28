//! Standard installer.

use std::ffi::{c_char, CStr, OsStr};
use std::ptr;

use portablemc::standard::Installer;


enum InstallerStore {
    Owned(Installer),
    Borrowed(*mut Installer),
}

impl InstallerStore {

    unsafe fn as_mut<'a>(this: *mut Self) -> &'a mut Installer {
        match unsafe { &mut *this } {
            Self::Owned(ret) => ret,
            Self::Borrowed(ret) => unsafe { &mut **ret },
        }
    }

}

#[no_mangle]
extern "C" fn pmc_standard_new(version: *const c_char) -> *mut InstallerStore {

    // SAFETY: The caller must ensure that it's a valid C string.
    let version = unsafe { CStr::from_ptr(version) };
    let version = match version.to_str() {
        Ok(s) => s.to_string(),
        Err(_e) => {
            return ptr::null_mut();
        }
    };

    let inst = Box::new(InstallerStore::Owned(Installer::new(version)));
    let inst = Box::into_raw(inst);
    inst

}

#[no_mangle]
extern "C" fn pmc_standard_set_version(inst: *mut InstallerStore, version: *const c_char) {

    // SAFETY: The caller guarantees that it's a valid, non shared, pointer.
    let inst = unsafe { &mut *inst };

    inst.

}
