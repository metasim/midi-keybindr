use crate::config::{Action, Mapping, MidiEvent};

#[derive(Debug, Clone)]
pub struct MappingEngine {
    mappings: Vec<Mapping>,
}

impl MappingEngine {
    pub fn new(mappings: Vec<Mapping>) -> Self {
        Self { mappings }
    }

    pub fn match_event<'a>(
        &'a self,
        port_name: &str,
        channel: u8,
        event: &MidiEvent,
    ) -> Option<&'a Action> {
        self.mappings
            .iter()
            .find(|mapping| {
                let device_ok = mapping.devices.matches(port_name);
                let channel_ok = mapping.channel.is_none_or(|set| set.contains(channel));
                device_ok && channel_ok && trigger_matches(&mapping.trigger, event)
            })
            .map(|mapping| &mapping.action)
    }
}

fn trigger_matches(trigger: &MidiEvent, event: &MidiEvent) -> bool {
    match (trigger, event) {
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
        (MidiEvent::ProgramChange { program: a }, MidiEvent::ProgramChange { program: b }) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Action, DeviceGlobs, Mapping, MidiEvent, trigger::NoteSpec};

    use super::MappingEngine;

    #[test]
    fn matches_by_device_channel_and_event() {
        let mappings = vec![Mapping {
            description: None,
            devices: DeviceGlobs::any(),
            channel: Some(crate::config::ChannelSet(1 << 2)),
            trigger: MidiEvent::NoteOn {
                note: NoteSpec {
                    raw: "C4".to_string(),
                    note: 60,
                },
            },
            action: Action {
                keys: crate::config::KeyCombo {
                    modifiers: vec![],
                    key: enigo::Key::F8,
                    raw: "F8".to_string(),
                },
            },
        }];

        let engine = MappingEngine::new(mappings);
        let event = MidiEvent::NoteOn {
            note: NoteSpec {
                raw: "60".to_string(),
                note: 60,
            },
        };

        assert!(engine.match_event("any", 3, &event).is_some());
        assert!(engine.match_event("any", 1, &event).is_none());
    }
}
