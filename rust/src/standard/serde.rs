//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;

use regex::Regex;

use super::specifier::LibrarySpecifier;


/// A version metadata JSON schema.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// The version id, should be the same as the directory the metadata is in.
    pub id: String,
    /// The version type, such as 'release' or 'snapshot'.
    pub r#type: String,
    /// The last time this version has been updated.
    pub time: String,
    /// The first release time of this version.
    pub release_time: String,
    /// If present, this is the name of another version to resolve after this one and
    /// where fallback values will be taken.
    pub inherits_from: Option<String>,
    /// Used by official launcher, optional.
    pub minimum_launcher_version: Option<u32>,
    /// Describe the Java version to use, optional.
    pub java_version: Option<VersionJavaVersion>,
    /// The asset index to use when launching the game, it also has download information.
    pub asset_index: Option<VersionAssetIndex>,
    /// Legacy asset index id without download information.
    pub assets: Option<String>,
    /// Unknown, used by official launcher.
    pub compliance_level: Option<u32>,
    #[serde(default)]
    pub downloads: HashMap<String, Download>,
    #[serde(default)]
    pub libraries: Vec<VersionLibrary>,
    pub main_class: String,
    /// Legacy arguments command line.
    #[serde(rename = "minecraftArguments")]
    pub legacy_arguments: String,
    pub arguments: Option<VersionArguments>,
}

/// Object describing the Mojang-provided Java version to use to launch the game.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionAssetIndex {
    pub id: String,
    pub total_size: u32,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibrary {
    pub name: LibrarySpecifier,
    #[serde(default)]
    pub downloads: VersionLibraryDownloads,
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownloads {
    pub artifact: Option<VersionLibraryDownload>,
    #[serde(default)]
    pub classifiers: HashMap<String, VersionLibraryDownload>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownload {
    pub path: Option<String>,
    #[serde(flatten)]
    pub common: Download,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionArguments {
    #[serde(flatten)]
    pub game: Vec<VersionArgument>,
    #[serde(flatten)]
    pub jvm: Vec<VersionArgument>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum VersionArgument {
    Raw(String),
    Conditional(VersionConditionalArgument),
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionConditionalArgument {
    pub value: SingleOrVec<String>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: RuleOs,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOs {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<RegexString>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, serde::Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: Option<Sha1HashString>,
    pub size: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
    Single(T),
    Vec(Vec<T>)
}

/// A SHA-1 hash parsed has a 40 hex characters string.
#[derive(Debug)]
pub struct Sha1HashString(pub [u8; 20]);

impl<'de> serde::Deserialize<'de> for Sha1HashString {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {

        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {

            type Value = Sha1HashString;
        
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a string sha-1 hash (40 hex characters)")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                parse_hex_bytes::<20>(v)
                    .map(Sha1HashString)
                    .ok_or_else(|| E::custom("invalid sha-1 hash (40 hex characters)"))
            }

        }

        deserializer.deserialize_str(Visitor)

    }

}

/// A Regex parsed from the string it is defined in.
#[derive(Debug)]
pub struct RegexString(pub Regex);

impl<'de> serde::Deserialize<'de> for RegexString {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {

            type Value = RegexString;
        
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a string regex")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error, 
            {
                Regex::new(v)
                    .map(RegexString)
                    .map_err(|e| E::custom(e))
            }

        }

        deserializer.deserialize_str(Visitor)

    }

}


/// Parse the given hex bytes string into the given destination slice, returning none if 
/// the input string cannot be parsed, is too short or too long.
fn parse_hex_bytes<const LEN: usize>(mut string: &str) -> Option<[u8; LEN]> {
    
    let mut dst = [0; LEN];
    for dst in &mut dst {
        if string.is_char_boundary(2) {

            let (num, rem) = string.split_at(2);
            string = rem;

            *dst = u8::from_str_radix(num, 16).ok()?;

        } else {
            return None;
        }
    }

    // Only successful if no string remains.
    string.is_empty().then_some(dst)

}
