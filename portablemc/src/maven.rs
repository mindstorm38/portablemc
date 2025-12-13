//! Maven related utilities, such as GAV and 'maven-metadata.xml' parsing.

use std::cmp::Ordering;
use std::iter::FusedIterator;
use std::path::PathBuf;
use std::num::NonZero;
use std::str::FromStr;
use std::borrow::Cow;
use std::ops::Range;
use std::{fmt, hash};


/// A maven-style library specifier, known as GAV, for Group, Artifact, Version, but it
/// also contains an optional classifier and extension for the pointed file. The memory
/// footprint of this structure is optimized to contain only one string, its format is the
/// the following: `group:artifact:version[:classifier][@extension]`. This structure
/// ensures that all the components have valid characters.
#[derive(Clone)]
pub struct Gav {
    /// Internal buffer, canonicalized without @jar.
    raw: Box<str>,
    /// Length of the group part in the specifier.
    group_len: NonZero<u16>,
    /// Length of the artifact part in the specifier.
    artifact_len: NonZero<u16>,
    /// Length of the version part in the specifier.
    version_len: NonZero<u16>,
    /// Length of the classifier part in the specifier, if relevant.
    classifier_len: Option<NonZero<u16>>,
    /// Length of the extension part in the specifier, if relevant.
    extension_len: Option<NonZero<u16>>,
}

impl Gav {

    /// Create a new library specifier with the given components.
    /// Each component, if given, should not be empty.
    pub fn new(group: &str, artifact: &str, version: &str, classifier: Option<&str>, extension: Option<&str>) -> Option<Self> {
        
        let mut raw = format!("{group}:{artifact}:{version}");
        
        let mut classifier_len = None;
        if let Some(classifier) = classifier {
            raw.push(':');
            raw.push_str(classifier);
            classifier_len = Some(NonZero::new(classifier.len() as _)?);
        }

        let mut extension_len = None;
        if let Some(extension) = extension && extension != "jar" {
            raw.push('@');
            raw.push_str(extension);
            extension_len = Some(NonZero::new(extension.len() as _)?);
        }

        // Read below, we ensure that every part fits in u16.
        if raw.len() > u16::MAX as usize {
            return None;
        }

        check_gav_chars(&raw)?;

        Some(Self {
            raw: raw.into_boxed_str(),
            group_len: NonZero::new(group.len() as _)?,
            artifact_len: NonZero::new(artifact.len() as _)?,
            version_len: NonZero::new(version.len() as _)?,
            classifier_len,
            extension_len,
        })

    }

    /// Internal method to parse 
    fn _from_str(raw: Cow<str>) -> Option<Self> {

        // Early check that raw string is not longer than u16 max because we cast using 
        // 'as' and we don't want the cast to overflow, checking the size of the full 
        // string is a guarantee that any of its piece will be less than u16 max long.
        if raw.len() > u16::MAX as usize {
            return None;
        }

        check_gav_chars(&raw)?;

        let mut split = raw.split('@');
        let raw0 = split.next()?;
        let (extension_len, strip_jar) = match split.next() {
            Some(s) if s == "jar" => (None, true),
            Some(s) => (Some(NonZero::new(s.len() as _)?), false),
            None => (None, false),
        };

        if split.next().is_some() {
            return None;
        }

        let mut split = raw0.split(':');
        let group_len = NonZero::new(split.next()?.len() as _)?;
        let artifact_len = NonZero::new(split.next()?.len() as _)?;
        let version_len = NonZero::new(split.next()?.len() as _)?;
        let classifier_len = match split.next() {
            Some(s) => Some(NonZero::new(s.len() as _)?),
            None => None,
        };

        if split.next().is_some() {
            return None;
        }

        let mut raw = raw.into_owned();
        if strip_jar {
            raw.truncate(raw.len() - "@jar".len());
        }

        Some(Self {
            raw: raw.into_boxed_str(),
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

    #[inline]
    pub fn with_group(&self, group: &str) -> Option<Self> {
        Self::new(group, self.artifact(), self.version(), self.classifier(), Some(self.extension()))
    }

    /// Return the artifact name of the library, never empty.
    #[inline]
    pub fn artifact(&self) -> &str {
        &self.raw[self.artifact_range()]
    }

    #[inline]
    pub fn with_artifact(&self, artifact: &str) -> Option<Self> {
        Self::new(self.group(), artifact, self.version(), self.classifier(), Some(self.extension()))
    }

    /// Return the version of the library, never empty.
    #[inline]
    pub fn version(&self) -> &str {
        &self.raw[self.version_range()]
    }

    #[inline]
    pub fn with_version(&self, version: &str) -> Option<Self> {
        Self::new(self.group(), self.artifact(), version, self.classifier(), Some(self.extension()))
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

    #[inline]
    pub fn with_classifier(&self, classifier: Option<&str>) -> Option<Self> {
        Self::new(self.group(), self.artifact(), self.version(), classifier, Some(self.extension()))
    }

    /// Return the extension of the library, "jar" if the default extension should 
    /// be used.
    #[inline]
    pub fn extension(&self) -> &str {
        let range = self.extension_range();
        if range.is_empty() {
            "jar"
        } else {
            &self.raw[range]
        }
    }

    #[inline]
    pub fn with_extension(&self, extension: Option<&str>) -> Option<Self> {
        Self::new(self.group(), self.artifact(), self.version(), self.classifier(), extension)
    }

    /// Get the representation of the GAV as a string, this form is always canonicalized
    /// which means that the @jar extension is never explicitly written.
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

    /// Create a file path of this GAV from a base directory. This may produce a path 
    /// that is insecure to join due to absolute or parent relative joining.
    /// 
    /// If the return path contains invalid component, such as relative or root 
    /// components, None is returned to enforce safety.
    pub fn file(&self) -> PathBuf {

        let len = 
            self.group_len.get() as usize + 1 + 
            self.artifact_len.get() as usize + 1 + 
            self.version_len.get() as usize + 1 +
            self.artifact_len.get() as usize + 1 +
            self.version_len.get() as usize +
            self.classifier_len.map(|len| 1 + len.get() as usize).unwrap_or(0) + 1 +
            self.extension().len();
        
        let mut buf = PathBuf::with_capacity(len);

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
        buf.as_mut_os_string().push(self.extension());

        debug_assert_eq!(buf.as_os_str().len(), len);

        buf

    }

}

impl FromStr for Gav {
    
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::_from_str(Cow::Borrowed(s)).ok_or(())
    }

}

impl PartialEq for Gav {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Gav { }

impl PartialOrd for Gav {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Gav {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl hash::Hash for Gav {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
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
        f.write_str(self.0.extension())

    }
}

/// That function validates that the GAV characters are safe for later use by the program,
/// such as joining and making URLs. We only accept ASCII, alphanumeric, '-_+' and '.' if
/// no '..' pattern is found. Note that ':' is allowed to simplify the caller, but the 
/// caller (builder or parser) should ensure that all sections are correct and that there
/// is a correct number of ':'. We also accepts '*' in order to allow the CLI to use this
/// as a wildcard for pattern matching libraries.
fn check_gav_chars(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    (0..bytes.len())
        .all(|i| 
            bytes[i].is_ascii_alphanumeric() || 
            matches!(bytes[i], b'-' | b'_' | b'+' | b':' | b'@' | b'*') || 
            (bytes[i] == b'.' && (i == 0 || bytes[i - 1] != b'.')))
        .then_some(s)
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
    fn empty_parts() {
        assert!(Gav::new("", "baz", "0.1.2-beta", None, None).is_none());
        assert!(Gav::new("foo.bar", "", "0.1.2-beta", None, None).is_none());
        assert!(Gav::new("foo.bar", "baz", "", None, None).is_none());
        assert!(Gav::new("foo.bar", "baz", "0.1.2-beta", Some(""), None).is_none());
        assert!(Gav::new("foo.bar", "baz", "0.1.2-beta", None, Some("")).is_none());
    }

    #[test]
    fn invalid_chars() {
        assert!(Gav::new("foo..bar", "baz", "0.1.2-beta", None, None).is_none());
        assert!(Gav::new("foo.bar", "/baz", "0.1.2-beta", None, None).is_none());
        assert!(Gav::new("foo.bar", "baz", "!0.1.2-beta", None, None).is_none());
    }

    #[test]
    fn file_debug_assert() {
        Gav::new("foo.bar", "baz", "0.1.2-beta", None, None).unwrap().file();
        Gav::new("foo.bar", "baz", "0.1.2-beta", Some("natives"), None).unwrap().file();
        Gav::new("foo.bar", "baz", "0.1.2-beta", None, Some("jar")).unwrap().file();
        Gav::new("foo.bar", "baz", "0.1.2-beta", Some("natives"), Some("jar")).unwrap().file();
    }

    #[test]
    fn as_str_correct() {
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", None, None).unwrap().as_str(), "foo.bar:baz:0.1.2-beta");
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", Some("natives"), None).unwrap().as_str(), "foo.bar:baz:0.1.2-beta:natives");
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", None, Some("jar")).unwrap().as_str(), "foo.bar:baz:0.1.2-beta");
        assert_eq!(Gav::new("foo.bar", "baz", "0.1.2-beta", Some("natives"), Some("jar")).unwrap().as_str(), "foo.bar:baz:0.1.2-beta:natives");
        assert_eq!(Gav::new("foo.bar_ok", "baz", "0.1.2+beta", None, None).unwrap().as_str(), "foo.bar_ok:baz:0.1.2+beta");
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
        assert_eq!(gav.extension(), "jar");

        let gav = Gav::from_str("foo.bar:baz:0.1.2-beta:natives@txt").unwrap();
        assert_eq!(gav.group(), "foo.bar");
        assert_eq!(gav.artifact(), "baz");
        assert_eq!(gav.version(), "0.1.2-beta");
        assert_eq!(gav.classifier(), Some("natives"));
        assert_eq!(gav.extension(), "txt");

    }

    #[test]
    fn modify() {

        let gav = Gav::from_str("foo.bar:baz:0.1.2-beta").unwrap();
        assert_eq!(gav.with_group("foo1.bar2").unwrap().as_str(), "foo1.bar2:baz:0.1.2-beta");
        assert_eq!(gav.with_artifact("baz1").unwrap().as_str(), "foo.bar:baz1:0.1.2-beta");
        assert_eq!(gav.with_version("0.1.3-alpha").unwrap().as_str(), "foo.bar:baz:0.1.3-alpha");
        assert_eq!(gav.with_classifier(Some("natives")).unwrap().as_str(), "foo.bar:baz:0.1.2-beta:natives");
        assert_eq!(gav.with_extension(Some("txt")).unwrap().as_str(), "foo.bar:baz:0.1.2-beta@txt");

    }

    #[test]
    fn canonicalized() {
        assert_eq!(Gav::from_str("foo.bar:baz:0.1.2-beta").unwrap(), Gav::from_str("foo.bar:baz:0.1.2-beta@jar").unwrap());
    }

}
