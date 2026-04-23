// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)

use glob::{MatchOptions, Pattern};
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use std::fmt;

/// Case-insensitive glob patterns used to match MIDI device names.
#[derive(Debug, Clone)]
pub struct DeviceGlobs(pub Vec<Pattern>);

impl DeviceGlobs {
    /// Creates a matcher that accepts any device.
    pub fn any() -> Self {
        Self(vec![Pattern::new("*").expect("valid wildcard")])
    }

    /// Returns `true` when at least one pattern matches `name`.
    pub fn matches(&self, name: &str) -> bool {
        let opts = MatchOptions {
            case_sensitive: false,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        self.0
            .iter()
            .any(|pattern| pattern.matches_with(name, opts))
    }
}

impl Default for DeviceGlobs {
    fn default() -> Self {
        Self::any()
    }
}

impl<'de> serde::Deserialize<'de> for DeviceGlobs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DeviceGlobsVisitor;

        impl<'de> Visitor<'de> for DeviceGlobsVisitor {
            type Value = DeviceGlobs;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a glob string or a list of glob strings")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Pattern::new(v)
                    .map(|p| DeviceGlobs(vec![p]))
                    .map_err(|e| E::custom(format!("invalid glob pattern {v:?}: {e}")))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut patterns = Vec::new();
                while let Some(value) = seq.next_element::<String>()? {
                    let pattern = Pattern::new(&value).map_err(|e| {
                        de::Error::custom(format!("invalid glob pattern {value:?}: {e}"))
                    })?;
                    patterns.push(pattern);
                }

                if patterns.is_empty() {
                    return Ok(DeviceGlobs::any());
                }

                Ok(DeviceGlobs(patterns))
            }
        }

        deserializer.deserialize_any(DeviceGlobsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceGlobs;

    /// Verifies device glob matching is case-insensitive.
    #[test]
    fn matches_case_insensitive() {
        let globs: DeviceGlobs = yaml_serde::from_str("\"*akai*\"").unwrap();
        assert!(globs.matches("My Akai Device"));
    }
}
