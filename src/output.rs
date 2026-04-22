use anyhow::Result;
use enigo::{Direction, Enigo, Keyboard, Settings};

use crate::config::KeyCombo;

pub struct KeyboardOutput {
    enigo: Enigo,
}

impl KeyboardOutput {
    pub fn new() -> Result<Self> {
        Ok(Self {
            enigo: Enigo::new(&Settings::default())?,
        })
    }

    pub fn send_combo(&mut self, combo: &KeyCombo) -> Result<()> {
        for modifier in &combo.modifiers {
            self.enigo.key(*modifier, Direction::Press)?;
        }

        self.enigo.key(combo.key, Direction::Click)?;

        for modifier in combo.modifiers.iter().rev() {
            self.enigo.key(*modifier, Direction::Release)?;
        }

        Ok(())
    }
}

pub fn ensure_accessibility_permission() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if !is_process_trusted_for_accessibility() {
            anyhow::bail!(
                "Accessibility permission is required. Grant access in System Settings → Privacy & Security → Accessibility for this process (or its parent app)."
            );
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn is_process_trusted_for_accessibility() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    unsafe { AXIsProcessTrusted() }
}
