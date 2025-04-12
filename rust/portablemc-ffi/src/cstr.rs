//! Various C characters (nul terminated) utilities.

use std::ffi::{c_char, CStr};


/// Load a UTF-8 string from a C nul-terminated pointer, this returns none if the given
/// string is not UTF-8.
/// 
/// # SAFETY
/// 
/// It starts by creating a [`CStr`] from the given pointer, so the caller must uphold
/// the same safety guarantees than [`CStr::from_ptr`].
pub unsafe fn str_from_cstr_ptr<'a>(cstr: *const c_char) -> Option<&'a str> {
    let app_id = unsafe { CStr::from_ptr::<'a>(cstr) };
    app_id.to_str().ok()
}
