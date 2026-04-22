use anyhow::{Result, anyhow};
use midly::MidiMessage;

use crate::config::trigger::{MidiEvent, NoteSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent {
    pub channel: u8,
    pub event: MidiEvent,
}

pub fn parse_message(message: &[u8]) -> Result<Option<ParsedEvent>> {
    let (status, data1, data2) = match message {
        [status, data1, data2, ..] => (*status, *data1, *data2),
        [status, data1] => (*status, *data1, 0),
        _ => return Ok(None),
    };

    let channel = (status & 0x0F) + 1;
    let ty = status & 0xF0;

    let midi_event = match ty {
        0x80 => MidiEvent::NoteOff {
            note: NoteSpec {
                raw: data1.to_string(),
                note: parse_u7(data1)?,
            },
        },
        0x90 => {
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
        0xB0 => MidiEvent::ControlChange {
            cc: parse_u7(data1)?,
            value: Some(crate::config::trigger::ValueRange {
                min: parse_u7(data2)?,
                max: parse_u7(data2)?,
            }),
        },
        0xC0 => MidiEvent::ProgramChange {
            program: parse_u7(data1)?,
        },
        _ => return Ok(None),
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
