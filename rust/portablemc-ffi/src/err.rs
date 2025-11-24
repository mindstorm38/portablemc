//! Error handling for C.

use std::ffi::{c_char, c_void};
use std::fmt::Arguments;
use std::error::Error;
use std::borrow::Cow;
use std::ptr;


#[inline]
pub fn wrap_error<F, R, E>(func: F, err: *mut *mut Err, default: R) -> R
where
    F: FnOnce() -> Result<R, E>,
    R: Copy,
    E: ExposedError,
{

    // If the given pointer isn't null, then we read it, and if this pointer isn't null
    // we free the old error first and set it null.
    if !err.is_null() {
        let old_err = unsafe { err.replace(ptr::null_mut()) };
        pmc_err_free(old_err);
    }

    match func() {
        Ok(v) => v,
        Err(e) => {

            if !err.is_null() {

                let new_err = Box::new(Err {
                    code: e.code(),
                    message: fmt_arguments_to_cow(format_args!("{e}")),
                    data: e.data(),
                });

                let new_err = Box::into_raw(new_err);

                unsafe { err.write(new_err); }
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

/// The `pmc_err` type.
pub struct Err {
    pub code: u8,
    pub message: Cow<'static, str>,
    pub data: Option<Box<dyn ExposedErrorData>>,
}

/// A trait to implement on all [`Error`] implementors that also can be exposed to C.
pub trait ExposedError: Error {

    /// Get a code defined for this exposed error.
    fn code(&self) -> u8;

    /// If the exposed error provides an additional data that should be given to the user
    /// through 
    fn data(&self) -> Option<Box<dyn ExposedErrorData>> {
        None
    }

}

/// A trait automatically implemented for every type that provides as way to get its 
/// pointer.
pub trait ExposedErrorData {

    /// Create a pointer that expose a C
    fn exposed_ptr(&self) -> *const ();

}

/// Internal function to optimize allocation of formatting arguments.
fn fmt_arguments_to_cow(args: Arguments<'_>) -> Cow<'static, str> {
    match args.as_str() {
        Some(message) => Cow::Borrowed(message),
        None => Cow::Owned(args.to_string()),
    }
}

// =======
// Bindings
// =======

#[no_mangle]
extern "C" fn pmc_err_code(err: *const Err) -> u8 {
    let err = unsafe { &*err };
    err.code
}

#[no_mangle]
extern "C" fn pmc_err_data(err: *const Err) -> *const c_void {
    let err = unsafe { &*err };
    match &err.data {
        Some(data) => data.exposed_ptr().cast(),
        None => ptr::null(),
    }
}

#[no_mangle]
extern "C" fn pmc_err_message(err: *const Err) -> *const c_char {
    let err = unsafe { &*err };
    err.message.as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_err_free(err: *mut Err) {
    if !err.is_null() {
        // SAFETY: The error was initially allocated as a box.
        let err = unsafe { Box::from_raw(err) };
        drop(err);
    }
}
