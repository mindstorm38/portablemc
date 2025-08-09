//! Utilities for easier error handling around the `raw::pmc_err` type.

use std::ptr::{self, NonNull};
use std::ffi::{c_char, CStr};
use std::pin::Pin;

use crate::alloc::{extern_box, extern_box_drop_unchecked};
use crate::{cstr, raw};


/// Allocate an extern error.
#[inline]
pub fn extern_err_static(tag: raw::pmc_err_tag, data: impl Into<raw::pmc_err_data>, message: &'static CStr) -> NonNull<raw::pmc_err> {
    extern_box(raw::pmc_err {
        tag,
        data: data.into(),
        message: message.as_ptr(),
    })
}

#[inline]
pub fn extern_err<O>(tag: raw::pmc_err_tag, data: impl Into<raw::pmc_err_data>, message: String, owned: O) -> NonNull<raw::pmc_err> {
    
    #[repr(C)]
    struct ExternErr<O> {
        inner: raw::pmc_err,
        owned_message: Pin<Box<[c_char]>>,
        owned: O,
    }

    let owned_message = Pin::new(cstr::from_string(message));
    
    extern_box(ExternErr {
        inner: raw::pmc_err {
            tag,
            data: data.into(),
            message: owned_message.as_ptr(),
        },
        owned_message,
        owned,
    }).cast::<raw::pmc_err>()

}

/// A trait to bundle an error into an extern `pmc_err` allocated object.
pub trait IntoExternErr {
    fn into(self) -> NonNull<raw::pmc_err>;
}

/// If this result is an error, then the error is extracted and moved into an extern
/// error, using [`extern_err`], and written in the pointer. Note that if the pointer
/// of the error is not null, then it is freed anyway, error or not.
#[inline]
pub fn extern_err_catch<T, E, F>(err_ptr: *mut *mut raw::pmc_err, func: F) -> Option<T>
where
    E: IntoExternErr,
    F: FnOnce() -> Result<T, E>,
{

    // If the given pointer isn't null, then we read it, and if this pointer isn't null
    // we free the old error first and set it null.
    if !err_ptr.is_null() {
        // SAFETY: A pointer is copy and we requires that it's not null and points to 
        // an initialized pointer, even if null.
        let old_err = unsafe { err_ptr.replace(ptr::null_mut()) };
        if let Some(old_err) = NonNull::new(old_err) {
            // SAFETY: The caller ensure that if there was a pointer, it was a Err ptr.
            unsafe { extern_box_drop_unchecked(old_err); }
        }
    }

    match func() {
        Ok(val) => Some(val),
        Err(err) => {
            
            if let Some(err_ptr) = NonNull::new(err_ptr) {
                // SAFETY: Write the extern error's pointer we just allocated. We are 
                // replacing the null pointer we stored above.
                unsafe { err_ptr.write(err.into().as_ptr()); }
            }

            None

        }
    }
    
}
