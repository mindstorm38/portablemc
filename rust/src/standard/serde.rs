//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;

use crate::serde::{Sha1HashString, RegexString};
use crate::download::EntrySource;
use crate::gav::Gav;


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
    pub r#type: VersionType,
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

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
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
    pub downloads: VersionLibraryDownloads,
    pub natives: Option<HashMap<String, String>>,
    pub rules: Option<Vec<Rule>>,
    pub url: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionLibraryDownloads {
    pub artifact: Option<VersionLibraryDownload>,
    #[serde(default)]
    pub classifiers: HashMap<String, VersionLibraryDownload>,
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
    pub game: Vec<VersionArgument>,
    #[serde(default)]
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
pub struct JvmMetaManifest {
    #[serde(flatten)]
    pub platforms: HashMap<String, JvmMetaManifestPlatform>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct JvmMetaManifestPlatform {
    #[serde(flatten)]
    pub distributions: HashMap<String, JvmMetaManifestDistribution>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
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
    pub released: String,
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
    pub lzma: Download,
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

impl From<Download> for EntrySource {
    fn from(value: Download) -> Self {
        Self {
            url: value.url.into_boxed_str(),
            size: value.size,
            sha1: value.sha1.as_deref().copied(),
        }
    }
}

impl<'a> From<&'a Download> for EntrySource {
    fn from(value: &'a Download) -> Self {
        Self {
            url: value.url.clone().into_boxed_str(),
            size: value.size,
            sha1: value.sha1.as_deref().copied(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum SingleOrVec<T> {
    Single(T),
    Vec(Vec<T>)
}
