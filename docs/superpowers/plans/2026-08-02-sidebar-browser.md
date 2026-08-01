# 目录侧栏（Sidebar）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把目录浏览器从独立主视图重构为「阅读器为主 + 可开关目录侧栏」：`o` 开栏、`Enter` 进目录/打开文件、`Backspace` 上级、`Tab` 切焦点、`Esc`/`q` 关栏。

**Architecture:** 删除 `Mode` 枚举；`App` 持有 `reader: Option<Reader>` + `sidebar: Option<Sidebar>`（`Sidebar { browser: Browser, focus: Focus }`）；`browse.rs` 零改动复用（仅删 `dir_stats`）；UI 改水平二分布局。Spec：`docs/superpowers/specs/2026-08-02-sidebar-browser-design.md`（分支 `feat/sidebar-browser` 已创建）。

**Tech Stack:** Rust、crossterm、ratatui；零新依赖。

**构建/测试命令（Windows/MSVC，必须用包装脚本）：**
- 测试：`cmd //c ".cargo-vc.bat test"`
- 分发构建：`cmd //c "build.bat"`

**Commit 格式（强制）：** `<gitmoji> <type>(<scope>): <message>`。

---

### Task 1: 配置项 `sidebar_width`

**Files:**
- Modify: `src/config.rs`
- Modify: `bin/config.toml`

- [ ] **Step 1: 写失败测试**

在 `src/config.rs` 的 `mod tests` 内追加：

```rust
    #[test]
    fn sidebar_width_defaults_and_clamps() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.sidebar_width(), 30);
        let cfg: Config = toml::from_str("sidebar_width = 45\n").unwrap();
        assert_eq!(cfg.sidebar_width(), 45);
        let cfg: Config = toml::from_str("sidebar_width = 3\n").unwrap();
        assert_eq!(cfg.sidebar_width(), 10, "低于下限 clamp 到 10");
        let cfg: Config = toml::from_str("sidebar_width = 99\n").unwrap();
        assert_eq!(cfg.sidebar_width(), 60, "高于上限 clamp 到 60");
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test config"`
Expected: 编译错误 `no method named 'sidebar_width'`

- [ ] **Step 3: 实现**

`src/config.rs`：`ContentAlign` 之后加常量：

```rust
/// 默认侧栏宽度（百分比）。
pub const DEFAULT_SIDEBAR_WIDTH: u16 = 30;
```

`Config` 结构体加字段（`history_size` 之后）：

```rust
    /// Sidebar width in percent (10..=60, default 30).
    pub sidebar_width: Option<u16>,
```

`impl Config` 加方法（`save_align` 之后）：

```rust
    /// 侧栏宽度百分比：缺省 30，clamp 到 10..=60。
    pub fn sidebar_width(&self) -> u16 {
        self.sidebar_width.unwrap_or(DEFAULT_SIDEBAR_WIDTH).clamp(10, 60)
    }
```

`bin/config.toml` 末尾追加：

```toml

# Sidebar width in percent (10-60, default 30)
# sidebar_width = 30
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 ok（含新测试），零警告

- [ ] **Step 5: Commit**

```bash
git add src/config.rs bin/config.toml
git commit -m "✨ feat(config): add sidebar_width option"
```

---

### Task 2: App 重构（Focus/Sidebar、按键分派、启动分流）+ Task 3: UI 重写

> 两任务同编译单元（旧 UI 引用 `Mode`/`preview`），必须一起完成才能编译，
> 合并为一个实现派发、一个 commit。

**Files:**
- Modify: `src/app.rs`（状态模型 + 按键 + 测试重写）
- Modify: `src/main.rs`（`run` 调用传 `sidebar_width`）
- Rename: `src/ui/browser.rs` → `src/ui/sidebar.rs`（`git mv`，然后重写）
- Modify: `src/ui/mod.rs`（布局 + 帮助表 + 删 resume_hint）
- Modify: `src/ui/reader.rs`（空态 + 焦点边框 + 状态栏 + 测试签名）
- Modify: `src/browse.rs`（删除 `dir_stats` 及其测试）

- [ ] **Step 1: 改造 `src/app.rs` 状态模型**

1. 删除 `Mode` 枚举、`App` 的 `mode`/`preview`/`resume_hint` 字段。新增：

```rust
/// 焦点：侧栏或阅读器。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Reader,
}

/// 目录侧栏：浏览器状态 + 焦点。
pub struct Sidebar {
    pub browser: Browser,
    pub focus: Focus,
}
```

2. `App` 结构体字段调整为（顺序保持现有风格）：

```rust
pub struct App {
    pub reader: Option<Reader>,
    pub sidebar: Option<Sidebar>,
    pub picker: Option<usize>,
    pub schemes: Vec<String>,
    pub searching: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub scheme: Scheme,
    pub level: ColorLevel,
    pub max_width: usize,
    pub align: ContentAlign,
    pub sidebar_width: u16,
    pub history: History,
    pub history_size: usize,
    pub show_help: bool,
    pub status: Option<String>,
    pub quit: bool,
}
```

3. `App::new` 增加 `sidebar_width: u16` 末参，初始化 `reader: None`、`sidebar: None`、
   `sidebar_width`，删除 `mode`/`preview`/`resume_hint`/`browser` 初始化。

4. `open_reader`：删除开头的 reveal 块（reveal 移到 `open_sidebar`）。

5. `apply_scheme`：删除 `self.preview = None;` 行。

6. `resume_latest`：删除 `resume_hint` 相关分支，doc 改为：

```rust
    /// 无参数启动：恢复最近可读文件（光标由 open_reader 恢复）。
    /// 无可用历史或禁用时什么都不做（由 start 负责开侧栏）。
    pub fn resume_latest(&mut self, width: u16, offset: u16) {
        if self.history_size == 0 {
            return;
        }
        if let Some(path) = self.history.latest_valid() {
            self.open_reader(path, width, offset);
        }
    }
```

7. 新增方法：

```rust
    /// 启动分流：有文件（CLI 参数或历史恢复）开阅读器；否则开侧栏选文件。
    pub fn start(&mut self, start_file: Option<PathBuf>, width: u16, offset: u16) {
        match start_file {
            Some(path) => self.open_reader(path, width, offset),
            None => self.resume_latest(width, offset),
        }
        if self.reader.is_none() {
            self.open_sidebar();
        }
    }

    /// 打开侧栏：定位到当前阅读文件所在目录（无文件则从 cwd 开始）。
    pub fn open_sidebar(&mut self) {
        let mut browser = Browser::from_cwd();
        if let Some(path) = self.reader.as_ref().map(|r| r.path.clone()) {
            // 定位到当前文件（best-effort）。
            if let Err(msg) = browser.reveal(&path) {
                self.status = Some(msg);
            }
        }
        self.sidebar = Some(Sidebar { browser, focus: Focus::Sidebar });
    }

    /// 当前焦点：侧栏开且焦点在侧栏 → Sidebar；否则 Reader。
    pub fn focus(&self) -> Focus {
        match &self.sidebar {
            Some(s) if matches!(s.focus, Focus::Sidebar) => Focus::Sidebar,
            _ => Focus::Reader,
        }
    }

    /// 侧栏开时切换焦点。
    pub fn toggle_focus(&mut self) {
        if let Some(s) = self.sidebar.as_mut() {
            s.focus = match s.focus {
                Focus::Sidebar => Focus::Reader,
                Focus::Reader => Focus::Sidebar,
            };
        }
    }
```

8. `run` / `event_loop` 增加 `sidebar_width: u16` 末参并传入 `App::new`；
   启动分流替换为 `app.start(start_file, width, offset);`；
   Resize 分支删除 `app.preview = None;`（保留 `reader.width = 0`）。
   `main.rs` 末参传 `cfg.sidebar_width()`。

9. `handle_key`：删除 `resume_hint` 拦截块；末尾分派改为：

```rust
    match key.code {
        KeyCode::Char('t') => {
            let idx = app
                .schemes
                .iter()
                .position(|n| n == &app.scheme.name)
                .unwrap_or(0);
            app.picker = Some(idx);
        }
        KeyCode::Tab if app.sidebar.is_some() => app.toggle_focus(),
        _ => match app.focus() {
            Focus::Sidebar => sidebar_key(app, key),
            Focus::Reader => reader_key(app, key),
        },
    }
```

10. `browser_key` 删除，替换为 `sidebar_key`：

```rust
fn sidebar_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.sidebar = None,
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(s) = app.sidebar.as_mut() {
                s.browser.move_sel(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(s) = app.sidebar.as_mut() {
                s.browser.move_sel(-1);
            }
        }
        KeyCode::Enter => {
            let outcome = app.sidebar.as_mut().map(|s| s.browser.enter());
            match outcome {
                Some(EnterOutcome::OpenFile(path)) => {
                    app.open_reader(path, app.max_width as u16, 0);
                    app.sidebar = None;
                }
                Some(EnterOutcome::Failed(msg)) => app.status = Some(msg),
                _ => {}
            }
        }
        KeyCode::Backspace => {
            if let Some(s) = app.sidebar.as_mut() {
                if let Err(msg) = s.browser.up() {
                    app.status = Some(msg);
                }
            }
        }
        KeyCode::Char('r') => {
            if let Some(s) = app.sidebar.as_mut() {
                if let Err(msg) = s.browser.refresh() {
                    app.status = Some(msg);
                }
            }
        }
        _ => {}
    }
}
```

11. `reader_key`：删除 `Esc` 分支（Esc 在阅读器不再绑定）；新增 `o`：

```rust
        KeyCode::Char('o') if app.sidebar.is_none() => app.open_sidebar(),
```

（`q` 退出等其余分支不变；`o` 放在 `q` 之后。）

12. `handle_mouse` 整体替换为：

```rust
fn handle_mouse(app: &mut App, kind: MouseEventKind) {
    let delta = match kind {
        MouseEventKind::ScrollDown => 3,
        MouseEventKind::ScrollUp => -3,
        _ => return,
    };
    match app.focus() {
        Focus::Sidebar => {
            if let Some(s) = app.sidebar.as_mut() {
                s.browser.move_sel(delta);
            }
        }
        Focus::Reader => scroll_reader(app, delta),
    }
}
```

13. 顶部导入：`use crate::browse::{Browser, EnterOutcome};` 保留（`Loc` 仍只入测试）。

- [ ] **Step 2: 重写 `src/app.rs` 测试**

1. `test_app`：`App::new(..., 0, 30)` 加末参；删除 `app.mode = Mode::Reader;`。
2. 删除旧浏览器测试（`o_on_file_opens_reader`、`o_on_dir_enters_it`、
   `enter_l_right_h_unbound_in_browser`、`left_and_backspace_go_up`、
   `open_reader_reveals_file_in_browser`）与 `temp_browser_dir` helper。
3. `resume_latest_*` 测试去 `mode`/`resume_hint` 断言：
   - `resume_latest_opens_reader_and_restores_cursor`：断言 `reader.is_some()` + cursor；
   - `resume_latest_empty_history_shows_hint_in_browser` 改名
     `resume_latest_empty_history_leaves_reader_none`，断言 `reader.is_none()`；
   - `resume_latest_disabled_history_is_silent`：断言 `reader.is_none()` 且
     `sidebar.is_none()`；
   - `resume_latest_all_stale_shows_hint_in_browser` 改名
     `resume_latest_all_stale_leaves_reader_none`，保留持久化断言；
   - 删除 `resume_hint_dismissed_by_any_key`。
4. `esc_saves_cursor_position` 改名 `quit_saves_cursor_position`：按 `q` 代替
   `Esc`，断言 `app.quit` 且历史已记。`history_size_zero_disables_save_and_restore`
   同样改按 `q`。
5. 新增 helper 与测试：

```rust
    /// 造临时目录（sub/ + a.md）和侧栏已开的 App（焦点在侧栏）。
    fn sidebar_app(tag: &str) -> (PathBuf, App) {
        let dir = std::env::temp_dir().join(format!("mdview-app-sb-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "hello").unwrap();
        let mut app = test_app(10, 24);
        app.sidebar = Some(Sidebar { browser: Browser::new(&dir), focus: Focus::Sidebar });
        (dir, app)
    }

    #[test]
    fn enter_on_file_opens_reader_and_closes_sidebar() {
        let (dir, mut app) = sidebar_app("open");
        app.sidebar.as_mut().unwrap().browser.selected = 1; // a.md
        handle_key(&mut app, KeyEvent::from(KeyCode::Enter));
        assert!(app.sidebar.is_none(), "打开文件后侧栏关闭");
        assert_eq!(app.reader.as_ref().unwrap().path, dir.join("a.md"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enter_on_dir_enters_it() {
        let (dir, mut app) = sidebar_app("enter");
        handle_key(&mut app, KeyEvent::from(KeyCode::Enter)); // selected 0 = sub
        let s = app.sidebar.as_ref().unwrap();
        assert_eq!(s.browser.loc, Loc::Dir(dir.join("sub")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backspace_goes_up() {
        let (dir, mut app) = sidebar_app("up");
        app.sidebar.as_mut().unwrap().browser = Browser::new(&dir.join("sub"));
        handle_key(&mut app, KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.sidebar.as_ref().unwrap().browser.loc, Loc::Dir(dir.clone()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tab_toggles_focus() {
        let (dir, mut app) = sidebar_app("tab");
        handle_key(&mut app, KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.focus(), Focus::Reader);
        handle_key(&mut app, KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.focus(), Focus::Sidebar);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn q_follows_focus() {
        let (dir, mut app) = sidebar_app("q");
        // 侧栏焦点：q 关侧栏。
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('q')));
        assert!(app.sidebar.is_none());
        assert!(!app.quit);
        // 阅读器焦点（侧栏开）：q 退出。
        app.open_sidebar();
        app.sidebar.as_mut().unwrap().focus = Focus::Reader;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('q')));
        assert!(app.quit);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn esc_closes_only_from_sidebar_focus() {
        let (dir, mut app) = sidebar_app("esc");
        app.sidebar.as_mut().unwrap().focus = Focus::Reader;
        handle_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.sidebar.is_some(), "阅读器焦点下 Esc 不关侧栏");
        app.sidebar.as_mut().unwrap().focus = Focus::Sidebar;
        handle_key(&mut app, KeyEvent::from(KeyCode::Esc));
        assert!(app.sidebar.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn o_opens_sidebar_and_reveals_current_file() {
        let (dir, file) = temp_doc("o-open", 5);
        let mut app = test_app(10, 24);
        app.open_reader(file.clone(), 80, 0);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('o')));
        let s = app.sidebar.as_ref().unwrap();
        assert_eq!(s.browser.loc, Loc::Dir(dir.clone()));
        assert_eq!(s.browser.entries[s.browser.selected].path(), file.as_path());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enter_unbound_in_reader_focus() {
        let (dir, mut app) = sidebar_app("reader-enter");
        app.sidebar.as_mut().unwrap().focus = Focus::Reader;
        let before = app.reader.as_ref().unwrap().path.clone();
        handle_key(&mut app, KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.reader.as_ref().unwrap().path, before);
        assert!(app.sidebar.is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn start_without_file_opens_sidebar() {
        let dir = std::env::temp_dir().join(format!("mdview-app-sb-{}-start", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = test_app(10, 24);
        app.reader = None;
        app.history_size = 200;
        app.history = History::load_from(&dir.join("history.toml"));
        app.start(None, 80, 0);
        assert!(app.reader.is_none());
        assert!(app.sidebar.is_some());
        assert_eq!(app.focus(), Focus::Sidebar);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn start_with_history_opens_reader_without_sidebar() {
        let (dir, file) = temp_doc("start-hist", 5);
        let mut app = test_app(10, 24);
        app.reader = None;
        app.history_size = 200;
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 2, 200);
        app.history = h;
        app.start(None, 80, 0);
        assert!(app.reader.is_some());
        assert!(app.sidebar.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
```

测试模块导入改为 `use crate::browse::{Browser, Loc};` + `use super::*`（`Sidebar`、`Focus`
经 `super::*` 可得）。

- [ ] **Step 3: UI 重写**

1. `git mv src/ui/browser.rs src/ui/sidebar.rs`，内容替换为：

```rust
//! Sidebar: directory list panel (focusable).

use super::{accent_style, chrome_style, dim_style, status_bar};
use crate::app::App;
use crate::browse::{Entry, Loc};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);

    let Some(sidebar) = &app.sidebar else { return };
    // 目录加 ▸ 前缀与 / 后缀；驱动器根无文件名，只加前缀。
    let items: Vec<ListItem> = sidebar
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
    if !sidebar.browser.entries.is_empty() {
        state.select(Some(sidebar.browser.selected));
    }
    let path_text = match &sidebar.browser.loc {
        Loc::Dir(p) => truncate_left(
            &p.display().to_string(),
            chunks[0].width.saturating_sub(4) as usize,
        ),
        Loc::Drives => "drives".to_string(),
    };
    let border = if focused { accent_style(app) } else { chrome_style(app) };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {path_text} "))
                .border_style(border),
        )
        .highlight_style(accent_style(app))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, chunks[0], &mut state);

    status_bar(
        frame,
        app,
        chunks[1],
        "Enter open · Bksp up · r refresh · Tab focus · ? help",
        "Esc close",
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

2. `src/ui/mod.rs`：
   - `pub mod browser;` → `pub mod sidebar;`；导入 `use crate::app::{App, Focus};`（去掉 Mode）。
   - `draw` 分派替换为：

```rust
    if let Some(focused) = app
        .sidebar
        .as_ref()
        .map(|s| matches!(s.focus, Focus::Sidebar))
    {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(app.sidebar_width),
                Constraint::Percentage(100 - app.sidebar_width),
            ])
            .split(frame.area());
        sidebar::draw(frame, app, panes[0], focused);
        reader::draw(frame, app, panes[1], !focused);
    } else {
        let area = frame.area();
        reader::draw(frame, app, area, true);
    }
```

   - 删除 `draw_resume_hint` 及其调用。
   - `draw_help` 的 `keys` 数组替换为：

```rust
    let keys = [
        ("j/k, ↓/↑", "move / scroll"),
        ("o", "open sidebar (reader)"),
        ("Tab", "switch focus (sidebar open)"),
        ("Enter", "open file / enter dir (sidebar)"),
        ("Bksp", "parent directory (sidebar)"),
        ("Esc", "close sidebar (sidebar focus)"),
        ("d/u, PgDn/PgUp", "half page down/up"),
        ("Ctrl+f/b", "page forward / back"),
        ("g/G", "top / bottom"),
        ("/, n/N", "search / next match"),
        ("t", "theme picker"),
        ("a", "toggle align (reader)"),
        ("r", "refresh directory (sidebar)"),
        ("q", "close sidebar / quit (reader)"),
        ("?", "toggle this help"),
    ];
```

3. `src/ui/reader.rs`：签名改 `pub fn draw(frame: &mut Frame, app: &mut App, area: Rect, focused: bool)`；
   内部 `frame.area()` 改用入参 `area`；边框样式 `let border = if focused { accent_style(app) } else { chrome_style(app) };`；
   reader 为 None 时渲染空态并仍画状态栏：

```rust
    // 空态：无打开文件时的提示。
    if app.reader.is_none() {
        let hint = if app.sidebar.is_some() {
            "Select a markdown file from the sidebar"
        } else {
            "Press o to open the sidebar"
        };
        let widget = Paragraph::new(Line::from(Span::styled(hint, dim_style(app)))).block(
            Block::default().borders(Borders::ALL).border_style(border),
        );
        frame.render_widget(widget, view);
        draw_status_bar(frame, app, chunks[1]);
        return;
    }
```

   状态栏提取为 `fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect)`，非搜索态文案：
   左侧 `format!("o sidebar · / search · t theme · ? help{matches}")`，右侧
   `if app.sidebar.is_some() { "Tab focus · q quit" } else { "q quit" }`（渲染逻辑沿用现状，
   含 `app.status` 追加 span）。
   - 文件头注释改为 `//! Reader view with scroll and search; empty-state hint when no file is open.`
   - 测试：`test_app` 删 `app.mode = Mode::Reader;`、`App::new` 加末参 `30`，删
     `use crate::app::{Mode, Reader};` 中的 `Mode`；`terminal.draw` 调用改为：

```rust
        terminal
            .draw(|f| {
                let area = f.area();
                draw(f, &mut app, area, true);
            })
            .unwrap();
```

4. `src/browse.rs`：删除 `dir_stats` 函数与 `dir_stats_counts_dirs_and_md_files` 测试。

- [ ] **Step 4: 全量测试 + 零警告构建**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 ok，零警告。残留检查：
`grep -rn "Mode\|preview\|resume_hint\|dir_stats\|browser::draw" src/` 只剩
`browse.rs` 模块本身与 `Browser` 类型引用。

- [ ] **Step 5: Commit**

```bash
git add -A src/
git commit -m "✨ feat(sidebar): rework directory browser as focusable sidebar"
```

---

### Task 4: 文档同步与整体验证

**Files:**
- Modify: `README.md`（第 33 行 Keys）
- Modify: `README.zh-CN.md`（按键表）
- Modify: `AGENTS.md`（仅在发现过时描述时）

- [ ] **Step 1: `README.md` 第 33 行替换为**

```
Keys: `j/k` scroll · `d/u` half page · `Ctrl+f/b` full page · `g/G` top/bottom · `/` `n/N` search · `t` themes · `a` align · `o` sidebar · `Tab` focus · `Enter` open / enter dir (sidebar) · `Bksp` parent dir · `?` help · `q` quit
```

- [ ] **Step 2: `README.zh-CN.md` 按键表相关行**

把这四行：

```
| `o` | 打开文件 / 进入目录（浏览器） |
| `←` `Backspace` | 返回上级目录（浏览器，越过盘符根显示驱动器列表） |
| `Esc` | 返回浏览器（阅读器）/ 退出（浏览器） |
| `r` | 刷新当前目录（浏览器） |
```

替换为：

```
| `o` | 打开目录侧栏（阅读器） |
| `Tab` | 侧栏 ↔ 阅读器切换焦点 |
| `Enter` | 打开文件 / 进入目录（侧栏） |
| `Backspace` | 返回上级目录（侧栏，越过盘符根显示驱动器列表） |
| `Esc` | 关闭侧栏（侧栏焦点） |
| `r` | 刷新当前目录（侧栏） |
| `q` | 关闭侧栏（侧栏焦点）/ 退出（阅读器焦点） |
```

并删除原表尾单独的 `` `q` | 退出 `` 行（已被上面合并行覆盖）。同时把
Config 一节若有按键相关描述同步检查。

- [ ] **Step 3: AGENTS.md 检查**

`src/browse.rs` 一行描述（directory browser logic: single-level load,
sorting/filtering, navigation, Windows drive list）仍然准确则不动；
`src/app.rs, src/ui/` 一行改为 `src/app.rs`, `src/ui/` — TUI state machine,
sidebar + reader views。若无其他过时描述（grep `browser`/`Mode`/`preview`），
不做额外改动。

- [ ] **Step 4: 全量测试 + 分发构建 + 冒烟**

```bash
cmd //c ".cargo-vc.bat test"
cmd //c "build.bat"
```

Expected: 测试全 ok、零警告；`bin/mdview.exe` 重新生成。

人工冒烟（由用户最终验证）：
- `bin/mdview.exe` 无参（有历史）→ 直接阅读器；`o` 开侧栏并定位到当前文件；
- 侧栏 `Enter` 进目录、`Backspace` 上级、越根出驱动器列表；
- 选中 md `Enter` → 关栏渲染文档；`Tab` 切焦点，边框高亮跟随；
- 侧栏焦点 `Esc`/`q` 关栏；阅读器焦点 `q` 退出；
- 清空 history 后启动 → 自动开侧栏 + 右侧提示文本。

- [ ] **Step 5: Commit**

```bash
git add README.md README.zh-CN.md AGENTS.md
git commit -m "📝 docs(readme): update keys for sidebar model"
```

---

## 自审记录

- Spec 覆盖：需求决策 11 条逐条映射（布局/宽度配置/启动分流/关闭方式/q 随焦点/
  Tab 焦点/Backspace/辅助键/滚轮/清理/空态提示 → Task 1–4）。
- 占位符：无 TBD；Step 2 测试重写给出了逐条改写指令 + 全部新测试完整代码。
- 类型一致性：`Focus`/`Sidebar`/`App::focus`/`open_sidebar`/`toggle_focus`/`start`
  在 Task 2 定义，Task 3 UI 引用一致（`ui/mod.rs` 用 `Focus::Sidebar` 判断）；
  `draw(frame, app, area, focused)` 签名在 sidebar.rs/reader.rs/mod.rs 三处一致。
- 已知折衷：Task 1 的 `sidebar_width()` 在 Task 2 才接入 `main.rs`，期间
  `cargo build` 可能有 dead_code 警告（`cargo test` 构建中测试已引用，无警告）；
  零警告门禁以每任务的 `cmd //c ".cargo-vc.bat test"` 与最终的 `build.bat` 为准。
