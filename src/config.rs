//! Local configuration: `config.toml` in the tool's working directory.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// Theme name: builtin or `md-styles/<name>.css`.
    pub theme: Option<String>,
    /// Max content width in columns.
    pub max_width: Option<usize>,
    /// Enable mouse capture in the TUI.
    pub mouse: Option<bool>,
}

impl Config {
    /// Load `./config.toml`; missing or invalid files yield defaults.
    pub fn load() -> Config {
        std::fs::read_to_string("config.toml")
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
}
