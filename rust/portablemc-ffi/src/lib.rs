//! PortableMC bindings for C.
//! 
//! In this library, the naming scheme is simple. All functions that are exported and
//! therefore also defined in the header file are prefixed with `pmc_`, they should use
//! the extern "C" ABI.

#![deny(unsafe_op_in_unsafe_fn)]

pub mod alloc;
pub mod err;

pub mod msa;

pub mod standard;


use std::ffi::{c_char, CStr};


/// The non-opaque `pmc_uuid` type.
#[allow(non_camel_case_types)]
pub type pmc_uuid = [u8; 16];


/// Load a UTF-8 string from a C nul-terminated pointer, this returns none if the given
/// string is not UTF-8.
/// 
/// # SAFETY
/// 
/// It starts by creating a [`CStr`] from the given pointer, so the caller must uphold
/// the same safety guarantees than [`CStr::from_ptr`].
pub unsafe fn str_from_cstr_ptr<'a>(cstr: *const c_char) -> Option<&'a str> {
    let cstr = unsafe { CStr::from_ptr::<'a>(cstr) };
    cstr.to_str().ok()
}
