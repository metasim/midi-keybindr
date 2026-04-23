// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)
// SPDX-FileContributor: GitHub Copilot Coding Agent (Claude Sonnet 4.6)

//! [`ChannelSet`] encodes a bitmask of selected MIDI channels (1-based, channels 1–16).

use anyhow::{Context, anyhow};
use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use std::fmt;

/// Bitmask over channels 1–16. Bit N-1 corresponds to channel N.
#[derive(Debug, Clone, Copy)]
pub struct ChannelSet(u16);

impl ChannelSet {
    /// Returns a set that contains all MIDI channels 1 through 16.
    pub fn all() -> Self {
        Self(0xFFFF)
    }

    /// Returns `true` when this set contains `channel`.
    pub fn contains(&self, channel: u8) -> bool {
        if !(1..=16).contains(&channel) {
            return false;
        }
        self.0 & (1 << (channel - 1)) != 0
    }

    fn insert_channel(&mut self, channel: u8) -> anyhow::Result<()> {
        if !(1..=16).contains(&channel) {
            return Err(anyhow!("channel {channel} is out of range (1-16)"));
        }
        self.0 |= 1 << (channel - 1);
        Ok(())
    }

    fn insert_range(&mut self, range: &str) -> anyhow::Result<()> {
        let (start, end) = range
            .split_once('-')
            .ok_or_else(|| anyhow!("invalid channel range {range:?}"))?;
        let start: u8 = start
            .trim()
            .parse()
            .with_context(|| format!("invalid channel in range {range:?}"))?;
        let end: u8 = end
            .trim()
            .parse()
            .with_context(|| format!("invalid channel in range {range:?}"))?;
        if start > end {
            return Err(anyhow!("invalid descending channel range {range:?}"));
        }
        for channel in start..=end {
            self.insert_channel(channel)?;
        }
        Ok(())
    }
}

/// Deserializes channel selectors from YAML.
///
/// Supported syntax:
/// - Integer scalar: `1`..`16`
/// - String scalar: `"*"` for all channels, `"3"` for one channel, `"2-4"` for a range
/// - Sequence combining integers and strings, e.g. `[1, "3-5", "9"]`
/// - A sequence containing `"*"` selects all channels
/// - Empty sequences select all channels
impl<'de> serde::Deserialize<'de> for ChannelSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ChannelSetVisitor;

        impl<'de> Visitor<'de> for ChannelSetVisitor {
            type Value = ChannelSet;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer channel, '*', or channel list")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let channel = u8::try_from(v).map_err(|_| E::custom("channel must fit in u8"))?;
                let mut set = ChannelSet(0);
                set.insert_channel(channel).map_err(E::custom)?;
                Ok(set)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.trim() == "*" {
                    return Ok(ChannelSet::all());
                }

                if v.contains('-') {
                    let mut set = ChannelSet(0);
                    set.insert_range(v).map_err(E::custom)?;
                    return Ok(set);
                }

                let channel: u8 = v
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid channel {v:?}")))?;
                let mut set = ChannelSet(0);
                set.insert_channel(channel).map_err(E::custom)?;
                Ok(set)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum ChannelItem {
                    Int(u8),
                    Str(String),
                }

                let mut set = ChannelSet(0);
                let mut any = false;

                while let Some(value) = seq.next_element::<ChannelItem>()? {
                    any = true;
                    let value = match value {
                        ChannelItem::Int(v) => v.to_string(),
                        ChannelItem::Str(v) => v,
                    };
                    if value.trim() == "*" {
                        return Ok(ChannelSet::all());
                    }
                    if value.contains('-') {
                        set.insert_range(&value).map_err(de::Error::custom)?;
                    } else {
                        let channel: u8 = value
                            .trim()
                            .parse()
                            .map_err(|_| de::Error::custom(format!("invalid channel {value:?}")))?;
                        set.insert_channel(channel).map_err(de::Error::custom)?;
                    }
                }

                if !any {
                    return Ok(ChannelSet::all());
                }

                Ok(set)
            }
        }

        deserializer.deserialize_any(ChannelSetVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::ChannelSet;

    /// Verifies list and range tokens are merged into one channel bitmask.
    #[test]
    fn parses_channel_list_and_range() {
        let set: ChannelSet = yaml_serde::from_str("[\"2-3\", \"9\"]").unwrap();
        assert!(set.contains(2));
        assert!(set.contains(3));
        assert!(set.contains(9));
        assert!(!set.contains(1));
    }

    /// Verifies integer scalar input selects that channel.
    #[test]
    fn parses_integer_scalar() {
        let set: ChannelSet = yaml_serde::from_str("1").unwrap();
        assert!(set.contains(1));
        assert!(!set.contains(2));
    }

    /// Verifies wildcard string selects all channels.
    #[test]
    fn parses_wildcard() {
        let set: ChannelSet = yaml_serde::from_str("\"*\"").unwrap();
        for ch in 1u8..=16 {
            assert!(set.contains(ch));
        }
    }

    /// Verifies empty sequence selects all channels.
    #[test]
    fn parses_empty_sequence() {
        let set: ChannelSet = yaml_serde::from_str("[]").unwrap();
        for ch in 1u8..=16 {
            assert!(set.contains(ch));
        }
    }

    /// Verifies descending range is an error.
    #[test]
    fn rejects_descending_range() {
        yaml_serde::from_str::<ChannelSet>("\"5-3\"").unwrap_err();
    }

    /// Verifies out-of-range channels are errors.
    #[test]
    fn rejects_out_of_range() {
        yaml_serde::from_str::<ChannelSet>("17").unwrap_err();
        yaml_serde::from_str::<ChannelSet>("0").unwrap_err();
    }
}
