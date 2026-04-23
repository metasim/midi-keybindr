use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use std::fmt;

/// MIDI trigger variants supported by mapping rules.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiEvent {
    /// Matches a note-on event for a specific note.
    NoteOn { note: NoteSpec },
    /// Matches a note-off event for a specific note.
    NoteOff { note: NoteSpec },
    /// Matches a control-change event, optionally constrained by value range.
    ControlChange { cc: u8, value: Option<ValueRange> },
    /// Matches a program-change event.
    ProgramChange { program: u8 },
}

/// Inclusive MIDI value range.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ValueRange {
    /// Minimum accepted value.
    pub min: u8,
    /// Maximum accepted value.
    pub max: u8,
}

/// Parsed note token preserving both original text and resolved MIDI value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSpec {
    /// Original note token from configuration.
    pub raw: String,
    /// Resolved MIDI note number in the range 0..=127.
    pub note: u8,
}

impl NoteSpec {
    /// Parses either a MIDI integer (`0..=127`) or a note token like `C4`, `Bb3`, or `C-1`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if let Ok(num) = raw.parse::<u8>() {
            return Ok(Self {
                raw: raw.to_owned(),
                note: num,
            });
        }

        let mut chars = raw.chars().peekable();
        let letter = chars
            .next()
            .ok_or_else(|| format!("invalid note {raw:?}"))?
            .to_ascii_uppercase();

        let base = match letter {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return Err(format!("invalid note letter in {raw:?}")),
        };

        let mut semitone = base;
        if let Some(acc) = chars.peek().copied() {
            match acc {
                '#' => {
                    semitone += 1;
                    chars.next();
                }
                'b' | 'B' => {
                    semitone -= 1;
                    chars.next();
                }
                _ => {}
            }
        }

        let octave_str: String = chars.collect();
        if octave_str.is_empty() {
            return Err(format!("missing octave in note {raw:?}"));
        }
        let mut octave: i16 = octave_str
            .parse()
            .map_err(|_| format!("invalid octave in note {raw:?}"))?;

        if semitone < 0 {
            semitone += 12;
            octave -= 1;
        } else if semitone > 11 {
            semitone -= 12;
            octave += 1;
        }

        let midi = (octave + 1) * 12 + semitone;
        if !(0..=127).contains(&midi) {
            return Err(format!("note {raw:?} resolves out of MIDI range"));
        }

        Ok(Self {
            raw: raw.to_owned(),
            note: midi as u8,
        })
    }
}

impl MidiEvent {
    /// Returns `true` when an incoming event satisfies this trigger definition.
    pub fn matches_event(&self, event: &MidiEvent) -> bool {
        match (self, event) {
            (MidiEvent::NoteOn { note: a }, MidiEvent::NoteOn { note: b })
            | (MidiEvent::NoteOff { note: a }, MidiEvent::NoteOff { note: b }) => a.note == b.note,
            (
                MidiEvent::ControlChange {
                    cc: trigger_cc,
                    value: trigger_value,
                },
                MidiEvent::ControlChange {
                    cc: event_cc,
                    value: event_value,
                },
            ) => {
                if trigger_cc != event_cc {
                    return false;
                }
                match (trigger_value, event_value) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(expected), Some(actual)) => {
                        actual.min >= expected.min && actual.max <= expected.max
                    }
                }
            }
            (MidiEvent::ProgramChange { program: a }, MidiEvent::ProgramChange { program: b }) => {
                a == b
            }
            _ => false,
        }
    }
}

impl<'de> serde::Deserialize<'de> for NoteSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NoteSpecVisitor;

        impl<'de> Visitor<'de> for NoteSpecVisitor {
            type Value = NoteSpec;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("MIDI note integer or note string like C4")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v > 127 {
                    return Err(E::custom("MIDI note must be between 0 and 127"));
                }
                Ok(NoteSpec {
                    raw: v.to_string(),
                    note: v as u8,
                })
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                NoteSpec::parse(v).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(NoteSpecVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::NoteSpec;

    /// Verifies scientific pitch notation parses middle C as MIDI note 60.
    #[test]
    fn parses_middle_c() {
        let note = NoteSpec::parse("C4").unwrap();
        assert_eq!(note.note, 60);
    }

    /// Verifies enharmonic edge cases map across octave boundaries correctly.
    #[test]
    fn parses_enharmonic_boundaries() {
        assert_eq!(NoteSpec::parse("B#3").unwrap().note, 60);
        assert_eq!(NoteSpec::parse("Cb4").unwrap().note, 59);
    }

    /// Verifies negative octaves are supported with the C-1 lower MIDI bound.
    #[test]
    fn parses_negative_octave() {
        assert_eq!(NoteSpec::parse("C-1").unwrap().note, 0);
    }
}
