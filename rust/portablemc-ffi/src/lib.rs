use std::ffi::{c_char, CStr, OsStr};
use std::ptr;

use portablemc::{standard, mojang, fabric, forge};


/// The abstract `pmc_inst` type used for pointers in the FFI.
enum Installer {
    Standard(standard::Installer),
    Mojang(mojang::Installer),
    Fabric(fabric::Installer),
    Forge(forge::Installer),
}

extern "C" fn pmc_standard_new(version: *const c_char, main_dir: *const c_char) -> *mut Installer {

    let version = unsafe { CStr::from_ptr(version) };
    let version = match version.to_str() {
        Ok(s) => s,
        Err(_e) => {
            return ptr::null_mut();
        }
    };

    let main_dir = if main_dir.is_null() { 
        None 
    } else { 
        Some(unsafe { CStr::from_ptr(main_dir) })
    };

    // let inst = standard::Installer::new(version, main_dir)
    // let inst = Box::new(Installer::Standard())

    todo!()

}
