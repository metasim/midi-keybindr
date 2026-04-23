use anyhow::{Context, Result, anyhow};
use midir::{Ignore, MidiInput, MidiInputConnection};

/// Lists available MIDI input ports as `(index, name)` pairs.
pub fn list_inputs() -> Result<Vec<(usize, String)>> {
    let midi_in = MidiInput::new("midi-mapper list")?;
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
pub fn connect_all<F>(mut make_callback: F) -> Result<Vec<MidiInputConnection<()>>>
where
    F: FnMut(String) -> Box<dyn FnMut(u64, &[u8]) + Send + 'static>,
{
    let midi_in = MidiInput::new("midi-mapper")?;
    let ports = midi_in.ports();
    let mut connections = Vec::new();
    for (idx, port) in ports.iter().enumerate() {
        let port_name = midi_in
            .port_name(port)
            .unwrap_or_else(|_| "<unknown midi port>".to_string());
        let mut connect_input = MidiInput::new("midi-mapper")?;
        connect_input.ignore(Ignore::None);
        let connect_ports = connect_input.ports();
        let Some(connect_port) = connect_ports.get(idx) else {
            continue;
        };
        let mut callback = make_callback(port_name.clone());
        let conn = connect_input
            .connect(
                connect_port,
                &format!("midi-mapper-{port_name}"),
                move |timestamp, message, _state| {
                    callback(timestamp, message);
                },
                (),
            )
            .map_err(|err| anyhow!(err.to_string()))?;
        connections.push(conn);
    }

    Ok(connections)
}
