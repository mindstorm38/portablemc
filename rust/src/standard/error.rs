//! Error type definition for standard installer.

use std::{fmt, io};
use std::path::Path;


/// Type alias for result with the install error type.
pub type Result<T> = std::result::Result<T, Error>;

/// The standard installer could not proceed to the installation of a version.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Some operation that needed to run is unsupported, the value usually contains a
    /// method path (from source code). This is used to avoid panicking if a function
    /// in a handler is unsupported and the next handler should manage the call.
    NotSupported(&'static str),
    /// The given version is not found when trying to fetch it.
    VersionNotFound(Box<str>),
    /// The version JAR file that is required has no download information and is not 
    /// already existing, is is mandatory to build the class path.
    JarNotFound(),
    /// A developer-oriented error that cannot be handled with other errors, it has an
    /// origin that could be a file or any other raw string, attached to the actual error.
    /// This includes filesystem, network, JSON parsing and schema errors.
    Other {
        /// The origin of the error, can be a file path.
        origin: ErrorOrigin,
        /// The error error kind from the origin.
        kind: ErrorKind,
    },
}

/// Origin of an uncategorized error.
#[derive(Debug)]
pub enum ErrorOrigin {
    File(Box<Path>),
    Raw(Box<str>),
}

/// Kind of an uncategorized error.
#[derive(Debug)]
pub enum ErrorKind {
    Io(io::Error),
    Json(serde_json::Error),
    Schema(Box<str>),
}

impl Error {
    
    #[inline]
    pub fn new_file_schema(origin_file: impl Into<Box<Path>>, message: impl Into<Box<str>>) -> Self {
        ErrorKind::Schema(message.into()).with_file_origin(origin_file)
    }
    
    #[inline]
    pub fn new_raw_schema(origin_raw: impl Into<Box<str>>, message: impl Into<Box<str>>) -> Self {
        ErrorKind::Schema(message.into()).with_raw_origin(origin_raw)
    }

    #[inline]
    pub fn new_file_io(origin_file: impl Into<Box<Path>>, e: io::Error) -> Self {
        ErrorKind::Io(e).with_file_origin(origin_file)
    }

    #[inline]
    pub fn new_file_json(origin_file: impl Into<Box<Path>>, e: serde_json::Error) -> Self {
        ErrorKind::Json(e).with_file_origin(origin_file)
    }

}

impl ErrorKind {

    #[inline]
    pub fn with_file_origin(self, file: impl Into<Box<Path>>) -> Error {
        Error::Other { origin: ErrorOrigin::File(file.into()), kind: self }
    }

    #[inline]
    pub fn with_raw_origin(self, raw: impl Into<Box<str>>) -> Error {
        Error::Other { origin: ErrorOrigin::Raw(raw.into()), kind: self }
    }

}

impl fmt::Display for Error {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotSupported(s) => write!(f, "unsupported: {s}"),
            Error::Other { origin, kind } => {
                
                write!(f, "unexpected in ")?;
                match origin {
                    ErrorOrigin::File(file) => write!(f, "file {file:?}: ")?,
                    ErrorOrigin::Raw(s) => write!(f, "{s}: ")?,
                }

                match kind {
                    ErrorKind::Io(e) => e.fmt(f),
                    ErrorKind::Json(e) => e.fmt(f),
                    ErrorKind::Schema(e) => e.fmt(f),
                }

            }
            Error::VersionNotFound(s) => write!(f, "version not found: {s}"),
            Error::JarNotFound() => write!(f, "jar not found"),
        }
    }

}

impl std::error::Error for Error {

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other { kind, .. } => {
                match kind {
                    ErrorKind::Io(e) => Some(e),
                    ErrorKind::Json(e) => Some(e),
                    _ => None
                }
            }
            _ => None
        }
    }

}
