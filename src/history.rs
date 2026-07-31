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

    /// 最近一个仍可打开的文件：从 MRU 头部遍历，剔除不存在/不可读
    /// 的条目（立即写盘），返回第一个可用条目。全部失效则清空历史。
    pub fn latest_valid(&mut self) -> Option<PathBuf> {
        let first_valid = self
            .entries
            .iter()
            .position(|e| std::fs::read_to_string(&e.path).is_ok());
        match first_valid {
            Some(0) => Some(self.entries[0].path.clone()),
            Some(i) => {
                self.entries.drain(..i);
                self.save();
                Some(self.entries[0].path.clone())
            }
            None => {
                if !self.entries.is_empty() {
                    self.entries.clear();
                    self.save();
                }
                None
            }
        }
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

    #[test]
    fn latest_valid_empty_history_returns_none() {
        let dir = temp_dir("latest-empty");
        let mut h = History::load_from(&dir.join("history.toml"));
        assert_eq!(h.latest_valid(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn latest_valid_skips_stale_and_persists() {
        let dir = temp_dir("latest-skip");
        let p = dir.join("history.toml");
        let gone = dir.join("gone.md");
        let keep = dir.join("keep.md");
        std::fs::write(&keep, "x").unwrap();
        let mut h = History::load_from(&p);
        h.record(&keep, 2, 200);
        h.record(&gone, 3, 200); // gone 从不存在，record 后位于 MRU 头部
        assert_eq!(h.latest_valid(), Some(canonical(&keep)));
        assert_eq!(h.entries.len(), 1, "失效条目被剔除");
        let h2 = History::load_from(&p);
        assert_eq!(h2.entries.len(), 1, "剔除结果已写盘");
        assert_eq!(h2.get(&keep), Some(2));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn latest_valid_all_stale_clears_history() {
        let dir = temp_dir("latest-clear");
        let p = dir.join("history.toml");
        let mut h = History::load_from(&p);
        h.record(&dir.join("a.md"), 1, 200);
        h.record(&dir.join("b.md"), 2, 200);
        assert_eq!(h.latest_valid(), None);
        assert!(h.entries.is_empty());
        let h2 = History::load_from(&p);
        assert!(h2.entries.is_empty(), "清空后已写盘");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn latest_valid_first_entry_valid_touches_nothing() {
        let dir = temp_dir("latest-first");
        let p = dir.join("history.toml");
        let a = dir.join("a.md");
        std::fs::write(&a, "x").unwrap();
        let mut h = History::load_from(&p);
        h.record(&dir.join("stale.md"), 1, 200);
        h.record(&a, 5, 200); // a 在 MRU 头部，stale 在后
        assert_eq!(h.latest_valid(), Some(canonical(&a)));
        assert_eq!(h.entries.len(), 2, "命中首个有效条目即停，后面的失效条目保留");
        std::fs::remove_dir_all(&dir).ok();
    }
}
