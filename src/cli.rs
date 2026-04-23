use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Top-level CLI arguments for the `midi-mapper` executable.
#[derive(Debug, Parser)]
#[command(
    name = "midi-mapper",
    about = "Map MIDI events to keyboard actions",
    version,
    propagate_version = true
)]
pub struct Cli {
    /// Optional CLI subcommand.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to config file (overrides default search locations)
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Supported top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List available MIDI input ports
    List(ListArgs),
}

/// Arguments for the `list` subcommand.
#[derive(Debug, clap::Args)]
pub struct ListArgs {
    /// Output format
    #[arg(short, long, default_value = "human")]
    pub format: OutputFormat,
}

/// Output format for command responses.
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output.
    Human,
    /// JSON output.
    Json,
}
