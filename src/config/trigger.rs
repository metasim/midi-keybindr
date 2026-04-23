// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! MIDI trigger types that describe which events activate a mapping rule.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use std::fmt;
use std::ops::RangeInclusive;

use crate::midi::event::IncomingMidiEvent;

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
    /// Matches a MIDI System Real-Time Start message.
    Start,
    /// Matches a MIDI System Real-Time Stop message.
    Stop,
    /// Matches a MIDI System Real-Time Continue message.
    Continue,
}

/// Inclusive MIDI value range, wrapping [`RangeInclusive<u8>`].
///
/// Deserializes from either `{min: N, max: N}` or `"N-M"` syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRange(RangeInclusive<u8>);

impl ValueRange {
    /// Returns the inner range.
    #[allow(dead_code)]
    pub fn range(&self) -> &RangeInclusive<u8> {
        &self.0
    }

    /// Returns true if the value is within the range.
    pub fn contains(&self, value: u8) -> bool {
        self.0.contains(&value)
    }
}

impl<'de> Deserialize<'de> for ValueRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueRangeVisitor;

        impl<'de> Visitor<'de> for ValueRangeVisitor {
            type Value = ValueRange;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(r#"a value range as {min: N, max: N} or "N-M""#)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let (min_str, max_str) = v
                    .split_once('-')
                    .ok_or_else(|| E::custom(format!("invalid range {v:?}: expected N-M")))?;
                let min: u8 = min_str
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid min in range {v:?}")))?;
                let max: u8 = max_str
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid max in range {v:?}")))?;
                if min > max {
                    return Err(E::custom(format!(
                        "invalid range {v:?}: min ({min}) must not exceed max ({max})"
                    )));
                }
                Ok(ValueRange(min..=max))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                let mut min: Option<u8> = None;
                let mut max: Option<u8> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "min" => min = Some(map.next_value()?),
                        "max" => max = Some(map.next_value()?),
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let min = min.ok_or_else(|| de::Error::missing_field("min"))?;
                let max = max.ok_or_else(|| de::Error::missing_field("max"))?;
                if min > max {
                    return Err(de::Error::custom(format!(
                        "invalid range: min ({min}) must not exceed max ({max})"
                    )));
                }
                Ok(ValueRange(min..=max))
            }
        }

        deserializer.deserialize_any(ValueRangeVisitor)
    }
}

/// Parsed note token preserving the resolved MIDI note number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSpec {
    /// Resolved MIDI note number in the range 0..=127.
    pub note: u8,
}

impl fmt::Display for NoteSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.note)
    }
}

impl NoteSpec {
    /// Parses either a MIDI integer (`0..=127`) or a note token like `C4`, `Bb3`, or `C-1`.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid MIDI note or resolves outside 0–127.
    pub fn parse(raw: &str) -> Result<Self> {
        if let Ok(num) = raw.parse::<u8>() {
            if num > 127 {
                return Err(anyhow!("note {raw:?} resolves out of MIDI range"));
            }
            return Ok(Self { note: num });
        }

        // Handle values > 127 typed as integers (will fail u8 parse above, but not i16)
        if let Ok(num) = raw.parse::<i16>() {
            return Err(anyhow!("note {raw:?} resolves out of MIDI range ({num})"));
        }

        let mut chars = raw.chars().peekable();
        let letter = chars
            .next()
            .ok_or_else(|| anyhow!("invalid note {raw:?}"))?
            .to_ascii_uppercase();

        let base: i16 = match letter {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => return Err(anyhow!("invalid note letter in {raw:?}")),
        };

        let mut semitone: i16 = base;
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
            return Err(anyhow!("missing octave in note {raw:?}"));
        }
        let mut octave: i16 = octave_str
            .parse()
            .with_context(|| format!("invalid octave in note {raw:?}"))?;

        if semitone < 0 {
            semitone += 12;
            octave -= 1;
        } else if semitone > 11 {
            semitone -= 12;
            octave += 1;
        }

        let midi = (octave + 1) * 12 + semitone;
        if !(0..=127).contains(&midi) {
            return Err(anyhow!("note {raw:?} resolves out of MIDI range"));
        }

        Ok(Self { note: midi as u8 })
    }
}

impl MidiEvent {
    /// Returns `true` when `incoming` satisfies this trigger definition.
    ///
    /// For `ControlChange` with no value range, any incoming CC value matches.
    /// For `ControlChange` with a value range, the incoming value must be within the range.
    pub fn matches(&self, incoming: &IncomingMidiEvent) -> bool {
        match (self, incoming) {
            (MidiEvent::NoteOn { note: a }, IncomingMidiEvent::NoteOn { note: b }) => a.note == *b,
            (MidiEvent::NoteOff { note: a }, IncomingMidiEvent::NoteOff { note: b }) => {
                a.note == *b
            }
            (
                MidiEvent::ControlChange {
                    cc: trigger_cc,
                    value: trigger_value,
                },
                IncomingMidiEvent::ControlChange {
                    cc: event_cc,
                    value: event_value,
                },
            ) => {
                if trigger_cc != event_cc {
                    return false;
                }
                match trigger_value {
                    None => true,
                    Some(range) => range.contains(*event_value),
                }
            }
            (
                MidiEvent::ProgramChange { program: a },
                IncomingMidiEvent::ProgramChange { program: b },
            ) => a == b,
            (MidiEvent::Start, IncomingMidiEvent::Start) => true,
            (MidiEvent::Stop, IncomingMidiEvent::Stop) => true,
            (MidiEvent::Continue, IncomingMidiEvent::Continue) => true,
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
                Ok(NoteSpec { note: v as u8 })
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

    /// Verifies notes below MIDI 0 are rejected.
    #[test]
    fn rejects_below_midi_zero() {
        NoteSpec::parse("Cb-1").unwrap_err();
    }

    /// Verifies notes above MIDI 127 are rejected.
    #[test]
    fn rejects_above_midi_127() {
        NoteSpec::parse("G#9").unwrap_err();
    }

    /// Verifies integer bounds: 0 and 127 accepted, 128 rejected.
    #[test]
    fn parses_integer_bounds() {
        assert_eq!(NoteSpec::parse("0").unwrap().note, 0);
        assert_eq!(NoteSpec::parse("127").unwrap().note, 127);
        NoteSpec::parse("128").unwrap_err();
    }

    /// Verifies note letter case-insensitivity.
    #[test]
    fn case_insensitive_note_letter() {
        assert_eq!(NoteSpec::parse("c4").unwrap().note, 60);
    }

    /// Verifies invalid inputs are rejected.
    #[test]
    fn rejects_invalid_inputs() {
        NoteSpec::parse("C").unwrap_err(); // missing octave
        NoteSpec::parse("").unwrap_err(); // empty
        NoteSpec::parse("H4").unwrap_err(); // unknown letter
    }
}
