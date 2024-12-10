//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, FixedOffset};

use crate::serde::{Sha1HashString, RegexString};
use crate::maven::Gav;


// ================== //
//  VERSION METADATA  //
// ================== //

/// A version metadata JSON schema.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
    /// The version id, should be the same as the directory the metadata is in.
    pub id: String,
    /// The version type, such as 'release' or 'snapshot'.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<VersionType>,
    /// The last time this version has been updated.
    #[serde(deserialize_with = "deserialize_date_time_chill")]
    pub time: DateTime<FixedOffset>,
    #[serde(deserialize_with = "deserialize_date_time_chill")]
    /// The first release time of this version.
    pub release_time: DateTime<FixedOffset>,
    /// If present, this is the name of another version to resolve after this one and
    /// where fallback values will be taken.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    /// Used by official launcher, optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_launcher_version: Option<u32>,
    /// Describe the Java version to use, optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_version: Option<VersionJavaVersion>,
    /// The asset index to use when launching the game, it also has download information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<VersionAssetIndex>,
    /// Legacy asset index id without download information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    /// Unknown, used by official launcher.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance_level: Option<u32>,
    /// A mapping of downloads for entry point JAR files, such as for client or for 
    /// server. This sometime also defines a server executable for old versions.
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub downloads: HashMap<String, Download>,
    /// The sequence of JAR libraries to include in the class path when running the
    /// version, the order of libraries should be respected in the class path (for
    /// some corner cases with mod loaders). When a library is defined, it can't be
    /// overridden by inherited versions.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<VersionLibrary>,
    /// The full class name to run as the main JVM class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    /// Legacy arguments command line.
    #[serde(rename = "minecraftArguments")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_arguments: Option<String>,
    /// Modern arguments for game and/or jvm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<VersionArguments>,
    /// Logging configuration.
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub logging: HashMap<String, VersionLogging>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

impl VersionType {

    pub fn as_str(&self) -> &'static str {
        match self {
            VersionType::Release => "release",
            VersionType::Snapshot => "snapshot",
            VersionType::OldBeta => "old_beta",
            VersionType::OldAlpha => "old_alpha",
        }
    }

}

/// Object describing the Mojang-provided Java version to use to launch the game.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionJavaVersion {
    pub component: Option<String>,
    pub major_version: u32,
}

/// Describe the asset index to use and how to download it when missing.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionAssetIndex {
    pub id: String,
    pub total_size: u32,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibrary {
    pub name: Gav,
    #[serde(default)]
    #[serde(skip_serializing_if = "VersionLibraryDownloads::is_empty")]
    pub downloads: VersionLibraryDownloads,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownloads {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<VersionLibraryDownload>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub classifiers: HashMap<String, VersionLibraryDownload>,
}

impl VersionLibraryDownloads {
    fn is_empty(&self) -> bool {
        self.artifact.is_none() && self.classifiers.is_empty()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownload {
    pub path: Option<String>,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionArguments {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub game: Vec<VersionArgument>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub jvm: Vec<VersionArgument>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum VersionArgument {
    Raw(String),
    Conditional(VersionConditionalArgument),
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionConditionalArgument {
    pub value: SingleOrVec<String>,
    pub rules: Option<Vec<Rule>>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLogging {
    #[serde(default)]
    pub r#type: VersionLoggingType,
    pub argument: String,
    pub file: VersionLoggingFile,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionLoggingType {
    #[default]
    #[serde(rename = "log4j2-xml")]
    Log4j2Xml,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLoggingFile {
    pub id: String,
    #[serde(flatten)]
    pub download: Download,
}


// ================== //
//    ASSET INDEX     //
// ================== //

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
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

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct AssetObject {
    pub size: u32,
    pub hash: Sha1HashString,
}

// ================== //
//   JVM MANIFESTS    //
// ================== //

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(transparent)]
pub struct JvmMetaManifest {
    pub platforms: HashMap<String, JvmMetaManifestPlatform>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(transparent)]
pub struct JvmMetaManifestPlatform {
    pub distributions: HashMap<String, JvmMetaManifestDistribution>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(transparent)]
pub struct JvmMetaManifestDistribution {
    pub variants: Vec<JvmMetaManifestVariant>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct JvmMetaManifestVariant {
    pub availability: JvmMetaManifestAvailability,
    pub manifest: Download,
    pub version: JvmMetaManifestVersion,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct JvmMetaManifestAvailability {
    pub group: u32,
    pub progress: u8,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct JvmMetaManifestVersion {
    pub name: String,
    pub released: DateTime<FixedOffset>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct JvmManifest {
    pub files: HashMap<String, JvmManifestFile>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase", tag = "type")] // Internally tagged.
pub enum JvmManifestFile {
    Directory,
    File {
        #[serde(default)]
        executable: bool,
        downloads: JvmManifestFileDownloads,
    },
    Link {
        target: String,
    },
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct JvmManifestFileDownloads {
    pub raw: Download,
    pub lzma: Option<Download>,
}

// ================== //
//       COMMON       //
// ================== //

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default)]
    pub os: RuleOs,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuleOs {
    pub name: Option<String>,
    pub arch: Option<String>,
    /// Only known value to use regex.
    pub version: Option<RegexString>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Download {
    pub url: String,
    pub size: Option<u32>,
    pub sha1: Option<Sha1HashString>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
    Single(T),
    Vec(Vec<T>)
}

/// Internal parsing function for RFC3339 date time parsing, specifically for 
/// [`VersionMetadata`] because it appears that some metadata might contain malformed
/// date time. 
/// 
/// This as been observed with NeoForge installer embedded version, an example of 
/// malformed time is "2024-12-09T23:22:49.408008176", where the timezone is missing.
fn deserialize_date_time_chill<'de, D>(deserializer: D) -> Result<DateTime<FixedOffset>, D::Error>
where 
    D: serde::Deserializer<'de>,
{

    use chrono::format::ParseErrorKind;

    struct Visitor;
    impl serde::de::Visitor<'_> for Visitor {

        type Value = DateTime<FixedOffset>;
        
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an RFC 3339 formatted date and time string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            match DateTime::parse_from_rfc3339(v) {
                Ok(date) => return Ok(date),
                Err(e) if e.kind() == ParseErrorKind::TooShort => {
                    // Try adding a 'Z' at the end, we don't know if this was the issue 
                    // so we retry.
                    let mut buf = v.to_string();
                    buf.push('Z');
                    DateTime::parse_from_rfc3339(&buf).map_err(|e| E::custom(e))
                }
                Err(e) => Err(E::custom(e)),
            }
        }
        
    }

    deserializer.deserialize_str(Visitor)

}
