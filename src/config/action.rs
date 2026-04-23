// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright 2026 Simeon H.K. Fitch
// SPDX-FileContributor: GitHub Copilot Coding Agent (OpenAI GPT-5.4)

use anyhow::{Result, anyhow};
use enigo::Key;
use serde::{Deserialize, Deserializer};

/// Parsed keyboard shortcut emitted when a mapping is triggered.
#[derive(Debug, Clone)]
pub struct KeyCombo {
    /// Modifier keys pressed before the primary key.
    pub modifiers: Vec<Key>,
    /// Primary key clicked while modifiers are held.
    pub key: Key,
    /// Original combo string from configuration.
    pub raw: String,
}

/// Action payload used by mapping rules.
#[derive(Debug, Deserialize, Clone)]
pub struct Action {
    /// Parsed keyboard shortcut for this action.
    #[serde(deserialize_with = "deserialize_key_combo")]
    pub keys: KeyCombo,
}

/// Deserializes a human-readable key combo string into a [`KeyCombo`].
///
/// Supported syntax:
/// - Tokens are separated by `+` (for example `Cmd+Shift+3`).
/// - All tokens except the final token are modifiers.
/// - Supported modifier aliases:
///   - Control: `Ctrl`, `Control`
///   - Shift: `Shift`
///   - Alt/Option: `Alt`, `Opt`, `Option`
///   - Command/Meta/Super: `Cmd`, `Command`, `Meta`, `Super`
/// - The final token is the primary key, supporting:
///   - One Unicode character (for example `a`, `3`, `/`)
///   - Named keys: `Space`, `Tab`, `Enter`/`Return`, arrows, `Esc`/`Escape`,
///     `Backspace`, `Delete`
///   - Function keys: `F1`..`F12`
pub fn deserialize_key_combo<'de, D>(deserializer: D) -> Result<KeyCombo, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    parse_key_combo(&raw).map_err(serde::de::Error::custom)
}

fn parse_key_combo(raw: &str) -> Result<KeyCombo> {
    let mut parts: Vec<&str> = raw
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        return Err(anyhow!("key combo cannot be empty"));
    }

    let key_token = parts.pop().expect("checked above");
    let mut modifiers = Vec::new();
    for modifier in parts {
        let key = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Key::Control,
            "shift" => Key::Shift,
            "alt" | "opt" | "option" => Key::Alt,
            "cmd" | "command" | "meta" | "super" => Key::Meta,
            _ => return Err(anyhow!("unsupported modifier {modifier:?}")),
        };
        modifiers.push(key);
    }

    let key = parse_primary_key(key_token)?;

    Ok(KeyCombo {
        modifiers,
        key,
        raw: raw.to_owned(),
    })
}

fn parse_primary_key(token: &str) -> Result<Key> {
    let lower = token.to_ascii_lowercase();
    let key = match lower.as_str() {
        "space" => return Ok(Key::Unicode(' ')),
        "tab" => Key::Tab,
        "enter" | "return" => Key::Return,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "esc" | "escape" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        _ => {
            if let Some(function_key) = parse_function_key(&lower) {
                return Ok(function_key);
            }

            let mut chars = token.chars();
            let ch = chars.next().ok_or_else(|| anyhow!("missing primary key"))?;
            if chars.next().is_none() {
                return Ok(Key::Unicode(ch));
            }

            return Err(anyhow!("unsupported primary key {token:?}"));
        }
    };

    Ok(key)
}

fn parse_function_key(token: &str) -> Option<Key> {
    match token {
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_key_combo;
    use enigo::Key;

    /// Verifies combo parsing splits modifiers and preserves original text.
    #[test]
    fn parses_combo() {
        let combo = parse_key_combo("Cmd+Shift+3").unwrap();
        assert_eq!(combo.modifiers.len(), 2);
        assert_eq!(combo.raw, "Cmd+Shift+3");
    }

    /// Verifies modifier aliases map to the expected key variants.
    #[test]
    fn parses_modifier_aliases() {
        let combo = parse_key_combo("Control+Option+F12").unwrap();
        assert_eq!(combo.modifiers, vec![Key::Control, Key::Alt]);
        assert_eq!(combo.key, Key::F12);
    }

    /// Verifies named non-character keys are supported as primary keys.
    #[test]
    fn parses_named_primary_keys() {
        let combo = parse_key_combo("Cmd+Space").unwrap();
        assert_eq!(combo.modifiers, vec![Key::Meta]);
        assert_eq!(combo.key, Key::Unicode(' '));
    }

    /// Verifies single-character primary keys are accepted directly.
    #[test]
    fn parses_single_character_primary() {
        let combo = parse_key_combo("Shift+/").unwrap();
        assert_eq!(combo.modifiers, vec![Key::Shift]);
        assert_eq!(combo.key, Key::Unicode('/'));
    }

    /// Verifies unknown modifiers are rejected with a parse error.
    #[test]
    fn rejects_unknown_modifier() {
        let err = parse_key_combo("Hyper+A").unwrap_err();
        assert!(err.to_string().contains("unsupported modifier"));
    }

    /// Verifies multi-character unknown primary tokens are rejected.
    #[test]
    fn rejects_unsupported_primary_token() {
        let err = parse_key_combo("Ctrl+Home").unwrap_err();
        assert!(err.to_string().contains("unsupported primary key"));
    }
}
