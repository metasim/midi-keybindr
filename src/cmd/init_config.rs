// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)

//! Implementation of the `init-config` subcommand: prints a sample YAML configuration.

use std::fs;
use std::io::Write;

use anyhow::{Context, Result};

use crate::cli::InitConfigArgs;

const SAMPLE_CONFIG: &str = "\
# midi-keybindr sample configuration
# Maps MIDI transport messages and notes to keyboard actions

mappings:
  - description: \"MIDI Start -> Play/Pause\"
    trigger:
      type: start
    action:
      keys: \"F8\"
  - description: \"MIDI Stop -> Stop\"
    trigger:
      type: stop
    action:
      keys: \"F8\"
  - description: \"MIDI Continue -> Play/Pause\"
    trigger:
      type: continue
    action:
      keys: \"F8\"
  - description: \"Middle C note-on -> F8\"
    trigger:
      type: note_on
      note: 60
    action:
      keys: \"F8\"
";

/// Executes the `init-config` subcommand.
pub fn execute(args: InitConfigArgs) -> Result<()> {
    if let Some(path) = args.output {
        fs::write(&path, SAMPLE_CONFIG)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        println!("Wrote sample config to {}", path.display());
    } else {
        std::io::stdout()
            .write_all(SAMPLE_CONFIG.as_bytes())
            .context("failed to write to stdout")?;
    }
    Ok(())
}
