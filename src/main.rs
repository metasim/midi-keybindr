// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)

//! `midi-keybindr` binary entry point: parses CLI arguments, initialises tracing,
//! resolves the config path, and dispatches to the appropriate subcommand.

mod cli;
mod cmd;
mod config;
mod mapper;
mod midi;
mod output;

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose)?;

    if cli.check {
        let path = resolve_config_path(cli.config)?;
        let config = config::Config::from_path(&path)?;
        println!(
            "Config OK: {} mapping(s) loaded from {}",
            config.mappings.len(),
            path.display()
        );
        return Ok(());
    }

    match cli.command {
        Some(Command::List(args)) => cmd::list::execute(args),
        Some(Command::InitConfig(args)) => cmd::init_config::execute(args),
        Some(Command::Display(args)) => cmd::display::execute(args),
        None => {
            let path = resolve_config_path(cli.config)?;
            cmd::run::execute(&path)
        }
    }
}

fn init_tracing(verbose: u8) -> Result<()> {
    let default_level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

fn resolve_config_path(cli_config: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = cli_config {
        return Ok(path);
    }

    if let Ok(path) = env::var("MIDI_KEYBINDR_CONFIG")
        && !path.trim().is_empty()
    {
        return Ok(PathBuf::from(path));
    }

    let home = env::var("HOME").context("HOME is not set")?;
    let default = PathBuf::from(home).join(".config/midi-keybindr/config.yaml");

    if default.exists() {
        return Ok(default);
    }

    bail!(
        "No config file found. Use --config, set MIDI_KEYBINDR_CONFIG, or create ~/.config/midi-keybindr/config.yaml"
    )
}

#[cfg(test)]
mod tests {
    use super::resolve_config_path;
    use std::env;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Verifies an explicit CLI config path takes precedence.
    #[test]
    fn cli_config_wins() {
        let path = resolve_config_path(Some(PathBuf::from("/tmp/config.yaml"))).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/config.yaml"));
    }

    /// Verifies the MIDI_KEYBINDR_CONFIG environment variable is used when set.
    #[test]
    fn env_var_is_used() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            env::set_var("MIDI_KEYBINDR_CONFIG", "/tmp/env-config.yaml");
        }
        let path = resolve_config_path(None).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/env-config.yaml"));
        unsafe {
            env::remove_var("MIDI_KEYBINDR_CONFIG");
        }
    }
}
