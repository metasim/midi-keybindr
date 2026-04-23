// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! Raw MIDI byte parsing: converts `midir` byte slices into [`ParsedEvent`] values.
//!
//! Note: a NoteOn with velocity 0 is normalized to a NoteOff, following the MIDI spec.

use anyhow::Result;

/// A normalized incoming MIDI event, separate from the trigger [`MidiEvent`] in config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingMidiEvent {
    /// A note-on event.
    NoteOn { note: u8 },
    /// A note-off event (including NoteOn with velocity 0).
    NoteOff { note: u8 },
    /// A control-change event with a single value.
    ControlChange { cc: u8, value: u8 },
    /// A program-change event.
    ProgramChange { program: u8 },
    /// MIDI System Real-Time: Start.
    Start,
    /// MIDI System Real-Time: Stop.
    Stop,
    /// MIDI System Real-Time: Continue.
    Continue,
}

/// Parsed MIDI message with channel and normalized event payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    /// One-based MIDI channel (1..=16), or `None` for system real-time messages.
    pub channel: Option<u8>,
    /// Normalized event representation used by the mapper.
    pub event: IncomingMidiEvent,
}

/// Parses raw MIDI bytes from `midir` into a normalized [`ParsedEvent`].
///
/// Returns `Ok(None)` for unrecognized or unsupported message types.
pub fn parse_message(message: &[u8]) -> Result<Option<ParsedEvent>> {
    use midly::live::LiveEvent;

    let event = match LiveEvent::parse(message) {
        Ok(LiveEvent::Midi { channel, message }) => {
            use midly::MidiMessage;
            let ch = u8::from(channel) + 1;
            let incoming = match message {
                MidiMessage::NoteOff { key, .. } => IncomingMidiEvent::NoteOff {
                    note: u8::from(key),
                },
                MidiMessage::NoteOn { key, vel } => {
                    if u8::from(vel) == 0 {
                        IncomingMidiEvent::NoteOff {
                            note: u8::from(key),
                        }
                    } else {
                        IncomingMidiEvent::NoteOn {
                            note: u8::from(key),
                        }
                    }
                }
                MidiMessage::Controller { controller, value } => IncomingMidiEvent::ControlChange {
                    cc: u8::from(controller),
                    value: u8::from(value),
                },
                MidiMessage::ProgramChange { program } => IncomingMidiEvent::ProgramChange {
                    program: u8::from(program),
                },
                _ => return Ok(None),
            };
            ParsedEvent {
                channel: Some(ch),
                event: incoming,
            }
        }
        Ok(LiveEvent::Realtime(rt)) => {
            use midly::live::SystemRealtime;
            let incoming = match rt {
                SystemRealtime::Start => IncomingMidiEvent::Start,
                SystemRealtime::Stop => IncomingMidiEvent::Stop,
                SystemRealtime::Continue => IncomingMidiEvent::Continue,
                _ => return Ok(None),
            };
            ParsedEvent {
                channel: None,
                event: incoming,
            }
        }
        Ok(_) => return Ok(None),
        Err(_) => return Ok(None),
    };

    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::{IncomingMidiEvent, parse_message};

    /// Verifies status byte and note data parse into a note-on event.
    #[test]
    fn parses_note_on() {
        let parsed = parse_message(&[0x92, 60, 100]).unwrap().unwrap();
        assert_eq!(parsed.channel, Some(3));
        assert_eq!(parsed.event, IncomingMidiEvent::NoteOn { note: 60 });
    }

    /// Verifies NoteOff status byte parses correctly.
    #[test]
    fn parses_note_off() {
        let parsed = parse_message(&[0x80, 60, 0]).unwrap().unwrap();
        assert_eq!(parsed.channel, Some(1));
        assert_eq!(parsed.event, IncomingMidiEvent::NoteOff { note: 60 });
    }

    /// Verifies NoteOn with velocity 0 produces NoteOff.
    #[test]
    fn note_on_vel_zero_is_note_off() {
        let parsed = parse_message(&[0x90, 60, 0]).unwrap().unwrap();
        assert_eq!(parsed.event, IncomingMidiEvent::NoteOff { note: 60 });
    }

    /// Verifies ControlChange parsing.
    #[test]
    fn parses_control_change() {
        let parsed = parse_message(&[0xB0, 7, 100]).unwrap().unwrap();
        assert_eq!(
            parsed.event,
            IncomingMidiEvent::ControlChange { cc: 7, value: 100 }
        );
    }

    /// Verifies ProgramChange parsing.
    #[test]
    fn parses_program_change() {
        let parsed = parse_message(&[0xC0, 5, 0]).unwrap().unwrap();
        assert_eq!(
            parsed.event,
            IncomingMidiEvent::ProgramChange { program: 5 }
        );
    }

    /// Verifies unknown status bytes return Ok(None).
    #[test]
    fn unknown_status_returns_none() {
        // SysEx start
        let result = parse_message(&[0xF0, 0x41, 0xF7]).unwrap();
        assert!(result.is_none());
    }

    /// Verifies short/malformed messages return Ok(None).
    #[test]
    fn short_message_returns_none() {
        let result = parse_message(&[]).unwrap();
        assert!(result.is_none());
    }
}
