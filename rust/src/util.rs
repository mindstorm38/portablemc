//! Various uncategorized utilities.

use std::path::{Path, PathBuf};
use std::io::{self, Read, Write};
use std::ffi::OsStr;

use digest::{Digest, Output};


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


/// A reader wrapper that will compute the SHA-1 of the underlying reader as it is read.
/// If the underlying read has not been fully read, then it can be consumed to the end
/// with the method [`finalize`].
/// This avoids reading the reader twice.
pub struct DigestReader<R, D> {
    inner: R,
    digest: D,
}

impl<R: Read, D: Digest> Read for DigestReader<R, D> {

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.inner.read(buf)?;
        self.digest.update(&buf[..len]);
        Ok(len)
    }
    
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf)?;
        self.digest.update(buf);
        Ok(())
    }

}

impl<R, D> DigestReader<R, D> {

    #[inline]
    pub fn new(inner: R, digest: D) -> Self {
        Self {
            inner,
            digest,
        }
    }

}

impl<R: Read, D: Digest + Write> DigestReader<R, D> {

    /// Finalize the hashing by consuming the remaining part of the underlying reader.
    pub fn finalize(mut self) -> io::Result<Output<D>> {
        io::copy(&mut self.inner, &mut self.digest)?;
        Ok(self.digest.finalize())
    }

    /// Destroy this wrapper and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner
    }

}
