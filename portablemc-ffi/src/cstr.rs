//! C-string utilities.

use std::ffi::{c_char, CStr, OsStr, OsString};
use std::path::{Path, PathBuf};
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

/// A trait for converting a type into a slice of bytes that should represent a c-string
/// bytes stream with a potential NUL byte somewhere.
pub trait AsCstrBytes {
    fn as_cstr_bytes(&self) -> &[u8];
}

impl AsCstrBytes for [u8] {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self
    }
}

impl AsCstrBytes for Vec<u8> {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsCstrBytes for str {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsCstrBytes for String {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_str().as_cstr_bytes()
    }
}

impl AsCstrBytes for OsStr {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_encoded_bytes()
    }
}

impl AsCstrBytes for OsString {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_os_str().as_cstr_bytes()
    }
}

impl AsCstrBytes for Path {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_os_str().as_encoded_bytes()
    }
}

impl AsCstrBytes for PathBuf {
    #[inline]
    fn as_cstr_bytes(&self) -> &[u8] {
        self.as_path().as_cstr_bytes()
    }
}

pub fn from_ref<'a>(buf: &'a (impl AsCstrBytes + ?Sized)) -> &'a [c_char] {
    // SAFETY: u8 and c_char have same layout.
    let buf = unsafe { &*(buf.as_cstr_bytes() as *const [u8] as *const [c_char]) };
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    &buf[..len]
}

/// A trait for converting an owned type into a owned box of bytes representing a 
/// c-string with a potential NUL byte somewhere.
pub trait IntoCstrBytes: Sized {
    fn into_cstr_bytes(self) -> Vec<u8>;
}

impl<T: AsCstrBytes + ?Sized> IntoCstrBytes for &'_ T {
    fn into_cstr_bytes(self) -> Vec<u8> {
        self.as_cstr_bytes().to_vec()
    }
}

impl<T: AsCstrBytes + ?Sized> IntoCstrBytes for Box<T> {
    #[inline]
    fn into_cstr_bytes(self) -> Vec<u8> {
        self.as_cstr_bytes().to_vec()
    }
}

impl IntoCstrBytes for Vec<u8> {
    #[inline]
    fn into_cstr_bytes(self) -> Vec<u8> {
        self
    }
}

impl IntoCstrBytes for String {
    #[inline]
    fn into_cstr_bytes(self) -> Vec<u8> {
        self.into_bytes()
    }
}

impl IntoCstrBytes for OsString {
    #[inline]
    fn into_cstr_bytes(self) -> Vec<u8> {
        self.into_encoded_bytes()
    }
}

impl IntoCstrBytes for PathBuf {
    #[inline]
    fn into_cstr_bytes(self) -> Vec<u8> {
        self.into_os_string().into_encoded_bytes()
    }
}

/// Truncate and then add a terminating NUL to the given octets buffer.
pub fn from(buf: impl IntoCstrBytes) -> Box<[c_char]> {
    let mut buf = buf.into_cstr_bytes();
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    buf.truncate(len);
    buf.push(0);
    // SAFETY: u8 and c_char have same layout!
    let raw = Box::into_raw(buf.into_boxed_slice());
    unsafe { Box::from_raw(raw as *mut [c_char]) }
}

/// Concatenate multiple c-string slices (without NUL, see [`from_ref`]).
pub fn concat<'a, const N: usize>(slices: [&'a [c_char]; N]) -> (Box<[c_char]>, [*const c_char; N]) {
    let mut buffer = Vec::with_capacity(slices.iter().map(|s| s.len() + 1).sum());
    let offset = slices.map(|slice| {
        let offset = buffer.len();
        buffer.extend_from_slice(slice);
        buffer.push(0);  // NUL
        offset
    });
    let buffer = buffer.into_boxed_slice();
    let pointers = offset.map(|offset| unsafe { buffer.as_ptr().add(offset) });
    (buffer, pointers)
}
