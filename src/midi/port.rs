// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! MIDI port enumeration and connection using `midir`.

use anyhow::{Context, Result, anyhow};
use midir::{Ignore, MidiInput, MidiInputConnection};

/// Lists available MIDI input ports as `(index, name)` pairs.
pub fn list_inputs() -> Result<Vec<(usize, String)>> {
    let midi_in = MidiInput::new("midi-keybindr list")?;
    let ports = midi_in.ports();

    ports
        .iter()
        .enumerate()
        .map(|(idx, port)| {
            let name = midi_in
                .port_name(port)
                .with_context(|| format!("failed to read name for MIDI port {idx}"))?;
            Ok((idx, name))
        })
        .collect()
}

/// Connects to all available MIDI input ports and returns live connection handles.
///
/// Port names are enumerated once from an initial `MidiInput`. Each connection
/// then creates a fresh `MidiInput` and matches by name so that the port index
/// remains stable across the enumeration.
pub fn connect_all<F>(mut make_callback: F) -> Result<Vec<MidiInputConnection<()>>>
where
    F: FnMut(String) -> Box<dyn FnMut(u64, &[u8]) + Send + 'static>,
{
    let midi_in = MidiInput::new("midi-keybindr-enum")?;
    let port_names: Vec<String> = midi_in
        .ports()
        .iter()
        .filter_map(|port| midi_in.port_name(port).ok())
        .collect();

    let mut connections = Vec::new();
    for port_name in port_names {
        let mut connect_input = MidiInput::new("midi-keybindr")?;
        connect_input.ignore(Ignore::None);
        let ports = connect_input.ports();
        let Some(port) = ports
            .iter()
            .find(|p| connect_input.port_name(p).ok().as_deref() == Some(&port_name))
        else {
            continue;
        };
        let mut callback = make_callback(port_name.clone());
        let conn = connect_input
            .connect(
                port,
                &format!("midi-keybindr-{port_name}"),
                move |timestamp, message, _state| callback(timestamp, message),
                (),
            )
            .map_err(|err| anyhow!(err.to_string()))?;
        connections.push(conn);
    }
    Ok(connections)
}
