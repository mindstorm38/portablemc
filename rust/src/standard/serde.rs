//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;
use std::ops::Deref;

use regex::Regex;

use crate::gav::Gav;


/// A version metadata JSON schema.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
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
    /// A mapping of downloads for entry point JAR files, such as for client or for 
    /// server. This sometime also defines a server executable for old versions.
    #[serde(default)]
    pub downloads: HashMap<String, Download>,
    /// The sequence of JAR libraries to include in the class path when running the
    /// version, the order of libraries should be respected in the class path (for
    /// some corner cases with mod loaders). When a library is defined, it can't be
    /// overridden by inherited versions.
    #[serde(default)]
    pub libraries: Vec<VersionLibrary>,
    /// The full class name to run as the main JVM class.
    pub main_class: String,
    /// Legacy arguments command line.
    #[serde(rename = "minecraftArguments")]
    pub legacy_arguments: Option<String>,
    /// Modern arguments for game and/or jvm.
    pub arguments: Option<VersionArguments>,
    /// Logging configuration.
    #[serde(default)]
    pub logging: HashMap<String, VersionLogging>,
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

/// Object describing the Mojang-provided Java version to use to launch the game.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionJavaVersion {
    pub component: String,
    pub major_version: u32,
}

/// Describe the asset index to use and how to download it when missing.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionAssetIndex {
    pub id: String,
    pub total_size: u32,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibrary {
    pub name: Gav,
    #[serde(default)]
    pub downloads: VersionLibraryDownloads,
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rule>>,
    pub url: Option<String>,
}

#[derive(serde::Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownloads {
    pub artifact: Option<VersionLibraryDownload>,
    #[serde(default)]
    pub classifiers: HashMap<String, VersionLibraryDownload>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownload {
    pub path: Option<String>,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionArguments {
    pub game: Vec<VersionArgument>,
    pub jvm: Vec<VersionArgument>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum VersionArgument {
    Raw(String),
    Conditional(VersionConditionalArgument),
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionConditionalArgument {
    pub value: SingleOrVec<String>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLogging {
    #[serde(default)]
    pub r#type: VersionLoggingType,
    pub argument: String,
    pub file: VersionLoggingFile,
}

#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionLoggingType {
    #[default]
    #[serde(rename = "log4j2-xml")]
    Log4j2Xml,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLoggingFile {
    pub id: String,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct AssetIndex {
    /// For version <= 13w23b (1.6.1).
    #[serde(default)]
    pub map_to_resources: bool,
    /// For 13w23b (1.6.1) < version <= 13w48b (1.7.2).
    #[serde(default)]
    pub r#virtual: bool,
    /// Mapping of assets from their real path to their download information.
    pub objects: HashMap<String, AssetObject>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct AssetObject {
    pub size: u32,
    pub hash: Sha1HashString,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: RuleOs,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(serde::Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuleOs {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// Only known value to use regex.
    pub version: Option<RegexString>,
}

#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Download {
    pub url: String,
    pub size: Option<u32>,
    pub sha1: Option<Sha1HashString>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
    Single(T),
    Vec(Vec<T>)
}

/// A SHA-1 hash parsed has a 40 hex characters string.
#[derive(Debug, Clone)]
pub struct Sha1HashString(pub [u8; 20]);

impl Deref for Sha1HashString {
    type Target = [u8; 20];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
#[derive(Debug, Clone)]
pub struct RegexString(pub Regex);

impl Deref for RegexString {
    type Target = Regex;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
