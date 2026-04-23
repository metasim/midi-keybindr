// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! Configuration loading: parses YAML mapping rules into strongly-typed structures.

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
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the YAML is malformed.
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file {}", path.display()))?;
        yaml_serde::from_str(&contents)
            .with_context(|| format!("Failed to parse YAML config {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::io::Write;

    /// Verifies Config::from_path round-trips a minimal YAML config.
    #[test]
    fn round_trips_minimal_config() {
        let yaml = "mappings:\n  - trigger:\n      type: note_on\n      note: 60\n    action:\n      keys: F8\n";
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(yaml.as_bytes()).unwrap();
        let config = Config::from_path(f.path()).unwrap();
        assert_eq!(config.mappings.len(), 1);
        match &config.mappings[0].trigger {
            crate::config::trigger::MidiEvent::NoteOn { note } => assert_eq!(note.note, 60),
            _ => panic!("expected NoteOn"),
        }
    }
}
