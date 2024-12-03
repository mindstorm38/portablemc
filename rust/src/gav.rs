//! Definition of a library specifier, known has GAV (Group, Artifact, Version).

use std::num::NonZeroU16;
use std::str::FromStr;
use std::borrow::Cow;
use std::ops::Range;
use std::fmt;


/// A maven-style library specifier, known as GAV, for Group, Artifact, Version, but it
/// also contains an optional classifier and extension for the pointed file. The memory
/// footprint of this structure is optimized to contain only one string, its format is
/// following: `group:artifact:version[:classifier][@extension]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Gav {
    /// Internal buffer.
    raw: String,
    /// Length of the group part in the specifier.
    group_len: NonZeroU16,
    /// Length of the artifact part in the specifier.
    artifact_len: NonZeroU16,
    /// Length of the version part in the specifier.
    version_len: NonZeroU16,
    /// Length of the classifier part in the specifier, if relevant.
    classifier_len: Option<NonZeroU16>,
    /// Length of the extension part in the specifier, if relevant.
    extension_len: Option<NonZeroU16>,
}

impl Gav {

    /// Create a new library specifier with the given components.
    /// Each component, if given, should not be empty.
    pub fn new(group: &str, artifact: &str, version: &str, classifier: Option<&str>, extension: Option<&str>) -> Self {
        
        let mut raw = format!("{group}:{artifact}:{version}");
        
        if let Some(classifier) = classifier {
            raw.push(':');
            raw.push_str(classifier);
        }

        if let Some(extension) = extension {
            raw.push('@');
            raw.push_str(extension);
        }

        Self {
            raw,
            group_len: NonZeroU16::new(group.len().try_into().expect("group too long")).expect("group empty"),
            artifact_len: NonZeroU16::new(artifact.len().try_into().expect("artifact too long")).expect("artifact empty"),
            version_len: NonZeroU16::new(version.len().try_into().expect("version too long")).expect("version empty"),
            classifier_len: classifier.map(|classifier| NonZeroU16::new(classifier.len().try_into().expect("classifier too long")).expect("classifier empty")),
            extension_len: extension.map(|extension| NonZeroU16::new(extension.len().try_into().expect("extension too long")).expect("extension empty")),
        }

    }

    /// Internal method to parse 
    fn _from_str(raw: Cow<str>) -> Option<Self> {

        // Early check that raw string is not longer than u16 max because we cast using 
        // 'as' and we don't want the cast to overflow, checking the size of the full 
        // string is a guarantee that any of its piece will be less than u16 max long.
        if raw.len() > u16::MAX as usize {
            return None;
        }

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
            raw: raw.into_owned(),
            group_len,
            artifact_len,
            version_len,
            classifier_len,
            extension_len,
        })

    }

    #[inline]
    fn group_range(&self) -> Range<usize> {
        0..self.group_len.get() as usize
    }

    #[inline]
    fn artifact_range(&self) -> Range<usize> {
        let prev = self.group_range();
        prev.end + 1..prev.end + 1 + self.artifact_len.get() as usize
    }

    #[inline]
    fn version_range(&self) -> Range<usize> {
        let prev = self.artifact_range();
        prev.end + 1..prev.end + 1 + self.version_len.get() as usize
    }

    #[inline]
    fn classifier_range(&self) -> Range<usize> {
        let prev = self.version_range();
        match self.classifier_len {
            Some(classifier_len) => prev.end + 1..prev.end + 1 + classifier_len.get() as usize,
            None => prev.end..prev.end
        }
    }

    #[inline]
    fn extension_range(&self) -> Range<usize> {
        let prev = self.classifier_range();
        match self.extension_len {
            Some(extension_len) => prev.end + 1..prev.end + 1 + extension_len.get() as usize,
            None => prev.end..prev.end
        }
    }

    /// Return the group name of the library, never empty.
    #[inline]
    pub fn group(&self) -> &str {
        &self.raw[self.group_range()]
    }

    /// Change the group of the library, should not be empty.
    pub fn set_group(&mut self, group: &str) {
        let range = self.group_range();
        self.group_len = NonZeroU16::new(group.len().try_into().expect("group too long")).expect("group empty");
        self.raw.replace_range(range, group);
    }

    /// Return the artifact name of the library, never empty.
    #[inline]
    pub fn artifact(&self) -> &str {
        &self.raw[self.artifact_range()]
    }

    /// Change the artifact of the library, should not be empty.
    pub fn set_artifact(&mut self, artifact: &str) {
        let range = self.artifact_range();
        self.artifact_len = NonZeroU16::new(artifact.len().try_into().expect("artifact too long")).expect("artifact empty");
        self.raw.replace_range(range, artifact);
    }

    /// Return the version of the library, never empty.
    #[inline]
    pub fn version(&self) -> &str {
        &self.raw[self.version_range()]
    }

    /// Change the version of the library, should not be empty.
    pub fn set_version(&mut self, version: &str) {
        let range = self.version_range();
        self.version_len = NonZeroU16::new(version.len().try_into().expect("version too long")).expect("version empty");
        self.raw.replace_range(range, version);
    }

    /// Return the classifier of the library, empty if no classifier.
    #[inline]
    pub fn classifier(&self) -> &str {
        &self.raw[self.classifier_range()]
    }

    /// Change the classifier of the library, should not be empty.
    pub fn set_classifier(&mut self, classifier: Option<&str>) {
        let range = self.classifier_range();
        if let Some(classifier) = classifier {
            self.classifier_len = Some(NonZeroU16::new(classifier.len().try_into().expect("classifier too long")).expect("classifier empty"));
            self.raw.replace_range(range.clone(), classifier);
            if range.is_empty() {
                self.raw.insert(range.start, ':');
            }
        } else if !range.is_empty() {
            self.classifier_len = None;
            self.raw.replace_range(range, "");
        }
    }

    /// Return the extension of the library, never empty, defaults to "jar".
    #[inline]
    pub fn extension(&self) -> &str {
        let range = self.extension_range();
        if range.is_empty() {
            "jar"
        } else {
            &self.raw[range]
        }
    }

    /// Change the extension of the library, should not be empty.
    pub fn set_extension(&mut self, extension: Option<&str>) {
        let range = self.extension_range();
        if let Some(extension) = extension {
            self.extension_len = Some(NonZeroU16::new(extension.len().try_into().expect("extension too long")).expect("extension empty"));
            self.raw.replace_range(range.clone(), extension);
            if range.is_empty() {
                self.raw.insert(range.start, ':');
            }
        } else if !range.is_empty() {
            self.extension_len = None;
            self.raw.replace_range(range, "");
        }
    }

    /// Iterator over standard file path component for this GAV, the iterating
    /// component is a cow because most of these are borrowed but the last 
    /// file part must be formatted and therefore owned.
    pub fn file_components(&self) -> impl Iterator<Item = Cow<'_, str>> + '_ {

        let artifact = self.artifact();
        let version = self.version();

        let mut file_name = format!("{artifact}-{version}");
        let classifier = self.classifier();
        if !classifier.is_empty() {
            file_name.push('-');
            file_name.push_str(classifier);
        }
        file_name.push_str(self.extension());

        self.group().split('.')
            .chain([artifact, version])
            .map(Cow::Borrowed)
            .chain([Cow::Owned(file_name)])

    }

}

impl FromStr for Gav {
    
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::_from_str(Cow::Borrowed(s)).ok_or(())
    }

}

impl fmt::Display for Gav {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl<'de> serde::Deserialize<'de> for Gav {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {

        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            
            type Value = Gav;
        
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a string gav (group:artifact:version[:classifier][@extension])")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                Gav::_from_str(Cow::Owned(v))
                    .ok_or_else(|| E::custom("invalid string gav (group:artifact:version[:classifier][@extension])"))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                Gav::_from_str(Cow::Borrowed(v))
                    .ok_or_else(|| E::custom("invalid string gav (group:artifact:version[:classifier][@extension])"))
            }

        }

        deserializer.deserialize_string(Visitor)

    }

}
