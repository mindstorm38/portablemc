//! JSON schemas structures for serde deserialization.

use crate::gav::Gav;


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Game {
    pub version: String,
    pub stable: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Loader {
    pub separator: String,
    pub build: u32,
    pub maven: Gav,
    pub version: String,
    pub stable: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Intermediary {
    pub maven: Gav,
    pub version: String,
    pub stable: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct VersionLoader {
    pub loader: Loader,
    pub intermediary: Intermediary,
    // missing: launcherMeta,
}
