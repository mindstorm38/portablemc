//! Various uncategorized utilities.

use std::path::{Path, PathBuf};
use std::ffi::OsStr;


/// Extension to the standard [`Path`].
pub trait PathExt {

    /// A shortcut method to join a file name with its extension to the current path.
    /// This shortcut avoids a temporary allocation of a formatted string when joining.
    fn join_with_extension<P: AsRef<Path>, S: AsRef<OsStr>>(&self, name: P, extension: S) -> PathBuf;

}

impl PathExt for Path {

    #[inline]
    fn join_with_extension<P: AsRef<Path>, S: AsRef<OsStr>>(&self, name: P, extension: S) -> PathBuf {
        let mut buf = self.join(name);
        buf.as_mut_os_string().push(".");
        buf.as_mut_os_string().push(extension);
        buf
    }

}
