# 目录树浏览器实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 mdview 的文件浏览器从「递归平铺 cwd」改为「目录树浏览器」，支持任意目录导航与 Windows 跨驱动器，并修正按键提示。

**Architecture:** 新建 `src/browse.rs` 纯逻辑模块（`Browser`/`Loc`/`Entry`，单层按需加载，越根出虚拟驱动器层）；`App` 的 `files/selected` 替换为 `browser: Browser`；`ui/browser.rs` 按条目类型渲染。Spec：`docs/superpowers/specs/2026-08-01-directory-browser-design.md`。

**Tech Stack:** Rust、crossterm、ratatui；零新依赖。

**构建/测试命令（Windows/MSVC，必须用包装脚本）：**
- 测试：`cmd //c ".cargo-vc.bat test"`
- 构建：`cmd //c ".cargo-vc.bat build"`

**Commit 格式（强制）：** `<gitmoji> <type>(<scope>): <message>`，如 `✨ feat(browser): ...`。

---

### Task 0: 创建功能分支

**Files:** 无

- [ ] **Step 1: 建分支**

```bash
git checkout -b feat/directory-browser
```

Expected: `Switched to a new branch 'feat/directory-browser'`

---

### Task 1: `src/browse.rs` 骨架与 `load`

**Files:**
- Create: `src/browse.rs`
- Modify: `src/main.rs`（加 `mod browse;`）

- [ ] **Step 1: 写失败测试**

创建 `src/browse.rs`，先只写类型骨架和测试模块：

```rust
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
```

在 `src/main.rs` 的模块声明区（`mod app;` 之后）按字母序加入：

```rust
mod browse;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test browse"`
Expected: 编译错误 `cannot find function `load` in this scope`

- [ ] **Step 3: 实现 `load` 与辅助函数**

在 `src/browse.rs` 的 `Entry` impl 之后、`#[cfg(test)]` 之前加入：

```rust
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
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test browse"`
Expected: `load_dirs_first_case_insensitive_hidden_skipped ... ok`

- [ ] **Step 5: Commit**

```bash
git add src/browse.rs src/main.rs
git commit -m "✨ feat(browse): add directory loading with sorting and filtering"
```

---

### Task 2: `Browser` 导航（enter / up / refresh / move_sel）

**Files:**
- Modify: `src/browse.rs`

- [ ] **Step 1: 写失败测试**

在 `src/browse.rs` 的 `mod tests` 内追加：

```rust
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
    fn move_sel_clamps() {
        let dir = fixture("clamp");
        let mut b = Browser::new(&dir);
        b.move_sel(-5);
        assert_eq!(b.selected, 0);
        b.move_sel(99);
        assert_eq!(b.selected, b.entries.len() - 1);
        std::fs::remove_dir_all(&dir).ok();
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test browse"`
Expected: 编译错误 `cannot find value `b` ...` / `no method named `enter`` 等（`Browser`、`EnterOutcome` 未定义）

- [ ] **Step 3: 实现 `Browser` 与 `EnterOutcome`**

在 `src/browse.rs` 的 `list_drives` 之后、`#[cfg(test)]` 之前加入：

```rust
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
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test browse"`
Expected: 7 个测试全部 ok（含 `#[cfg(windows)]` 的 drives 测试）

- [ ] **Step 5: Commit**

```bash
git add src/browse.rs
git commit -m "✨ feat(browse): add navigation (enter/up/refresh) with drive support"
```

---

### Task 3: `reveal` 与 `dir_stats`

**Files:**
- Modify: `src/browse.rs`

- [ ] **Step 1: 写失败测试**

在 `mod tests` 内追加：

```rust
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
        // 用相对路径 reveal：临时切 cwd。
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let mut b = Browser::new(&dir.join("adir"));
        b.reveal(Path::new("b.md")).unwrap();
        std::env::set_current_dir(prev).unwrap();
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
```

注意：`reveal_relative_path_works` 会改进程 cwd，cargo 测试默认多线程并行——为防与其他测试竞争，该测试内完成切换并恢复即可（本 crate 其他测试不依赖 cwd，安全）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test browse"`
Expected: 编译错误 `no method named `reveal`` / `cannot find function `dir_stats``

- [ ] **Step 3: 实现 `reveal` / `absolutize` / `dir_stats`**

在 `impl Browser` 内（`refresh` 之后）加入：

```rust
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
```

在 `impl Browser` 之后加入自由函数：

```rust
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
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test browse"`
Expected: 全部 ok（11 个测试）

- [ ] **Step 5: Commit**

```bash
git add src/browse.rs
git commit -m "✨ feat(browse): add reveal and dir_stats"
```

---

### Task 4: App 接线（替换 files/selected，重绑按键）

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: 写失败测试**

在 `src/app.rs` 的 `mod tests` 内追加：

```rust
    /// 造临时目录：sub/ 子目录 + a.md 文件，返回目录路径。
    fn temp_browser_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-app-key-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "hello").unwrap();
        dir
    }

    #[test]
    fn o_on_file_opens_reader() {
        let dir = temp_browser_dir("open");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir);
        app.browser.selected = 1; // a.md（目录优先，sub 在 0）
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('o')));
        assert!(matches!(app.mode, Mode::Reader));
        assert_eq!(app.reader.as_ref().unwrap().path, dir.join("a.md"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn o_on_dir_enters_it() {
        let dir = temp_browser_dir("enter");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir);
        app.browser.selected = 0; // sub
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('o')));
        assert!(matches!(app.mode, Mode::Browser), "进目录不打开阅读器");
        assert_eq!(app.browser.loc, Loc::Dir(dir.join("sub")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enter_and_l_are_unbound_in_browser() {
        let dir = temp_browser_dir("unbound");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir);
        handle_key(&mut app, KeyEvent::from(KeyCode::Enter));
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('l')));
        assert!(matches!(app.mode, Mode::Browser));
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn left_and_backspace_go_up() {
        let dir = temp_browser_dir("up");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir.join("sub"));
        handle_key(&mut app, KeyEvent::from(KeyCode::Left));
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        app.browser = Browser::new(&dir.join("sub"));
        handle_key(&mut app, KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_reader_reveals_file_in_browser() {
        let (dir, file) = temp_doc("reveal-open", 5);
        let mut app = test_app(10, 24);
        app.open_reader(file.clone(), 80, 0);
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        assert_eq!(app.browser.entries[app.browser.selected].path(), file.as_path());
        std::fs::remove_dir_all(&dir).ok();
    }
```

同时在 `mod tests` 顶部补导入：

```rust
    use crate::browse::{Browser, Loc};
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test app"`
Expected: 编译错误 `no field `browser` on type `App`` 等

- [ ] **Step 3: 改造 `App`**

`src/app.rs` 的改动：

1. 顶部导入区新增（`std::path::{Path, PathBuf}` 保留，`render_file` 等仍在用）：

```rust
use crate::browse::{Browser, EnterOutcome};
```

注意：`Loc` 只在测试代码中使用，不要导入主代码（零警告要求）；测试模块已通过 Step 1 的 `use crate::browse::{Browser, Loc};` 导入。

2. `App` 结构体：删除 `pub files: Vec<PathBuf>` 与 `pub selected: usize`，改为：

```rust
    pub browser: Browser,
```

3. `App::new`：删除 `files: scan_files(Path::new("."))` 与 `selected: 0`，改为：

```rust
            browser: Browser::from_cwd(),
```

4. `open_reader` 开头（`let rendered = ...` 之前）加入：

```rust
        // 浏览器同步定位到所打开的文件（best-effort）。
        if let Err(msg) = self.browser.reveal(&path) {
            self.status = Some(msg);
        }
```

5. 整个删除 `scan_files` 函数（含其 doc 注释）。

6. `browser_key` 整体替换为：

```rust
fn browser_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.browser.move_sel(1),
        KeyCode::Char('k') | KeyCode::Up => app.browser.move_sel(-1),
        KeyCode::Char('o') => match app.browser.enter() {
            EnterOutcome::OpenFile(path) => {
                app.preview = None;
                app.open_reader(path, app.max_width as u16, 0);
            }
            EnterOutcome::Failed(msg) => app.status = Some(msg),
            EnterOutcome::Entered => app.preview = None,
            EnterOutcome::Noop => {}
        },
        KeyCode::Left | KeyCode::Backspace => {
            if let Err(msg) = app.browser.up() {
                app.status = Some(msg);
            }
            app.preview = None;
        }
        KeyCode::Char('r') => {
            if let Err(msg) = app.browser.refresh() {
                app.status = Some(msg);
            }
            app.preview = None;
        }
        _ => {}
    }
}
```

7. `handle_mouse` 两个 Browser 分支替换为：

```rust
            Mode::Browser => app.browser.move_sel(3),
```

和

```rust
            Mode::Browser => app.browser.move_sel(-3),
```

8. 检查残留：`grep -n "scan_files\|app.files\|app.selected" src/app.rs` 应为空（`self.browser` 新引用除外）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 ok；`src/ui/browser.rs` 暂时编译错误——若报错先完成 Task 5 的 Step 3 再回来跑（两个任务在同一编译单元，可交换顺序；建议先改 UI 再跑测试）。

> 说明：Rust 按 crate 整体编译，`ui/browser.rs` 仍引用 `app.files` 会编译失败。本任务与 Task 5 必须一起通过编译；执行时先做 Task 5 Step 3 的 UI 改动，再统一跑 `cmd //c ".cargo-vc.bat test"`。

- [ ] **Step 5: Commit（与 Task 5 合并提交或分开均可）**

```bash
git add src/app.rs src/ui/browser.rs src/ui/mod.rs
git commit -m "✨ feat(app): wire directory browser into app and rebind keys"
```

---

### Task 5: UI 渲染（browser.rs 重写 + 帮助/状态栏文案）

**Files:**
- Modify: `src/ui/browser.rs`（整体重写）
- Modify: `src/ui/mod.rs`（帮助按键表）

- [ ] **Step 1: 准备**

无新测试（渲染为视觉效果，逻辑已被 Task 1–4 覆盖）；本任务靠编译 + 全量测试 + 人工运行验证。

- [ ] **Step 2: N/A**

- [ ] **Step 3: 重写 `src/ui/browser.rs`**

完整替换为：

```rust
//! Two-pane browser: entry list + live preview.

use super::{accent_style, chrome_style, convert, dim_style, status_bar};
use crate::app::App;
use crate::browse::{dir_stats, Entry, Loc};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(chunks[0]);

    // Left: entry list. 目录加 ▸ 前缀与 / 后缀；驱动器根无文件名，只加前缀。
    let items: Vec<ListItem> = app
        .browser
        .entries
        .iter()
        .map(|e| match e {
            Entry::Dir(p) if p.file_name().is_some() => {
                ListItem::new(format!("▸ {}/", e.name()))
            }
            Entry::Dir(_) => ListItem::new(format!("▸ {}", e.name())),
            Entry::File(_) => ListItem::new(e.name()),
        })
        .collect();
    let mut state = ListState::default();
    if !app.browser.entries.is_empty() {
        state.select(Some(app.browser.selected));
    }
    let path_text = match &app.browser.loc {
        Loc::Dir(p) => truncate_left(
            &p.display().to_string(),
            panes[0].width.saturating_sub(4) as usize,
        ),
        Loc::Drives => "drives".to_string(),
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {path_text} "))
                .border_style(chrome_style(app)),
        )
        .highlight_style(accent_style(app))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, panes[0], &mut state);

    // Right: preview. 文件 → 渲染预览；目录 → 统计；空目录/驱动器层 → 提示。
    let preview_inner = panes[1].width.saturating_sub(2);
    let selected = app.browser.entries.get(app.browser.selected).cloned();
    let lines: Vec<Line> = match &selected {
        Some(Entry::File(path)) => {
            let valid = app.preview.as_ref().is_some_and(|(p, w, theme, _)| {
                p == path && *w == preview_inner && *theme == app.scheme.name
            });
            if !valid {
                let rendered = app.render_file(path, preview_inner, 0);
                app.preview =
                    Some((path.clone(), preview_inner, app.scheme.name.clone(), rendered));
            }
            app.preview
                .as_ref()
                .map(|(_, _, _, r)| convert(app, r))
                .unwrap_or_default()
        }
        Some(Entry::Dir(path)) => match dir_stats(path) {
            Some((d, f)) => vec![
                Line::from(Span::styled(format!("{d} subdirectories"), dim_style(app))),
                Line::from(Span::styled(format!("{f} markdown files"), dim_style(app))),
            ],
            None => vec![Line::from(Span::styled(
                "cannot read this directory",
                dim_style(app),
            ))],
        },
        None if matches!(app.browser.loc, Loc::Drives) => vec![Line::from(Span::styled(
            "select a drive and press o",
            dim_style(app),
        ))],
        None => vec![Line::from(Span::styled(
            "no markdown files or subdirectories here",
            dim_style(app),
        ))],
    };
    let title = selected
        .as_ref()
        .map(|e| format!(" {} ", e.path().display()))
        .unwrap_or_else(|| " preview ".to_string());
    let preview = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(chrome_style(app)),
    );
    frame.render_widget(preview, panes[1]);

    status_bar(
        frame,
        app,
        chunks[1],
        "o open · ←/Bksp up · r refresh · t theme · ? help",
        "q quit",
    );
}

/// 路径超宽时左侧截断，保留尾部并加 … 前缀。
fn truncate_left(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let tail: String = s.chars().rev().take(keep).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}
```

- [ ] **Step 4: 更新 `src/ui/mod.rs` 帮助按键表**

把 `draw_help` 中的 `keys` 数组整体替换为：

```rust
    let keys = [
        ("j/k, ↓/↑", "move / scroll"),
        ("o", "open file / enter dir (browser)"),
        ("←, Bksp", "parent directory (browser)"),
        ("Esc", "back (reader) / quit (browser)"),
        ("d/u, PgDn/PgUp", "half page down/up"),
        ("Ctrl+f/b", "page forward / back"),
        ("g/G", "top / bottom"),
        ("/, n/N", "search / next match"),
        ("t", "theme picker"),
        ("a", "toggle align (reader)"),
        ("r", "refresh directory (browser)"),
        ("q", "quit"),
        ("?", "toggle this help"),
    ];
```

- [ ] **Step 5: 全量测试 + 零警告构建**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部测试 ok，无 warning

- [ ] **Step 6: Commit（若 Task 4 Step 5 未提交则一起）**

```bash
git add src/ui/browser.rs src/ui/mod.rs
git commit -m "✨ feat(ui): render directory browser with path title and hints"
```

---

### Task 6: 文档同步与整体验证

**Files:**
- Modify: `README.md:33`
- Modify: `README.zh-CN.md`（按键表）

- [ ] **Step 1: 更新 `README.md` 按键行**

把第 33 行：

```
Keys: `j/k` scroll · `d/u` half page · `Ctrl+f/b` full page · `g/G` top/bottom · `/` `n/N` search · `t` themes · `a` align (reader) · `Enter` open · `Esc` back · `?` help · `q` quit
```

替换为：

```
Keys: `j/k` scroll · `d/u` half page · `Ctrl+f/b` full page · `g/G` top/bottom · `/` `n/N` search · `t` themes · `a` align (reader) · `o` open / enter dir · `←/Bksp` parent dir · `Esc` back · `?` help · `q` quit
```

- [ ] **Step 2: 更新 `README.zh-CN.md` 按键表**

把这三行：

```
| `Enter/l` | 打开文件（浏览器） |
| `Esc` | 返回浏览器 |
| `r` | 重新扫描文件（浏览器） |
```

替换为：

```
| `o` | 打开文件 / 进入目录（浏览器） |
| `←` `Backspace` | 返回上级目录（浏览器，越过盘符根显示驱动器列表） |
| `Esc` | 返回浏览器（阅读器）/ 退出（浏览器） |
| `r` | 刷新当前目录（浏览器） |
```

- [ ] **Step 3: 全量测试 + 分发构建 + 人工冒烟**

```bash
cmd //c ".cargo-vc.bat test"
cmd //c "build.bat"
```

Expected: 测试全 ok、零警告；`bin/mdview.exe` 重新生成。

人工冒烟（执行者若有终端可交互环境，否则由用户验证）：
- `bin/mdview.exe` 无参启动 → 浏览器显示 cwd 内容，标题为完整路径；
- `o` 进目录、`←` 返回；在盘符根再 `←` → 驱动器列表，`o` 进其他盘；
- `o` 打开 md → `Esc` 返回 → 定位到该文件并选中；
- `?` 帮助与底部状态栏文案与新按键一致。

- [ ] **Step 4: Commit**

```bash
git add README.md README.zh-CN.md
git commit -m "📝 docs(readme): update key bindings for directory browser"
```

---

## 自审记录

- Spec 覆盖：需求决策 12 条均映射到 Task 1–6（模型→T1，导航→T2，reveal/stats→T3，按键/Esc reveal/删 scan_files→T4，UI/路径标题/预览统计/提示修正→T5，README→T6）。
- 占位符：无 TBD/TODO；Task 5 Step 2 标 N/A 是有意留空（该任务无独立测试步骤，原因已在 Step 1 说明）。
- 类型一致性：`Loc`/`Entry`/`Browser`/`EnterOutcome`/`load`/`dir_stats`/`reveal`/`absolutize` 在各任务间签名一致；`Loc` 只在 app.rs 测试模块导入以避免未用警告。
- 已知折衷：Task 4 与 Task 5 同编译单元，需一起通过编译（Task 4 Step 4 已注明执行顺序）。
