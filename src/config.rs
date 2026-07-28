//! Local configuration: `config.toml` next to the executable.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 内容水平对齐方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ContentAlign {
    Center,
    Left,
}

impl ContentAlign {
    /// 容错解析配置字符串；非法值返回 None（回退默认）。
    pub fn from_str(s: &str) -> Option<ContentAlign> {
        <ContentAlign as clap::ValueEnum>::from_str(s, true).ok()
    }

    // TODO(content-align): remove allow once wired up in later tasks.
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentAlign::Center => "center",
            ContentAlign::Left => "left",
        }
    }

    // TODO(content-align): remove allow once wired up in later tasks.
    #[allow(dead_code)]
    pub fn toggle(&self) -> ContentAlign {
        match self {
            ContentAlign::Center => ContentAlign::Left,
            ContentAlign::Left => ContentAlign::Center,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// Theme name: builtin or `md-styles/<name>.css`.
    pub theme: Option<String>,
    /// Max content width in columns.
    pub max_width: Option<usize>,
    /// Enable mouse capture in the TUI.
    pub mouse: Option<bool>,
    /// Content alignment: "center" (default) or "left".
    pub align: Option<String>,
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
        let _ = save_key_to(&config_path(), "theme", name);
    }

    /// Persist the content alignment into `config.toml`, preserving all
    /// other keys. Same best-effort semantics as `save_theme`.
    // TODO(content-align): remove allow once wired up in later tasks.
    #[allow(dead_code)]
    pub fn save_align(value: &str) {
        let _ = save_key_to(&config_path(), "align", value);
    }
}

/// Update `key` in the TOML file at `path`, creating the file if missing.
/// Returns Ok(true) when the file was written; Ok(false) when the value
/// was already current or the existing file could not be parsed (in which
/// case it is left untouched).
fn save_key_to(path: &Path, key: &str, value: &str) -> std::io::Result<bool> {
    let mut doc: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => match text.parse() {
            Ok(v) => v,
            Err(_) => return Ok(false),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(e),
    };
    let Some(table) = doc.as_table_mut() else {
        return Ok(false);
    };
    if table.get(key).and_then(|v| v.as_str()) == Some(value) {
        return Ok(false);
    }
    table.insert(key.to_string(), toml::Value::String(value.to_string()));
    let text = toml::to_string(&doc)
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
        assert!(save_key_to(&p, "theme", "nord").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("nord"));
        cleanup(&p);
    }

    #[test]
    fn updates_theme_and_preserves_other_keys() {
        let p = temp_path("keep");
        std::fs::write(&p, "theme = \"nord\"\nmax_width = 80\nmouse = false\ncustom = \"x\"\n").unwrap();
        assert!(save_key_to(&p, "theme", "dracula").unwrap());
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
        assert!(!save_key_to(&p, "theme", "nord").unwrap());
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
        assert!(!save_key_to(&p, "theme", "nord").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        cleanup(&p);
    }

    #[test]
    fn content_align_parse_toggle_and_as_str() {
        assert_eq!(ContentAlign::from_str("center"), Some(ContentAlign::Center));
        assert_eq!(ContentAlign::from_str("left"), Some(ContentAlign::Left));
        assert_eq!(ContentAlign::from_str("bogus"), None);
        assert_eq!(ContentAlign::Center.toggle(), ContentAlign::Left);
        assert_eq!(ContentAlign::Left.toggle(), ContentAlign::Center);
        assert_eq!(ContentAlign::Center.as_str(), "center");
        assert_eq!(ContentAlign::Left.as_str(), "left");
    }

    #[test]
    fn saves_align_preserving_other_keys() {
        let p = temp_path("align");
        std::fs::write(&p, "theme = \"nord\"\n").unwrap();
        assert!(save_key_to(&p, "align", "left").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("align").and_then(|t| t.as_str()), Some("left"));
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("nord"));
        cleanup(&p);
    }
}
