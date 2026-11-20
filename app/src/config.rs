use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub vault_path: String,
    pub choice: String,
    /// GTK accelerator string, e.g. "<Control><Alt>n". Empty = none.
    pub shortcut: String,
    /// Capture window background opacity in percent (30..=100).
    pub opacity: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config { vault_path: String::new(), choice: String::new(), shortcut: String::new(), opacity: 100 }
    }
}

fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(fallback))
}

fn path() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config").join("quicknote/config.json")
}

impl Config {
    pub fn load() -> Config {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }
}
