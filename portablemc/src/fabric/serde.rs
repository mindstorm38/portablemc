//! Internal module for deserialization of the Fabric-like APIs.

#![allow(unused)]

use crate::maven::Gav;

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Game {
    pub version: String,
    pub stable: bool,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Loader {
    pub separator: String,
    pub build: u32,
    pub maven: Gav,
    pub version: String,
    pub stable: Option<bool>,  // Absent for some APIs (quilt)
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Intermediary {
    pub maven: Gav,
    pub version: String,
    pub stable: Option<bool>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct GameLoader {
    pub loader: Loader,
    pub intermediary: Intermediary,
    // missing: launcherMeta,
}