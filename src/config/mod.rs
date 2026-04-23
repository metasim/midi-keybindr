// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)

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

/// Root configuration document loaded from YAML.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Ordered mappings evaluated from first to last.
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

/// One mapping rule from MIDI trigger to keyboard action.
#[derive(Debug, Deserialize, Clone)]
pub struct Mapping {
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Device name glob filter for matching MIDI ports.
    #[serde(default = "DeviceGlobs::any")]
    pub devices: DeviceGlobs,
    /// Optional channel filter; absent means all channels.
    #[serde(default)]
    pub channel: Option<ChannelSet>,
    /// MIDI trigger that activates this mapping.
    pub trigger: MidiEvent,
    /// Keyboard action to emit when matched.
    pub action: Action,
}

impl Config {
    /// Loads and parses a configuration file from disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file {}", path.display()))?;
        yaml_serde::from_str(&contents)
            .with_context(|| format!("Failed to parse YAML config {}", path.display()))
    }
}
