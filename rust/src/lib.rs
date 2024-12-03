use std::collections::HashMap;
use std::num::NonZeroU16;


/// Handlers can be added to a version to alter the game while resolving it.
pub trait Handler {

    /// Filter features that will be used to resolve metadata libraries and arguments.
    fn filter_features(&self, features: &mut HashMap<String, String>) {
        let _ = features;
    }

    /// Filter libraries after initial resolution.
    fn filter_libraries(&self, libraries: &mut HashMap<LibrarySpecifier, Library>) {
        let _ = libraries;
    }

}

pub struct Library {

}

/// A maven-style library specifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LibrarySpecifier {
    /// Internal buffer containing the whole specifier. This should follows the pattern 
    /// `group:artifact:version[:classifier][@extension]`.
    raw: String,
    group_len: NonZeroU16,
    artifact_len: NonZeroU16,
    version_len: NonZeroU16,
    classifier_len: Option<NonZeroU16>,
    extension_len: Option<NonZeroU16>,
}

impl LibrarySpecifier {

    /// Parse the given library specifier and return it if successful.
    pub fn new(raw: String) -> Option<Self> {

        let mut split = raw.split('@');
        let raw0 = split.next()?;
        let extension_len = match split.next() {
            Some(s) => Some(NonZeroU16::new(s.len() as _)?),
            None => None,
        };

        if split.next().is_some() {
            return None;
        }

        let mut split = raw0.split(':');
        let group_len = NonZeroU16::new(split.next()?.len() as _)?;
        let artifact_len = NonZeroU16::new(split.next()?.len() as _)?;
        let version_len = NonZeroU16::new(split.next()?.len() as _)?;
        let classifier_len = match split.next() {
            Some(s) => Some(NonZeroU16::new(s.len() as _)?),
            None => None,
        };

        if split.next().is_some() {
            return None;
        }

        Some(Self {
            raw,
            group_len,
            artifact_len,
            version_len,
            classifier_len,
            extension_len,
        })

    }

    #[inline]
    fn split(&self) -> (&str, &str, &str, &str, &str) {
        let (group, rem) = self.raw.split_at(self.group_len.get() as usize);
        let (artifact, rem) = rem[1..].split_at(self.artifact_len.get() as usize);
        let (version, rem) = rem[1..].split_at(self.version_len.get() as usize);
        let (classifier, rem) = self.classifier_len.map(|len| rem[1..].split_at(len.get() as usize)).unwrap_or(("", rem));
        let (extension, rem) = self.extension_len.map(|len| rem[1..].split_at(len.get() as usize)).unwrap_or(("jar", rem));
        debug_assert!(rem.is_empty());
        (group, artifact, version, classifier, extension)
    }

    #[inline]
    pub fn group(&self) -> &str {
        self.split().0
    }

    #[inline]
    pub fn artifact(&self) -> &str {
        self.split().1
    }

    #[inline]
    pub fn version(&self) -> &str {
        self.split().2
    }

    #[inline]
    pub fn classifier(&self) -> &str {
        self.split().3
    }

    #[inline]
    pub fn extension(&self) -> &str {
        self.split().4
    }

}
