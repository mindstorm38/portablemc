//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;

use crate::standard;


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangManifest {
    /// A map associated the latest versions, usually for release and snapshot, but we
    /// keep this a map because we don't really know if more types can be added in the
    /// future.
    pub latest: HashMap<String, String>,
    /// List of all versions.
    pub versions: Vec<MojangManifestVersion>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangManifestVersion {
    pub id: String,
    pub r#type: standard::serde::VersionType,
    pub time: String,
    pub release_time: String,
    #[serde(flatten)]
    pub download: standard::serde::Download,
    /// Unknown, used by official launcher.
    pub compliance_level: Option<u32>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub(crate) struct PmcMojangManifest {
    #[serde(flatten)]
    pub inner: MojangManifest,
    pub last_modified: Option<String>,
}
