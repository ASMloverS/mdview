//! Local configuration: `config.toml` next to the executable.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// Theme name: builtin or `md-styles/<name>.css`.
    pub theme: Option<String>,
    /// Max content width in columns.
    pub max_width: Option<usize>,
    /// Enable mouse capture in the TUI.
    pub mouse: Option<bool>,
}

/// Path of `config.toml` next to the executable; falls back to the
/// cwd-relative path when the exe location is unavailable.
fn config_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("config.toml")))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

impl Config {
    /// Load `config.toml` next to the executable; missing or invalid
    /// files yield defaults.
    pub fn load() -> Config {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Persist the selected theme into `config.toml` next to the
    /// executable, preserving all other keys. Best-effort: IO errors are
    /// ignored and a malformed existing file is never overwritten.
    pub fn save_theme(name: &str) {
        let _ = save_theme_to(&config_path(), name);
    }
}

/// Update the `theme` key in the TOML file at `path`, creating the file
/// if missing. Returns Ok(true) when the file was written; Ok(false)
/// when the theme was already current or the existing file could not
/// be parsed (in which case it is left untouched).
fn save_theme_to(path: &Path, name: &str) -> std::io::Result<bool> {
    let mut value: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => match text.parse() {
            Ok(v) => v,
            Err(_) => return Ok(false),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(e),
    };
    let Some(table) = value.as_table_mut() else {
        return Ok(false);
    };
    if table.get("theme").and_then(|v| v.as_str()) == Some(name) {
        return Ok(false);
    }
    table.insert("theme".to_string(), toml::Value::String(name.to_string()));
    let text = toml::to_string(&value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, text)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-cfg-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    fn cleanup(p: &std::path::Path) {
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[test]
    fn writes_new_file() {
        let p = temp_path("new");
        assert!(save_theme_to(&p, "nord").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("nord"));
        cleanup(&p);
    }

    #[test]
    fn updates_theme_and_preserves_other_keys() {
        let p = temp_path("keep");
        std::fs::write(&p, "theme = \"nord\"\nmax_width = 80\nmouse = false\ncustom = \"x\"\n").unwrap();
        assert!(save_theme_to(&p, "dracula").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("dracula"));
        assert_eq!(v.get("max_width").and_then(|t| t.as_integer()), Some(80));
        assert_eq!(v.get("mouse").and_then(|t| t.as_bool()), Some(false));
        assert_eq!(v.get("custom").and_then(|t| t.as_str()), Some("x"));
        cleanup(&p);
    }

    #[test]
    fn skips_write_when_theme_unchanged() {
        let p = temp_path("same");
        let original = "theme = \"nord\"\nmax_width = 80\n";
        std::fs::write(&p, original).unwrap();
        assert!(!save_theme_to(&p, "nord").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        cleanup(&p);
    }

    #[test]
    fn config_path_is_next_to_executable() {
        let p = config_path();
        assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("config.toml"));
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(p.parent().unwrap(), exe_dir);
    }

    #[test]
    fn never_clobbers_malformed_file() {
        let p = temp_path("bad");
        let original = "theme = [not valid toml";
        std::fs::write(&p, original).unwrap();
        assert!(!save_theme_to(&p, "nord").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        cleanup(&p);
    }
}
