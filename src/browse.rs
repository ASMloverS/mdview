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

/// enter 的结果。
pub enum EnterOutcome {
    OpenFile(PathBuf),
    Entered,
    Failed(String),
    Noop,
}

pub struct Browser {
    pub loc: Loc,
    pub entries: Vec<Entry>,
    pub selected: usize,
}

impl Browser {
    /// 从指定目录启动。
    pub fn new(dir: &Path) -> Browser {
        let loc = Loc::Dir(dir.to_path_buf());
        let entries = load(&loc).unwrap_or_default();
        Browser { loc, entries, selected: 0 }
    }

    /// 从当前工作目录启动。
    pub fn from_cwd() -> Browser {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Browser::new(&cwd)
    }

    /// 移动选中（clamp 在列表范围内）。
    pub fn move_sel(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() - 1;
        let next = self.selected as isize + delta;
        self.selected = next.clamp(0, last as isize) as usize;
    }

    /// 进入选中项：目录 → 重载；文件 → 返回路径交 App 打开。
    pub fn enter(&mut self) -> EnterOutcome {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return EnterOutcome::Noop;
        };
        match entry {
            Entry::File(p) => EnterOutcome::OpenFile(p),
            Entry::Dir(p) => match load(&Loc::Dir(p.clone())) {
                Ok(entries) => {
                    self.loc = Loc::Dir(p);
                    self.entries = entries;
                    self.selected = 0;
                    EnterOutcome::Entered
                }
                Err(e) => EnterOutcome::Failed(format!("cannot read {}: {e}", p.display())),
            },
        }
    }

    /// 返回上级；Windows 盘符根再向上 → 驱动器列表；Drives 再向上无操作。
    pub fn up(&mut self) -> Result<(), String> {
        let Loc::Dir(cur) = &self.loc else { return Ok(()) };
        let cur = cur.clone();
        match cur.parent() {
            Some(parent) => {
                let loc = Loc::Dir(parent.to_path_buf());
                let entries = load(&loc)
                    .map_err(|e| format!("cannot read {}: {e}", parent.display()))?;
                self.selected = entries.iter().position(|e| e.path() == cur).unwrap_or(0);
                self.loc = loc;
                self.entries = entries;
                Ok(())
            }
            None => {
                #[cfg(windows)]
                {
                    let entries = load(&Loc::Drives).map_err(|e| e.to_string())?;
                    self.selected =
                        entries.iter().position(|e| e.path() == cur).unwrap_or(0);
                    self.loc = Loc::Drives;
                    self.entries = entries;
                }
                Ok(())
            }
        }
    }

    /// 刷新当前层，选中项按路径尽量保留。
    pub fn refresh(&mut self) -> Result<(), String> {
        let cur = self.entries.get(self.selected).map(|e| e.path().to_path_buf());
        let entries = load(&self.loc).map_err(|e| match &self.loc {
            Loc::Dir(p) => format!("cannot read {}: {e}", p.display()),
            Loc::Drives => e.to_string(),
        })?;
        self.selected = match cur.and_then(|p| entries.iter().position(|e| e.path() == p)) {
            Some(i) => i,
            None => self.selected.min(entries.len().saturating_sub(1)),
        };
        self.entries = entries;
        Ok(())
    }

    /// 定位到文件所在目录并选中该文件；失败返回错误信息，浏览器不动。
    pub fn reveal(&mut self, file: &Path) -> Result<(), String> {
        let abs = absolutize(file);
        let Some(parent) = abs.parent() else {
            return Err(format!("cannot locate {}", file.display()));
        };
        let loc = Loc::Dir(parent.to_path_buf());
        let entries =
            load(&loc).map_err(|e| format!("cannot read {}: {e}", parent.display()))?;
        self.selected = entries.iter().position(|e| e.path() == abs).unwrap_or(0);
        self.loc = loc;
        self.entries = entries;
        Ok(())
    }
}

/// 转绝对路径：相对路径基于 cwd 拼接（不用 canonicalize，避免 Windows UNC 前缀）。
fn absolutize(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    }
}

/// 目录统计：(子目录数, md 文件数)，单层不递归；不可读返回 None。
pub fn dir_stats(path: &Path) -> Option<(usize, usize)> {
    let entries = load(&Loc::Dir(path.to_path_buf())).ok()?;
    let dirs = entries.iter().filter(|e| e.is_dir()).count();
    Some((dirs, entries.len() - dirs))
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

    #[test]
    fn enter_dir_loads_it_and_enter_file_returns_path() {
        let dir = fixture("enter");
        let mut b = Browser::new(&dir);
        // 选中 adir（目录优先排第一）。
        match b.enter() {
            EnterOutcome::Entered => {}
            _ => panic!("expected Entered"),
        }
        assert_eq!(b.loc, Loc::Dir(dir.join("adir")));
        assert!(b.entries.is_empty(), "adir 为空目录");
        // 回到 fixture 根，enter 文件返回路径。
        b.loc = Loc::Dir(dir.clone());
        b.entries = load(&b.loc).unwrap();
        b.selected = 2; // A.MD
        match b.enter() {
            EnterOutcome::OpenFile(p) => assert_eq!(p, dir.join("A.MD")),
            _ => panic!("expected OpenFile"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enter_unreadable_dir_fails_in_place() {
        let dir = fixture("enter-fail");
        let mut b = Browser::new(&dir);
        b.entries = vec![Entry::Dir(dir.join("gone"))];
        match b.enter() {
            EnterOutcome::Failed(msg) => assert!(msg.contains("cannot read")),
            _ => panic!("expected Failed"),
        }
        assert_eq!(b.loc, Loc::Dir(dir.clone()), "失败后停留原位");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn up_moves_to_parent_and_selects_child() {
        let dir = fixture("up");
        let mut b = Browser::new(&dir.join("zdir"));
        b.up().unwrap();
        assert_eq!(b.loc, Loc::Dir(dir.clone()));
        assert_eq!(b.entries[b.selected].name(), "zdir", "返回后选中刚离开的目录");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(windows)]
    #[test]
    fn up_past_drive_root_shows_drives() {
        let mut b = Browser {
            loc: Loc::Dir(PathBuf::from("C:\\")),
            entries: Vec::new(),
            selected: 0,
        };
        b.up().unwrap();
        assert_eq!(b.loc, Loc::Drives);
        assert!(!b.entries.is_empty(), "至少存在 C:\\");
        assert_eq!(b.entries[b.selected].path(), Path::new("C:\\"));
        // Drives 层再向上：无操作。
        b.up().unwrap();
        assert_eq!(b.loc, Loc::Drives);
    }

    #[test]
    fn refresh_preserves_selection_by_path() {
        let dir = fixture("refresh");
        let mut b = Browser::new(&dir);
        b.selected = 3; // b.md
        // 新增一个排前面的文件，b.md 顺位后移。
        std::fs::write(dir.join("a0.md"), "x").unwrap();
        b.refresh().unwrap();
        assert_eq!(b.entries[b.selected].name(), "b.md");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reveal_locates_file_dir_and_selects_it() {
        let dir = fixture("reveal");
        // 起点在别处：adir。
        let mut b = Browser::new(&dir.join("adir"));
        b.reveal(&dir.join("b.md")).unwrap();
        assert_eq!(b.loc, Loc::Dir(dir.clone()));
        assert_eq!(b.entries[b.selected].name(), "b.md");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reveal_relative_path_works() {
        let dir = fixture("reveal-rel");
        let file = dir.join("b.md");
        // 用相对路径 reveal：临时切 cwd。先恢复 cwd 再断言，保证 panic 安全
        // （app 测试经 App::new 读取 cwd，并行时会受影响）。
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let mut b = Browser::new(&dir.join("adir"));
        let result = b.reveal(Path::new("b.md"));
        std::env::set_current_dir(prev).unwrap();
        result.unwrap();
        assert_eq!(b.entries[b.selected].path(), file.as_path());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reveal_missing_parent_fails_in_place() {
        let dir = fixture("reveal-fail");
        let mut b = Browser::new(&dir);
        let before = b.loc.clone();
        assert!(b.reveal(&dir.join("gone").join("x.md")).is_err());
        assert_eq!(b.loc, before, "失败后浏览器不动");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_stats_counts_dirs_and_md_files() {
        let dir = fixture("stats");
        let (dirs, files) = dir_stats(&dir).unwrap();
        assert_eq!((dirs, files), (2, 2), "adir/zdir 两个目录，A.MD/b.md 两个文件");
        assert!(dir_stats(&dir.join("gone")).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn move_sel_clamps() {
        let dir = fixture("clamp");
        let mut b = Browser::new(&dir);
        b.move_sel(-5);
        assert_eq!(b.selected, 0);
        b.move_sel(99);
        assert_eq!(b.selected, b.entries.len() - 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
