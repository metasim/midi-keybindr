// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent

//! [`MappingEngine`] matches incoming MIDI events against configured mappings.

use crate::config::Mapping;
use crate::midi::event::IncomingMidiEvent;

/// Matches parsed MIDI events against configured mappings.
#[derive(Debug, Clone)]
pub struct MappingEngine {
    mappings: Vec<Mapping>,
}

impl MappingEngine {
    /// Creates a mapping engine from parsed configuration mappings.
    pub fn new(mappings: Vec<Mapping>) -> Self {
        Self { mappings }
    }

    /// Returns the first mapping that matches the given input context and event.
    ///
    /// For system real-time events, `channel` should be `None` and channel filtering is skipped.
    pub fn match_event<'a>(
        &'a self,
        port_name: &str,
        channel: Option<u8>,
        event: &IncomingMidiEvent,
    ) -> Option<&'a Mapping> {
        self.mappings.iter().find(|mapping| {
            mapping.devices.matches(port_name)
                && (channel.is_none()
                    || mapping
                        .channel
                        .is_none_or(|set| channel.is_some_and(|ch| set.contains(ch))))
                && mapping.trigger.matches(event)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Action, DeviceGlobs, Mapping, MidiEvent, trigger::NoteSpec};
    use crate::midi::event::IncomingMidiEvent;

    use super::MappingEngine;

    fn make_channel_set(channel: u8) -> crate::config::ChannelSet {
        yaml_serde::from_str(&channel.to_string()).unwrap()
    }

    fn make_f8_action() -> Action {
        Action {
            keys: crate::config::action::parse_key_combo("F8").unwrap(),
        }
    }

    /// Verifies matching requires the same trigger and allowed channel.
    #[test]
    fn matches_by_device_channel_and_event() {
        let mappings = vec![Mapping {
            description: None,
            devices: DeviceGlobs::any(),
            channel: Some(make_channel_set(3)),
            trigger: MidiEvent::NoteOn {
                note: NoteSpec { note: 60 },
            },
            action: make_f8_action(),
        }];

        let engine = MappingEngine::new(mappings);
        let event = IncomingMidiEvent::NoteOn { note: 60 };

        assert!(engine.match_event("any", Some(3), &event).is_some());
        assert!(engine.match_event("any", Some(1), &event).is_none());
    }

    /// Verifies a non-matching device name excludes the mapping.
    #[test]
    fn device_glob_filters_non_matching_device() {
        let globs: DeviceGlobs = yaml_serde::from_str("\"*akai*\"").unwrap();
        let mappings = vec![Mapping {
            description: None,
            devices: globs,
            channel: None,
            trigger: MidiEvent::NoteOn {
                note: NoteSpec { note: 60 },
            },
            action: make_f8_action(),
        }];
        let engine = MappingEngine::new(mappings);
        let event = IncomingMidiEvent::NoteOn { note: 60 };

        assert!(engine.match_event("Akai MPK", Some(1), &event).is_some());
        assert!(engine.match_event("Roland A-49", Some(1), &event).is_none());
    }

    /// Verifies first-mapping-wins when multiple could match.
    #[test]
    fn first_mapping_wins() {
        let action1 = Action {
            keys: crate::config::action::parse_key_combo("F1").unwrap(),
        };
        let action2 = Action {
            keys: crate::config::action::parse_key_combo("F2").unwrap(),
        };
        let mappings = vec![
            Mapping {
                description: None,
                devices: DeviceGlobs::any(),
                channel: None,
                trigger: MidiEvent::NoteOn {
                    note: NoteSpec { note: 60 },
                },
                action: action1,
            },
            Mapping {
                description: None,
                devices: DeviceGlobs::any(),
                channel: None,
                trigger: MidiEvent::NoteOn {
                    note: NoteSpec { note: 60 },
                },
                action: action2,
            },
        ];
        let engine = MappingEngine::new(mappings);
        let result = engine
            .match_event("any", Some(1), &IncomingMidiEvent::NoteOn { note: 60 })
            .unwrap();
        assert_eq!(result.action.keys.to_string(), "F1");
    }

    /// Verifies ControlChange matching with and without a value range.
    #[test]
    fn matches_control_change_with_and_without_value_range() {
        let mapping_no_range = Mapping {
            description: None,
            devices: DeviceGlobs::any(),
            channel: None,
            trigger: MidiEvent::ControlChange { cc: 7, value: None },
            action: make_f8_action(),
        };
        let mapping_with_range = Mapping {
            description: None,
            devices: DeviceGlobs::any(),
            channel: None,
            trigger: MidiEvent::ControlChange {
                cc: 7,
                value: Some(yaml_serde::from_str("{min: 0, max: 63}").unwrap()),
            },
            action: make_f8_action(),
        };

        let engine_no_range = MappingEngine::new(vec![mapping_no_range]);
        let engine_with_range = MappingEngine::new(vec![mapping_with_range]);

        let cc_50 = IncomingMidiEvent::ControlChange { cc: 7, value: 50 };
        let cc_100 = IncomingMidiEvent::ControlChange { cc: 7, value: 100 };

        // No range matches any value
        assert!(
            engine_no_range
                .match_event("any", Some(1), &cc_50)
                .is_some()
        );
        assert!(
            engine_no_range
                .match_event("any", Some(1), &cc_100)
                .is_some()
        );

        // With range only matches in-range
        assert!(
            engine_with_range
                .match_event("any", Some(1), &cc_50)
                .is_some()
        );
        assert!(
            engine_with_range
                .match_event("any", Some(1), &cc_100)
                .is_none()
        );
    }

    /// Verifies ProgramChange matching.
    #[test]
    fn matches_program_change() {
        let mappings = vec![Mapping {
            description: None,
            devices: DeviceGlobs::any(),
            channel: None,
            trigger: MidiEvent::ProgramChange { program: 5 },
            action: make_f8_action(),
        }];
        let engine = MappingEngine::new(mappings);
        assert!(
            engine
                .match_event(
                    "any",
                    Some(1),
                    &IncomingMidiEvent::ProgramChange { program: 5 }
                )
                .is_some()
        );
        assert!(
            engine
                .match_event(
                    "any",
                    Some(1),
                    &IncomingMidiEvent::ProgramChange { program: 6 }
                )
                .is_none()
        );
    }

    /// Verifies NoteOff matching.
    #[test]
    fn matches_note_off() {
        let mappings = vec![Mapping {
            description: None,
            devices: DeviceGlobs::any(),
            channel: None,
            trigger: MidiEvent::NoteOff {
                note: NoteSpec { note: 60 },
            },
            action: make_f8_action(),
        }];
        let engine = MappingEngine::new(mappings);
        assert!(
            engine
                .match_event("any", Some(1), &IncomingMidiEvent::NoteOff { note: 60 })
                .is_some()
        );
        assert!(
            engine
                .match_event("any", Some(1), &IncomingMidiEvent::NoteOn { note: 60 })
                .is_none()
        );
    }
}
