// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! Implementation of the `list` subcommand: enumerates available MIDI input ports.

use anyhow::Result;
use serde::Serialize;

use crate::cli::OutputFormat;
use crate::midi::port;

#[derive(Serialize)]
struct PortEntry<'a> {
    index: usize,
    name: &'a str,
}

/// Executes the `list` subcommand.
pub fn execute(args: crate::cli::ListArgs) -> Result<()> {
    let ports = port::list_inputs()?;

    match args.format {
        OutputFormat::Human => print_human(&ports),
        OutputFormat::Json => print_json(&ports)?,
    }

    Ok(())
}

fn print_human(ports: &[(usize, String)]) {
    if ports.is_empty() {
        println!("No MIDI input ports found. Connect a MIDI device and try again.");
        return;
    }
    println!("Index  Name");
    println!("─────  ────────────────────────────────");
    for (index, name) in ports {
        println!("{index:<5}  {name}");
    }
}

fn print_json(ports: &[(usize, String)]) -> Result<()> {
    let entries: Vec<PortEntry<'_>> = ports
        .iter()
        .map(|(index, name)| PortEntry {
            index: *index,
            name: name.as_str(),
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&entries)?);
    Ok(())
}
