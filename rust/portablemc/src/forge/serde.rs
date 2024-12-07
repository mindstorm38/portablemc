//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::maven::Gav;

use crate::standard;


#[derive(serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum InstallProfile {
    Modern(ModernInstallProfile),
    Legacy(LegacyInstallProfile),
}

/// For loader >= 1.12.2-14.23.5.2851
#[derive(serde::Deserialize, Debug, Clone)]
pub struct ModernInstallProfile {
    /// The installing forge GAV.
    pub path: Gav,
    /// Path to the 'version.json' file containing the full version metadata.
    pub json: String,
    #[serde(default)]
    pub processors: Vec<InstallProcessor>,
    /// Libraries for the installation.
    #[serde(default)]
    pub libraries: Vec<standard::serde::VersionLibrary>,
    pub data: HashMap<String, HashMap<InstallSide, String>>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct InstallProcessor {
    pub jar: Gav,
    pub sides: Vec<InstallSide>,
    pub classpath: Vec<Gav>,
    pub args: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum InstallSide {
    Client,
    Server,
}

/// For loader <= 1.12.2-14.23.5.2847
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstallProfile {
    pub install: LegacyInstall,
    pub version_info: standard::serde::VersionMetadata,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstall {
    /// The game version.
    pub minecraft: String,
    pub path: Gav,
    pub file_path: PathBuf,
}
