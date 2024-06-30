//! Error type definition for standard installer.

use std::path::Path;
use std::{fmt, io};


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
#[derive(Debug, Default)]
pub enum ErrorOrigin {
    /// Unknown origin for the error.
    #[default]
    Unknown,
    /// The error is related to a specific file.
    File(Box<Path>),
    /// The origin of the error is explained in this raw message.
    Raw(Box<str>),
}

/// Kind of an uncategorized error.
#[derive(Debug)]
pub enum ErrorKind {
    Io(io::Error),
    Json(serde_json::Error),
    Schema(Box<str>),
}

// Note: methods are pub(crate) for now because I'm not sure of the API, it is designed 
// to be super practical and short to create errors but these may change in the future.

impl Error {
    
    #[inline]
    pub(crate) fn new_file_schema(origin_file: impl Into<Box<Path>>, message: impl Into<Box<str>>) -> Self {
        ErrorKind::Schema(message.into()).with_file_origin(origin_file)
    }
    
    #[inline]
    pub(crate) fn new_raw_schema(origin_raw: impl Into<Box<str>>, message: impl Into<Box<str>>) -> Self {
        ErrorKind::Schema(message.into()).with_raw_origin(origin_raw)
    }

    #[inline]
    pub(crate) fn new_schema(message: impl Into<Box<str>>) -> Self {
        ErrorKind::Schema(message.into()).without_origin()
    }

    #[inline]
    pub(crate) fn new_file_io(origin_file: impl Into<Box<Path>>, e: io::Error) -> Self {
        ErrorKind::Io(e).with_file_origin(origin_file)
    }

    #[inline]
    pub(crate) fn new_file_json(origin_file: impl Into<Box<Path>>, e: serde_json::Error) -> Self {
        ErrorKind::Json(e).with_file_origin(origin_file)
    }

    #[inline]
    pub(crate) fn map_schema<F: FnOnce(&str) -> String>(mut self, map: F) -> Self {
        
        if let Self::Other { kind: ErrorKind::Schema(ref mut schema), .. } = self {
            let new_schema = map(&schema);
            *schema = new_schema.into();
        }

        self

    }

    #[inline]
    pub(crate) fn map_origin<F: FnOnce(ErrorOrigin) -> ErrorOrigin>(mut self, map: F) -> Self {
        
        if let Self::Other { ref mut origin, .. } = self {
            *origin = map(std::mem::take(origin));
        }

        self

    }

}

impl ErrorOrigin {

    #[inline]
    pub(crate) fn new_file(file: impl Into<Box<Path>>) -> Self {
        Self::File(file.into())
    }

    #[inline]
    pub(crate) fn new_raw(raw: impl Into<Box<str>>) -> Self {
        Self::Raw(raw.into())
    }

}

impl ErrorKind {

    #[inline]
    pub(crate) fn with_file_origin(self, file: impl Into<Box<Path>>) -> Error {
        Error::Other { origin: ErrorOrigin::new_file(file), kind: self }
    }

    #[inline]
    pub(crate) fn with_raw_origin(self, raw: impl Into<Box<str>>) -> Error {
        Error::Other { origin: ErrorOrigin::new_raw(raw), kind: self }
    }

    #[inline]
    pub(crate) fn without_origin(self) -> Error {
        Error::Other { origin: ErrorOrigin::Unknown, kind: self }
    }

}

impl fmt::Display for Error {

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotSupported(s) => write!(f, "unsupported: {s}"),
            Error::Other { origin, kind } => {
                
                match origin {
                    ErrorOrigin::File(file) => write!(f, "unexpected in file {file:?}: ")?,
                    ErrorOrigin::Raw(s) => write!(f, "unexpected in {s}: ")?,
                    ErrorOrigin::Unknown => write!(f, "unexpected: ")?,
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
