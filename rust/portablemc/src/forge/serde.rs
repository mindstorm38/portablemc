//! JSON schemas structures for serde deserialization.

use std::collections::HashMap;

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
    /// The loader version.
    pub version: String,
    /// The minecraft version.
    pub minecraft: String,
    /// The installing forge GAV, for early installers, no longer used in modern ones.
    pub path: Option<Gav>,
    /// Path to the 'version.json' file containing the full version metadata.
    pub json: String,
    /// Libraries for the installation.
    #[serde(default)]
    pub libraries: Vec<InstallLibrary>,
    /// Post-processors used to generate the final client.
    #[serde(default)]
    pub processors: Vec<InstallProcessor>,
    /// Constant data used for replacement in post-processor arguments.
    pub data: HashMap<String, InstallDataEntry>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct InstallLibrary {
    pub name: Gav,
    pub downloads: InstallLibraryDownloads,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct InstallLibraryDownloads {
    pub artifact: standard::serde::VersionLibraryDownload,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct InstallProcessor {
    pub jar: Gav,
    #[serde(default)]
    pub sides: Option<Vec<InstallSide>>,
    #[serde(default)]
    pub classpath: Vec<Gav>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct InstallDataEntry {
    pub client: String,
    pub server: String,
}

impl InstallDataEntry {

    pub fn get(&self, side: InstallSide) -> &str {
        match side {
            InstallSide::Client => &self.client,
            InstallSide::Server => &self.server,
        }
    }

}

/// For loader <= 1.12.2-14.23.5.2847
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstallProfile {
    pub install: LegacyInstall,
    pub version_info: Box<standard::serde::VersionMetadata>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstall {
    /// The game version.
    pub minecraft: String,
    pub path: Gav,
    /// The path, within the installer archive, where the universal JAR is located and
    /// can be extracted from.
    pub file_path: String,
}

#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum InstallSide {
    Client,
    Server,
}

impl InstallSide {

    pub fn as_str(self) -> &'static str {
        match self {
            InstallSide::Client => "client",
            InstallSide::Server => "server",
        }
    }

}
