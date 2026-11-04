//! What Welcome remembers between runs (`~/.config/nyx/welcome.toml`): whether setup
//! was finished, and the steps skipped.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub done: bool,
    pub skipped: BTreeSet<String>,
}

fn path() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    config.join("nyx/welcome.toml")
}

impl Saved {
    pub fn load() -> Saved {
        std::fs::read_to_string(path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = nyx_core::cmd::atomic_write(&path(), text);
        }
    }
}
