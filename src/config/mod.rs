pub mod action;
pub mod channel;
pub mod device;
pub mod trigger;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

pub use action::{Action, KeyCombo};
pub use channel::ChannelSet;
pub use device::DeviceGlobs;
pub use trigger::MidiEvent;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Mapping {
    pub description: Option<String>,
    #[serde(default = "DeviceGlobs::any")]
    pub devices: DeviceGlobs,
    #[serde(default)]
    pub channel: Option<ChannelSet>,
    pub trigger: MidiEvent,
    pub action: Action,
}

impl Config {
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file {}", path.display()))?;
        yaml_serde::from_str(&contents)
            .with_context(|| format!("Failed to parse YAML config {}", path.display()))
    }
}
