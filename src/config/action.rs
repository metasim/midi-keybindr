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
pub fn deserialize_key_combo<'de, D>(deserializer: D) -> Result<KeyCombo, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    parse_key_combo(&raw).map_err(serde::de::Error::custom)
}

fn parse_key_combo(raw: &str) -> Result<KeyCombo, String> {
    let mut parts: Vec<&str> = raw
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parts.is_empty() {
        return Err("key combo cannot be empty".to_string());
    }

    let key_token = parts.pop().expect("checked above");
    let mut modifiers = Vec::new();
    for modifier in parts {
        let key = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Key::Control,
            "shift" => Key::Shift,
            "alt" | "opt" | "option" => Key::Alt,
            "cmd" | "command" | "meta" | "super" => Key::Meta,
            _ => return Err(format!("unsupported modifier {modifier:?}")),
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

fn parse_primary_key(token: &str) -> Result<Key, String> {
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
            let ch = chars
                .next()
                .ok_or_else(|| "missing primary key".to_string())?;
            if chars.next().is_none() {
                return Ok(Key::Unicode(ch));
            }

            return Err(format!("unsupported primary key {token:?}"));
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

    /// Verifies combo parsing splits modifiers and preserves original text.
    #[test]
    fn parses_combo() {
        let combo = parse_key_combo("Cmd+Shift+3").unwrap();
        assert_eq!(combo.modifiers.len(), 2);
        assert_eq!(combo.raw, "Cmd+Shift+3");
    }
}
