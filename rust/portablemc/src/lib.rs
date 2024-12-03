//! PortableMC is a library and CLI for programmatically launching Minecraft.

pub(crate) mod path;
pub(crate) mod http;
pub(crate) mod tokio;

pub mod gav;
pub mod serde;

pub mod download;

pub mod msa;

pub mod standard;
pub mod mojang;
pub mod fabric;
