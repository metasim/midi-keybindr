// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)

//! CLI argument types for the `midi-keybindr` executable, parsed with `clap`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Top-level CLI arguments for the `midi-keybindr` executable.
#[derive(Debug, Parser)]
#[command(
    name = "midi-keybindr",
    about = "Map MIDI events to keyboard actions",
    long_about = "midi-keybindr listens on MIDI input devices and translates MIDI events \
into synthesized keyboard events delivered to the focused application.\n\n\
Configuration is loaded in this order:\n  \
1. --config flag\n  \
2. MIDI_KEYBINDR_CONFIG environment variable\n  \
3. ~/.config/midi-keybindr/config.yaml\n\n\
See the project README for the config file format.",
    after_help = "EXAMPLES:\n    midi-keybindr -v              Start with verbose logging\n    midi-keybindr --check         Validate config without connecting\n    midi-keybindr list            List available MIDI input ports\n    midi-keybindr list --format json  Output as JSON",
    version,
    propagate_version = true
)]
pub struct Cli {
    /// Optional CLI subcommand.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to config file (overrides default search locations)
    #[arg(
        short,
        long,
        global = true,
        long_help = "Path to config file.\n\nSearch order when not specified:\n  1. MIDI_KEYBINDR_CONFIG environment variable\n  2. ~/.config/midi-keybindr/config.yaml"
    )]
    pub config: Option<PathBuf>,

    /// Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Validate the config file and exit without connecting to MIDI hardware
    #[arg(long, global = true)]
    pub check: bool,
}

/// Supported top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List available MIDI input ports
    List(ListArgs),
    /// Print a sample configuration file
    InitConfig(InitConfigArgs),
    /// Display live MIDI and keyboard events in an interactive TUI
    Display(DisplayArgs),
}

/// Arguments for the `list` subcommand.
#[derive(Debug, clap::Args)]
pub struct ListArgs {
    /// Output format
    #[arg(short, long, default_value = "human")]
    pub format: OutputFormat,
}

/// Arguments for the `init-config` subcommand.
#[derive(Debug, clap::Args)]
pub struct InitConfigArgs {
    /// Write output to this file instead of stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Arguments for the `display` subcommand.
#[derive(Debug, clap::Args)]
pub struct DisplayArgs {}

/// Output format for command responses.
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output.
    Human,
    /// JSON output.
    Json,
}
