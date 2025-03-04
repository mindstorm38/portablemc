//! Maven related utilities, such as GAV and 'maven-metadata.xml' parsing.

use std::path::{Path, PathBuf};
use std::iter::FusedIterator;
use std::num::NonZeroU16;
use std::str::FromStr;
use std::borrow::Cow;
use std::ops::Range;
use std::fmt;


/// A maven-style library specifier, known as GAV, for Group, Artifact, Version, but it
/// also contains an optional classifier and extension for the pointed file. The memory
/// footprint of this structure is optimized to contain only one string, its format is the
/// the following: `group:artifact:version[:classifier][@extension]`.
#[derive(Clone, PartialEq, Eq, Hash)]
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

    pub fn with_version(&self, version: &str) -> Self {
        Self::new(self.group(), self.artifact(), version, self.classifier(), self.extension())
    }

    /// Return the classifier of the library, none if no classifier.
    #[inline]
    pub fn classifier(&self) -> Option<&str> {
        let range = self.classifier_range();
        if range.is_empty() {
            None
        } else {
            Some(&self.raw[self.classifier_range()])
        }
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

    pub fn with_classifier(&self, classifier: Option<&str>) -> Self {
        Self::new(self.group(), self.artifact(), self.version(), classifier, self.extension())
    }

    /// Return the extension of the library, none if the default extension should be used,
    /// like "jar".
    #[inline]
    pub fn extension(&self) -> Option<&str> {
        let range = self.extension_range();
        if range.is_empty() {
            None
        } else {
            Some(&self.raw[range])
        }
    }

    /// Return the extension of the library, "jar" if default extension should be used.
    #[inline]
    pub fn extension_or_default(&self) -> &str {
        self.extension().unwrap_or("jar")
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

    /// Get the representation of the GAV as a string.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Get a URL formatter for this GAV, this can be appended to any full URL.
    /// 
    /// For example, 
    /// `net.minecraft:client:1.21.1` will transform into 
    /// `net/minecraft/client/1.21.1/client-1.21.1.jar`.
    #[inline]
    pub fn url(&self) -> GavUrl<'_> {
        GavUrl(self)
    }

    /// Create a file path of this GAV from a base directory.
    /// 
    /// NOTE: Unsafe path joining if any component as a '..'!
    pub fn file<P: AsRef<Path>>(&self, dir: P) -> PathBuf {

        let mut buf = dir.as_ref().to_path_buf();
        for group_part in self.group().split('.') {
            buf.push(group_part);
        }

        let artifact = self.artifact();
        let version = self.version();
        buf.push(artifact);
        buf.push(version);

        // Build the terminal file name.
        buf.push(artifact);
        buf.as_mut_os_string().push("-");
        buf.as_mut_os_string().push(version);
        if let Some(classifier) = self.classifier() {
            buf.as_mut_os_string().push("-");
            buf.as_mut_os_string().push(classifier);
        }
        buf.as_mut_os_string().push(".");
        buf.as_mut_os_string().push(self.extension_or_default());

        buf

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
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Gav {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Gav").field(&self.raw).finish()
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

impl serde::Serialize for Gav {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        serializer.serialize_str(self.as_str())
    }
}

/// URL formatter for a gav.
pub struct GavUrl<'a>(&'a Gav);

impl fmt::Display for GavUrl<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        for group_part in self.0.group().split('.') {
            f.write_str(group_part)?;
            f.write_str("/")?;
        }
        
        let artifact = self.0.artifact();
        let version = self.0.version();

        f.write_str(artifact)?;
        f.write_str("/")?;
        f.write_str(version)?;
        f.write_str("/")?;
        f.write_str(artifact)?;
        f.write_str("-")?;
        f.write_str(version)?;

        if let Some(classifier) = self.0.classifier() {
            f.write_str("-")?;
            f.write_str(classifier)?;
        }

        f.write_str(".")?;
        f.write_str(self.0.extension_or_default())

    }
}

/// A streaming parser for a 'maven-metadata.xml' file, this is an iterator that return
/// each versions.
#[derive(Debug)]
pub(crate) struct MetadataParser<'a> {
    tokenizer: Option<xmlparser::Tokenizer<'a>>,
}

impl<'a> MetadataParser<'a> {

    pub fn new(buffer: &'a str) -> Self {
        Self {
            tokenizer: Some(xmlparser::Tokenizer::from(buffer)),
        }
    }

}

impl<'a> Iterator for MetadataParser<'a> {

    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {

        use xmlparser::{Token, ElementEnd};

        let tokenizer = self.tokenizer.as_mut()?;
        let mut version = false;

        while let Ok(token) = tokenizer.next()? {

            match token {
                Token::ElementStart { prefix, local, .. } => {
                    if prefix.is_empty() && local == "version" {
                        if !version {
                            version = true;
                        } else {
                            break;  // return none
                        }
                    } else if version {
                        break;  // return none
                    }
                }
                Token::ElementEnd { end: ElementEnd::Close(prefix, local), .. } => {
                    if version {
                        if prefix.is_empty() && local == "version" {
                            version = false;
                        } else {
                            break;  // return none
                        }
                    }
                }
                Token::ElementEnd { end: ElementEnd::Empty, .. } => {
                    if version {
                        break;  // return none
                    }
                }
                Token::Text { text } => {
                    if version {
                        return Some(text.as_str());
                    }
                }
                _ => continue,
            }

        }

        // Tokenizer doesn't implement FusedIterator yet so we nullify if none's returned.
        // If any error we nullify tokenizer so we always return none.
        self.tokenizer = None;
        None

    }
    
}

/// Valid to implement because we return none forever after any error.
impl FusedIterator for MetadataParser<'_> {  }

#[cfg(test)]
mod tests {

    use std::str::FromStr;
    use super::Gav;

    #[test]
    #[should_panic]
    fn empty_group() {
        Gav::new("", "baz", "0.1.2-beta", None, None);
    }

    #[test]
    #[should_panic]
    fn empty_artifact() {
        Gav::new("foo.bar", "", "0.1.2-beta", None, None);
    }

    #[test]
    #[should_panic]
    fn empty_version() {
        Gav::new("foo.bar", "baz", "", None, None);
    }

    #[test]
    #[should_panic]
    fn empty_classifier() {
        Gav::new("foo.bar", "baz", "0.1.2-beta", Some(""), None);
    }

    #[test]
    #[should_panic]
    fn empty_extension() {
        Gav::new("foo.bar", "baz", "0.1.2-beta", None, Some(""));
    }

    #[test]
    fn as_str_correct() {
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", None, None).as_str(), "foo.bar:baz:0.1.2-beta");
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", Some("natives"), None).as_str(), "foo.bar:baz:0.1.2-beta:natives");
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", None, Some("jar")).as_str(), "foo.bar:baz:0.1.2-beta@jar");
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", Some("natives"), Some("jar")).as_str(), "foo.bar:baz:0.1.2-beta:natives@jar");
    }

    #[test]
    fn from_str_correct() {

        const WRONG_CASES: &'static [&'static str] = &[
            "", ":", "::",
            "foo.bar::", ":baz:", "::0.1.2-beta",
            "foo.bar:baz:", "foo.bar::0.1.2-beta", ":baz:0.1.2-beta",
            "foo.bar:baz:0.1.2-beta:",
            "foo.bar:baz:0.1.2-beta@",
        ];

        for case in WRONG_CASES {
            assert_eq!(Gav::from_str(case), Err(()));
        }

        let gav = Gav::from_str("foo.bar:baz:0.1.2-beta").unwrap();
        assert_eq!(gav.group(), "foo.bar");
        assert_eq!(gav.artifact(), "baz");
        assert_eq!(gav.version(), "0.1.2-beta");
        assert_eq!(gav.classifier(), None);
        assert_eq!(gav.extension(), None);

        let gav = Gav::from_str("foo.bar:baz:0.1.2-beta:natives@txt").unwrap();
        assert_eq!(gav.group(), "foo.bar");
        assert_eq!(gav.artifact(), "baz");
        assert_eq!(gav.version(), "0.1.2-beta");
        assert_eq!(gav.classifier(), Some("natives"));
        assert_eq!(gav.extension(), Some("txt"));

    }

}
