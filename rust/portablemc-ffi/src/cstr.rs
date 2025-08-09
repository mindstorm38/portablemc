//! C-string utilities.

use std::ffi::{c_char, CStr};
use std::borrow::Cow;


/// Load a UTF-8 string from a C nul-terminated pointer, this returns none if the given
/// string is not UTF-8.
/// 
/// # SAFETY
/// 
/// It starts by creating a [`CStr`] from the given pointer, so the caller must uphold
/// the same safety guarantees than [`CStr::from_ptr`].
#[inline]
pub unsafe fn to_str<'a>(cstr: *const c_char) -> Option<&'a str> {
    let cstr = unsafe { CStr::from_ptr(cstr) };
    cstr.to_str().ok()
}

/// Same as [`to_str`] but returning an owned string with replacement characters for
/// any invalid UTF-8 characters.
#[inline]
pub unsafe fn to_str_lossy<'a>(cstr: *const c_char) -> Cow<'a, str> {
    let cstr = unsafe { CStr::from_ptr(cstr) };
    cstr.to_string_lossy()
}

/// Get a bytes slice of cstr of the string, optionally until a nul-terminating char.
/// The nul char is not included!
#[inline]
pub fn from_str<'a>(s: &'a str) -> &'a [c_char] {
    // SAFETY: u8 and c_char have same layout.
    let bytes = unsafe { &*(s.as_bytes() as *const [u8] as *const [c_char]) };
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..len]
}

#[inline]
pub fn from_string(s: String) -> Box<[c_char]> {
    let mut bytes = s.into_bytes();
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes.truncate(len);
    bytes.push(0);
    // SAFETY: u8 and c_char have same layout!
    let raw = Box::into_raw(bytes.into_boxed_slice());
    unsafe { Box::from_raw(raw as *mut [c_char]) }
}
