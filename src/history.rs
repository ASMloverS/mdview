//! Per-file reading position history: `history.toml` next to the executable.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 默认保留的历史条数上限。
pub const DEFAULT_HISTORY_SIZE: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    path: PathBuf,
    line: usize,
}

/// history.toml 的顶层结构。
#[derive(Debug, Default, Serialize, Deserialize)]
struct Doc {
    #[serde(default)]
    history: Vec<Entry>,
}

/// 阅读位置历史：MRU 在前，记录即写盘（best-effort）。
#[derive(Debug, Default)]
pub struct History {
    entries: Vec<Entry>,
    path: PathBuf,
}

/// Path of `history.toml` next to the executable; falls back to the
/// cwd-relative path when the exe location is unavailable.
fn history_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("history.toml")))
        .unwrap_or_else(|| PathBuf::from("history.toml"))
}

/// 规范化路径键：canonicalize 失败（文件不存在等）时用原路径。
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl History {
    /// Load `history.toml` next to the executable; missing or invalid
    /// files yield an empty history.
    pub fn load() -> History {
        Self::load_from(&history_path())
    }

    pub(crate) fn load_from(path: &Path) -> History {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str::<Doc>(&text).ok())
            .map(|doc| doc.history)
            .unwrap_or_default();
        History { entries, path: path.to_path_buf() }
    }

    /// 查文件上次的光标行。
    pub fn get(&self, path: &Path) -> Option<usize> {
        let key = canonical(path);
        self.entries.iter().find(|e| e.path == key).map(|e| e.line)
    }

    /// 记录文件的光标行：去重提到最前，截断到 cap，立即写盘。
    /// best-effort：IO 错误静默忽略。
    pub fn record(&mut self, path: &Path, line: usize, cap: usize) {
        let key = canonical(path);
        self.entries.retain(|e| e.path != key);
        self.entries.insert(0, Entry { path: key, line });
        self.entries.truncate(cap);
        self.save();
    }

    fn save(&self) {
        let doc = Doc { history: self.entries.clone() };
        if let Ok(text) = toml::to_string(&doc) {
            let _ = std::fs::write(&self.path, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-hist-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_missing_or_malformed_yields_empty() {
        let dir = temp_dir("load");
        let p = dir.join("history.toml");
        assert!(History::load_from(&p).entries.is_empty());
        std::fs::write(&p, "[[history] not toml").unwrap();
        assert!(History::load_from(&p).entries.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_inserts_mru_and_dedups() {
        let dir = temp_dir("mru");
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&a, 1, 200);
        h.record(&b, 2, 200);
        h.record(&a, 3, 200);
        assert_eq!(h.get(&a), Some(3));
        assert_eq!(h.get(&b), Some(2));
        assert_eq!(h.entries.len(), 2);
        assert_eq!(h.entries[0].path, canonical(&a), "最近使用的排在最前");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_truncates_to_cap() {
        let dir = temp_dir("cap");
        let mut h = History::load_from(&dir.join("history.toml"));
        for i in 0..5usize {
            let f = dir.join(format!("f{i}.md"));
            std::fs::write(&f, "x").unwrap();
            h.record(&f, i, 3);
        }
        assert_eq!(h.entries.len(), 3);
        assert_eq!(h.get(&dir.join("f4.md")), Some(4));
        assert_eq!(h.get(&dir.join("f1.md")), None, "最旧条目被淘汰");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_persists_and_reloads() {
        let dir = temp_dir("roundtrip");
        let p = dir.join("history.toml");
        let f = dir.join("note.md");
        std::fs::write(&f, "x").unwrap();
        let mut h = History::load_from(&p);
        h.record(&f, 42, 200);
        let h2 = History::load_from(&p);
        assert_eq!(h2.get(&f), Some(42));
        std::fs::remove_dir_all(&dir).ok();
    }
}
