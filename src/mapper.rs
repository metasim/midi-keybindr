use crate::config::{Action, Mapping, MidiEvent};

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

    /// Returns the first action that matches the given input context and event.
    pub fn match_event<'a>(
        &'a self,
        port_name: &str,
        channel: u8,
        event: &MidiEvent,
    ) -> Option<&'a Action> {
        self.mappings
            .iter()
            .find(|mapping| {
                mapping.devices.matches(port_name)
                    && mapping.channel.is_none_or(|set| set.contains(channel))
                    && mapping.trigger.matches_event(event)
            })
            .map(|mapping| &mapping.action)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Action, DeviceGlobs, Mapping, MidiEvent, trigger::NoteSpec};

    use super::MappingEngine;

    /// Verifies matching requires the same trigger and allowed channel.
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
