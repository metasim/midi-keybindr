// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! Runtime orchestration: connects MIDI ports, routes events through the mapper, and
//! emits keyboard actions.

use std::path::Path;
use std::sync::{Arc, mpsc};

use anyhow::Result;
use tracing::{debug, info, warn};

use crate::config::{Action, Config};
use crate::mapper::MappingEngine;
use crate::midi::{event, port};
use crate::output::{KeyboardOutput, ensure_accessibility_permission};

/// Executes mapper runtime mode using the provided config path.
pub fn execute(config_path: &Path) -> Result<()> {
    ensure_accessibility_permission()?;

    let config = Config::from_path(config_path)?;
    info!(mappings = config.mappings.len(), config = %config_path.display(), "Loaded mappings");

    let engine = Arc::new(MappingEngine::new(config.mappings));

    let (tx, rx) = mpsc::channel::<Action>();
    let callback_tx = tx.clone();
    drop(tx);

    let port_names: Vec<String> = port::list_inputs()?
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    info!(ports = port_names.len(), "Listening on MIDI ports");
    for name in &port_names {
        info!(port = %name, "Connected to MIDI port");
    }

    let callback_engine = Arc::clone(&engine);
    let _connections = port::connect_all(move |port_name| {
        let local_engine = Arc::clone(&callback_engine);
        let local_tx = callback_tx.clone();

        Box::new(
            move |_timestamp, message| match event::parse_message(message) {
                Ok(Some(parsed)) => {
                    if let Some(action) =
                        local_engine.match_event(&port_name, parsed.channel, &parsed.event)
                    {
                        info!(
                            port = %port_name,
                            channel = ?parsed.channel,
                            event = ?parsed.event,
                            action = %action.keys,
                            "Matched MIDI event"
                        );
                        if let Err(err) = local_tx.send(action.clone()) {
                            warn!(error = %err, "failed to enqueue action");
                        }
                    } else {
                        debug!(
                            port = %port_name,
                            channel = ?parsed.channel,
                            event = ?parsed.event,
                            "Unmatched MIDI event"
                        );
                    }
                }
                Ok(None) => {}
                Err(err) => debug!(error = %err, "failed to parse MIDI event"),
            },
        )
    })?;

    let mut output = KeyboardOutput::new()?;
    loop {
        let action = rx.recv()?;
        output.send_combo(&action.keys)?;
    }
}
