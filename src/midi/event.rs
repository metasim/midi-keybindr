// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)

use anyhow::{Result, anyhow};
use midly::MidiMessage;
use std::convert::TryFrom;

use crate::config::trigger::{MidiEvent, NoteSpec};

/// Parsed MIDI message with channel and normalized event payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    /// One-based MIDI channel (1..=16).
    pub channel: u8,
    /// Normalized event representation used by the mapper.
    pub event: MidiEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MidiMessageType {
    NoteOff,
    NoteOn,
    ControlChange,
    ProgramChange,
}

impl TryFrom<u8> for MidiMessageType {
    type Error = ();

    fn try_from(status: u8) -> std::result::Result<Self, Self::Error> {
        match status & 0xF0 {
            0x80 => Ok(Self::NoteOff),
            0x90 => Ok(Self::NoteOn),
            0xB0 => Ok(Self::ControlChange),
            0xC0 => Ok(Self::ProgramChange),
            _ => Err(()),
        }
    }
}

/// Parses raw MIDI bytes from `midir` into a normalized [`ParsedEvent`].
pub fn parse_message(message: &[u8]) -> Result<Option<ParsedEvent>> {
    let (status, data1, data2) = match message {
        [status, data1, data2, ..] => (*status, *data1, *data2),
        [status, data1] => (*status, *data1, 0),
        _ => return Ok(None),
    };

    let channel = (status & 0x0F) + 1;
    let Ok(message_type) = MidiMessageType::try_from(status) else {
        return Ok(None);
    };

    let midi_event = match message_type {
        MidiMessageType::NoteOff => MidiEvent::NoteOff {
            note: NoteSpec {
                raw: data1.to_string(),
                note: parse_u7(data1)?,
            },
        },
        MidiMessageType::NoteOn => {
            let note = NoteSpec {
                raw: data1.to_string(),
                note: parse_u7(data1)?,
            };
            if data2 == 0 {
                MidiEvent::NoteOff { note }
            } else {
                MidiEvent::NoteOn { note }
            }
        }
        MidiMessageType::ControlChange => MidiEvent::ControlChange {
            cc: parse_u7(data1)?,
            value: Some(crate::config::trigger::ValueRange {
                min: parse_u7(data2)?,
                max: parse_u7(data2)?,
            }),
        },
        MidiMessageType::ProgramChange => MidiEvent::ProgramChange {
            program: parse_u7(data1)?,
        },
    };

    Ok(Some(ParsedEvent {
        channel,
        event: midi_event,
    }))
}

#[allow(dead_code)]
fn _parse_with_midly(message: &[u8]) -> Result<Option<(u8, MidiMessage)>> {
    let packet = midly::live::LiveEvent::parse(message)?;
    match packet {
        midly::live::LiveEvent::Midi { channel, message } => {
            Ok(Some((u8::from(channel) + 1, message)))
        }
        _ => Ok(None),
    }
}

fn parse_u7(value: u8) -> Result<u8> {
    if value <= 127 {
        Ok(value)
    } else {
        Err(anyhow!("MIDI value {value} is out of 7-bit range"))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_message;
    use crate::config::trigger::MidiEvent;

    /// Verifies status byte and note data parse into a note-on event.
    #[test]
    fn parses_note_on() {
        let parsed = parse_message(&[0x92, 60, 100]).unwrap().unwrap();
        assert_eq!(parsed.channel, 3);
        match parsed.event {
            MidiEvent::NoteOn { note } => assert_eq!(note.note, 60),
            _ => panic!("unexpected event"),
        }
    }
}
