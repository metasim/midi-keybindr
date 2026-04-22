use std::path::Path;
use std::sync::{Arc, mpsc};

use anyhow::Result;
use tracing::{debug, warn};

use crate::config::{Action, Config};
use crate::mapper::MappingEngine;
use crate::midi::{event, port};
use crate::output::{KeyboardOutput, ensure_accessibility_permission};

pub fn execute(config_path: &Path) -> Result<()> {
    ensure_accessibility_permission()?;

    let config = Config::from_path(config_path)?;
    let engine = Arc::new(MappingEngine::new(config.mappings));

    let (tx, rx) = mpsc::channel::<Action>();
    let callback_engine = Arc::clone(&engine);
    let callback_tx = tx.clone();

    let _connections = port::connect_all(move |port_name| {
        let local_engine = Arc::clone(&callback_engine);
        let local_tx = callback_tx.clone();

        Box::new(
            move |_timestamp, message| match event::parse_message(message) {
                Ok(Some(parsed)) => {
                    if let Some(action) =
                        local_engine.match_event(&port_name, parsed.channel, &parsed.event)
                    {
                        if let Err(err) = local_tx.send(action.clone()) {
                            warn!(error = %err, "failed to enqueue action");
                        }
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
