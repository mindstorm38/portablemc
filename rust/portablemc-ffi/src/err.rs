//! Error handling for C.

use std::ffi::{c_char, c_void};
use std::error::Error;
use std::ptr;

use crate::alloc::{extern_box, extern_box_drop_unchecked, extern_box_cstr_from_fmt};


/// Error codes definitions, shared with C define macros.
pub mod code {

    pub const INTERNAL: u8                      = 0x01;

    pub const MSA_AUTH_DECLINED: u8             = 0x10;
    pub const MSA_AUTH_TIMED_OUT: u8            = 0x11;
    pub const MSA_AUTH_OUTDATED_TOKEN: u8       = 0x12;
    pub const MSA_AUTH_DOES_NOT_OWN_GAME: u8    = 0x13;
    pub const MSA_AUTH_INVALID_STATUS: u8       = 0x14;
    pub const MSA_AUTH_UNKNOWN: u8              = 0x15;

    pub const MSA_DATABASE_IO: u8               = 0x20;
    pub const MSA_DATABASE_CORRUPTED: u8        = 0x21;
    pub const MSA_DATABASE_WRITE_FAILED: u8     = 0x22;
    
    pub const STANDARD_HIERARCHY_LOOP: u8       = 0x30;
    pub const STANDARD_VERSION_NOT_FOUND: u8    = 0x31;
    pub const STANDARD_ASSETS_NOT_FOUND: u8     = 0x32;
    pub const STANDARD_CLIENT_NOT_FOUND: u8     = 0x33;
    pub const STANDARD_LIBRARY_NOT_FOUND: u8    = 0x34;
    pub const STANDARD_JVM_NOT_FOUND: u8        = 0x35;
    pub const STANDARD_MAIN_CLASS_NOT_FOUND: u8 = 0x36;
    pub const STANDARD_DOWNLOAD_RESOURCES_CANCELLED: u8 = 0x37;
    pub const STANDARD_DOWNLOAD: u8             = 0x38;

}

/// A trait to implement on all [`Error`] implementors that also can be exposed to C.
pub trait ExposedError: Error {

    /// Get a code defined for this exposed error.
    fn code(&self) -> u8;

    /// If the exposed error provides an additional data that should be given to the user
    /// through, this function returns that extern data. Note that this function is 
    /// responsible for allocating an extern box type that can be later freed using
    /// `pmc_free`.
    fn extern_data(&self) -> *mut () {
        ptr::null_mut()
    }

}

/// Allocate an extern box (see [`crate::alloc::extern_box`]) that contains the given 
/// error type, returning a pointer to the describing structure.
#[inline]
pub fn extern_err<E: ExposedError + 'static>(err: E) -> *mut Err {
    extern_box(Err { inner: Box::new(err) })
}

/// If this result is an error, then the error is extracted and moved into an extern
/// error, using [`extern_err`], and written in the pointer. Note that if the pointer
/// of the error is not null, then it is freed anyway, error or not.
#[inline]
pub fn extern_err_with<T, E, F>(err_ptr: *mut *mut Err, func: F) -> Result<T, ()>
where
    E: ExposedError + 'static,
    F: FnOnce() -> Result<T, E>,
{

    // If the given pointer isn't null, then we read it, and if this pointer isn't null
    // we free the old error first and set it null.
    if !err_ptr.is_null() {
        // SAFETY: A pointer is copy and we requires that it's not null and points to 
        // an initialized pointer, even if null.
        let old_err = unsafe { err_ptr.replace(ptr::null_mut()) };
        if !old_err.is_null() {
            // SAFETY: The caller ensure that if there was a pointer, it was a Err ptr.
            unsafe { extern_box_drop_unchecked(old_err); }
        }
    }

    match func() {
        Ok(val) => Ok(val),
        Err(err) => {
            // SAFETY: Write the extern error's pointer we just allocated. We are 
            // replacing the null pointer we stored above.
            unsafe { err_ptr.write(extern_err(err)); }
            Err(())
        }
    }
    
}

/// The opaque `pmc_err` type.
pub struct Err {
    inner: Box<dyn ExposedError>,
}

// =======
// Bindings
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_err_code(err: *const Err) -> u8 {
    let err = unsafe { &*err };
    err.inner.code()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_err_data(err: *const Err) -> *mut c_void {
    let err = unsafe { &*err };
    err.inner.extern_data().cast()
}

#[no_mangle]
pub unsafe extern "C" fn pmc_err_message(err: *const Err) -> *mut c_char {
    let err = unsafe { &*err };
    extern_box_cstr_from_fmt(format_args!("{}", err.inner))
}
