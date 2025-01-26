//! Various uncategorized utilities.

use std::path::{Path, PathBuf};
use std::ffi::OsStr;


/// Extension to the standard [`Path`].
pub trait PathExt {

    /// A shortcut method to join a file name with its extension to the current path.
    /// This shortcut avoids a temporary allocation of a formatted string when joining.
    fn join_with_extension<P: AsRef<Path>, S: AsRef<OsStr>>(&self, name: P, extension: S) -> PathBuf;

    fn append<S: AsRef<OsStr>>(&self, s: S) -> PathBuf;

}

impl PathExt for Path {

    #[inline]
    fn join_with_extension<P: AsRef<Path>, S: AsRef<OsStr>>(&self, name: P, extension: S) -> PathBuf {
        self.join(name).appended(".").appended(extension)
    }

    #[inline]
    fn append<S: AsRef<OsStr>>(&self, s: S) -> PathBuf {
        self.to_path_buf().appended(s)
    }

}


/// Extension to the standard [`PathBuf`], mainly to ease joining and raw appending. In
/// this launcher we do a lot of path joining so we don't want to allocate each time.
pub trait PathBufExt {

    /// Return this path joined with another one, this is different from [`Path::join`]
    /// in that is doesn't reallocate a new path on each join.
    fn joined<P: AsRef<Path>>(self, path: P) -> Self;

    /// Return this path appended with another string, this doesn't add any path separator.
    fn appended<S: AsRef<OsStr>>(self, s: S) -> Self;

}

impl PathBufExt for PathBuf {
    
    #[inline]
    fn joined<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.push(path);
        self
    }

    #[inline]
    fn appended<S: AsRef<OsStr>>(mut self, s: S) -> Self {
        self.as_mut_os_string().push(s);
        self
    }

}
