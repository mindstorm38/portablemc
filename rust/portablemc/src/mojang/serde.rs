//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};

use crate::standard;


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangManifest {
    /// A map associated the latest versions.
    pub latest: HashMap<standard::serde::VersionType, String>,
    /// List of all versions.
    pub versions: Vec<MojangManifestVersion>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangManifestVersion {
    pub id: String,
    pub r#type: standard::serde::VersionType,
    pub time: DateTime<FixedOffset>,
    pub release_time: DateTime<FixedOffset>,
    #[serde(flatten)]
    pub download: standard::serde::Download,
    /// Unknown, used by official launcher.
    pub compliance_level: Option<u32>,
}