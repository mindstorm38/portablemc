//! Error handling for C.

use std::ffi::{c_char, c_void};
use std::error::Error;
use std::ptr;

use crate::alloc::{extern_box, pmc_free, extern_box_cstr_from_fmt};


/// This function is a helper for functions that takes an 'pmc_err *err' double pointer.
#[inline]
pub fn wrap_error<F, R, E>(func: F, err: *mut *mut Err, default: R) -> R
where
    F: FnOnce() -> Result<R, E>,
    R: Copy,
    E: ExposedError + 'static,
{

    // If the given pointer isn't null, then we read it, and if this pointer isn't null
    // we free the old error first and set it null.
    if !err.is_null() {
        let old_err = unsafe { err.replace(ptr::null_mut()) };
        pmc_free(old_err.cast());
    }

    match func() {
        Ok(v) => v,
        Err(e) => {

            if !err.is_null() {
                unsafe { err.write(extern_err(e)); }
            }

            default

        }
    }

}

/// Error codes definitions, shared with C define macros.
pub mod code {

    pub const INTERNAL: u8                      = 0x01;

    pub const MSA_AUTH_DECLINED: u8             = 0x40;
    pub const MSA_AUTH_TIMED_OUT: u8            = 0x41;
    pub const MSA_AUTH_OUTDATED_TOKEN: u8       = 0x42;
    pub const MSA_AUTH_DOES_NOT_OWN_GAME: u8    = 0x43;
    pub const MSA_AUTH_INVALID_STATUS: u8       = 0x44;
    pub const MSA_AUTH_UNKNOWN: u8              = 0x45;

    pub const MSA_DATABASE_IO: u8               = 0x50;
    pub const MSA_DATABASE_CORRUPTED: u8        = 0x51;
    pub const MSA_DATABASE_WRITE_FAILED: u8     = 0x52;
    
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

/// Extension trait for a Result<T, E: ExposedError>
pub trait ResultErrExt<T> {
    fn with_extern_err(self, err: *mut *mut Err) -> Result<T, ()>;
}

impl<T, E: ExposedError + 'static> ResultErrExt<T> for Result<T, E> {
    fn with_extern_err(self, err: *mut *mut Err) -> Result<T, ()> {
        
        // If the given pointer isn't null, then we read it, and if this pointer isn't null
        // we free the old error first and set it null.
        if !err.is_null() {
            let old_err = unsafe { err.replace(ptr::null_mut()) };
            pmc_free(old_err.cast());
        }

        // if let Err(e) = &self {
        //     unsafe { err.write(extern_err(e)); }
        // }

    }
}

/// Allocate an extern box (see [`crate::alloc::extern_box`]) that contains the given 
/// error type, returning a pointer to the describing structure.
#[inline]
pub fn extern_err<E: ExposedError + 'static>(err: E) -> *mut Err {
    extern_box(Err { inner: Box::new(err) })
}

/// The opaque `pmc_err` type.
pub struct Err {
    inner: Box<dyn ExposedError>,
}

// =======
// Bindings
// =======

#[no_mangle]
extern "C" fn pmc_err_code(err: *const Err) -> u8 {
    let err = unsafe { &*err };
    err.inner.code()
}

#[no_mangle]
extern "C" fn pmc_err_data(err: *const Err) -> *mut c_void {
    let err = unsafe { &*err };
    err.inner.extern_data().cast()
}

#[no_mangle]
extern "C" fn pmc_err_message(err: *const Err) -> *mut c_char {
    let err = unsafe { &*err };
    extern_box_cstr_from_fmt(format_args!("{}", err.inner))
}
