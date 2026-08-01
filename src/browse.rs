//! 目录树浏览器：单层按需加载的纯逻辑模块，与 App/UI 解耦。

use std::path::{Path, PathBuf};

/// 浏览位置：具体目录，或 Windows 虚拟驱动器列表层。
#[derive(Debug, Clone, PartialEq)]
pub enum Loc {
    Dir(PathBuf),
    Drives,
}

/// 列表条目：子目录（含驱动器）或 markdown 文件。
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Dir(PathBuf),
    File(PathBuf),
}

impl Entry {
    pub fn path(&self) -> &Path {
        match self {
            Entry::Dir(p) | Entry::File(p) => p,
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, Entry::Dir(_))
    }

    /// 显示名：常规条目取文件名；驱动器根（无文件名）取完整路径。
    pub fn name(&self) -> String {
        let p = self.path();
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string())
    }
}

/// 单层读取：目录 + md 文件；跳过 `.` 开头；目录优先，名称排序（大小写不敏感）。
pub fn load(loc: &Loc) -> std::io::Result<Vec<Entry>> {
    match loc {
        Loc::Drives => Ok(list_drives().into_iter().map(Entry::Dir).collect()),
        Loc::Dir(dir) => {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            for entry in std::fs::read_dir(dir)?.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if name.starts_with('.') {
                    continue;
                }
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(Entry::Dir(path));
                } else if is_markdown(name) {
                    files.push(Entry::File(path));
                }
            }
            let by_name = |a: &Entry, b: &Entry| {
                a.name().to_lowercase().cmp(&b.name().to_lowercase())
            };
            dirs.sort_by(by_name);
            files.sort_by(by_name);
            dirs.extend(files);
            Ok(dirs)
        }
    }
}

fn is_markdown(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

/// 可用驱动器：Windows A–Z 探测；其他平台仅根目录（不会实际用到）。
#[cfg(windows)]
fn list_drives() -> Vec<PathBuf> {
    (b'A'..=b'Z')
        .map(|c| PathBuf::from(format!("{}:\\", c as char)))
        .filter(|p| p.exists())
        .collect()
}

#[cfg(not(windows))]
fn list_drives() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时目录：子目录 zdir/adir、文件 b.md、A.MD、notes.txt、.hidden.md、.hdir/。
    fn fixture(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-browse-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(dir.join("zdir")).unwrap();
        std::fs::create_dir_all(dir.join("adir")).unwrap();
        std::fs::create_dir_all(dir.join(".hdir")).unwrap();
        std::fs::write(dir.join("b.md"), "b").unwrap();
        std::fs::write(dir.join("A.MD"), "a").unwrap();
        std::fs::write(dir.join("notes.txt"), "t").unwrap();
        std::fs::write(dir.join(".hidden.md"), "h").unwrap();
        dir
    }

    #[test]
    fn load_dirs_first_case_insensitive_hidden_skipped() {
        let dir = fixture("load");
        let entries = load(&Loc::Dir(dir.clone())).unwrap();
        let names: Vec<String> = entries.iter().map(|e| e.name()).collect();
        assert_eq!(names, vec!["adir", "zdir", "A.MD", "b.md"]);
        assert!(entries[0].is_dir() && entries[1].is_dir());
        assert!(!entries[2].is_dir() && !entries[3].is_dir());
        std::fs::remove_dir_all(&dir).ok();
    }
}
