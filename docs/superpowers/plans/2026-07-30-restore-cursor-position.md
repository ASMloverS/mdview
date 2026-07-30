# 恢复上次光标位置 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TUI 阅读器关闭文件时（Esc/q/Ctrl+C）把光标行写入 exe 旁的 `history.toml`，再次打开同一文件时恢复光标位置；LRU 上限由 `config.toml` 的 `history_size` 控制（默认 200，0 禁用）。

**Architecture:** 新增 `src/history.rs`（`History`：MRU 在前的 `Vec<Entry>`，记录即写盘，best-effort），与 `config.rs` 并列。`App` 持有 `History` 实例与 `history_size`；`open_reader` 查历史恢复 `cursor`/`scroll`，`reader_key` 的三个关闭分支调 `save_position` 落盘。

**Tech Stack:** Rust、serde/toml、ratatui/crossterm；Windows/MSVC 下构建测试一律用 `cmd //c ".cargo-vc.bat test"`。

**关键事实（已勘察）：**

- 规格：`docs/superpowers/specs/2026-07-30-restore-cursor-position-design.md`（已确认）。
- `Reader { path, rendered, width, offset, scroll, cursor, view_height }` 在 `src/app.rs:25-33`；`open_reader` 在 `src/app.rs:84-97`。
- `reader_key` 关闭分支在 `src/app.rs:428-429`（q / Esc）和 `src/app.rs:446`（Ctrl+C）。
- `App::new` 调用点仅三处：`src/app.rs:246`（event_loop）、`src/app.rs:491`（test_app）、`src/ui/reader.rs:105`（test_app）。
- `app::run` 仅 `src/main.rs:79` 调用；`event_loop` 仅 `run` 调用。
- `config.rs` 的 `config_path()`（`src/config.rs:48-53`）是 exe 旁路径解析的既有模式，`history.rs` 复刻。
- 提交信息格式（强制）：`<gitmoji> <type>(<scope>): <message>`。

---

### Task 1: `src/history.rs` 历史存储模块

**Files:**
- Create: `src/history.rs`
- Modify: `src/main.rs:5-10`（mod 声明区）

- [ ] **Step 1: 声明模块并写失败测试**

`src/main.rs` 的 mod 声明区（`mod config;` 之后）加一行：

```rust
mod history;
```

创建 `src/history.rs`，先只写测试：

```rust
//! Per-file reading position history: `history.toml` next to the executable.

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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test history"`
Expected: 编译失败，`History`、`canonical` 未定义。

- [ ] **Step 3: 实现**

`src/history.rs` 顶部（`mod tests` 之前）加入：

```rust
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
```

注意：本 Task 结束后 `load`/`get`/`DEFAULT_HISTORY_SIZE` 等尚无调用方，会有 dead_code 警告；Task 2 接线后消除，属预期中间状态。

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test history"`
Expected: 4 个测试 PASS（dead_code 警告此时存在，忽略）。

- [ ] **Step 5: 提交**

```bash
git add src/history.rs src/main.rs
git commit -m "✨ feat(history): add per-file cursor position history store"
```

---

### Task 2: `history_size` 配置键与 App 接线

**Files:**
- Modify: `src/config.rs:34-44`（Config 结构体）
- Modify: `src/app.rs:35-76`（App 结构体与 App::new）、`src/app.rs:204-252`（run/event_loop）、`src/app.rs:489-506`（test_app）
- Modify: `src/ui/reader.rs:103-105`（test_app）
- Modify: `src/main.rs:79`（run 调用）

- [ ] **Step 1: 写失败测试**

`src/config.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn parses_history_size() {
        let cfg: Config = toml::from_str("history_size = 50\n").unwrap();
        assert_eq!(cfg.history_size, Some(50));
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.history_size, None);
    }
```

`src/app.rs` 的 `test_app` 与 `src/ui/reader.rs` 的 `test_app` 中 `App::new(scheme, ColorLevel::True, 100, ContentAlign::Center)` 均改为五参形式（末尾加 `, 0`）——此时编译失败即为预期。

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 编译失败，`App::new` 参数数量不匹配、`history_size` 未定义。

- [ ] **Step 3: 实现**

`src/config.rs` 的 `Config` 结构体加字段（放在 `align` 之后）：

```rust
    /// Max entries kept in history.toml (0 disables position restore).
    pub history_size: Option<usize>,
```

`src/app.rs`：

1. import 区加 `use crate::history::History;`。
2. `App` 结构体加字段（放在 `align` 之后）：

```rust
    pub history: History,
    pub history_size: usize,
```

3. `App::new` 签名加 `history_size: usize` 参数，初始化加：

```rust
            history: History::load(),
            history_size,
```

4. `run(...)` 签名末尾加 `history_size: usize`，透传给 `event_loop`；`event_loop` 签名同样加 `history_size: usize`，传给 `App::new`。
5. `src/app.rs` 的 `test_app`：`App::new(scheme, ColorLevel::True, 100, ContentAlign::Center, 0)`（0 = 测试默认禁用，用例按需自行覆盖）。

`src/ui/reader.rs` 的 `test_app`：`App::new(scheme, ColorLevel::True, 100, ContentAlign::Center, 0)`。

`src/main.rs` 的 `run` 调用处改为：

```rust
    let history_size = cfg.history_size.unwrap_or(history::DEFAULT_HISTORY_SIZE);
    app::run(cli.file, scheme, level, max_width, cfg.mouse.unwrap_or(true), align, history_size)
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全量 PASS，零警告（Task 1 的 dead_code 警告应已消除）。

- [ ] **Step 5: 提交**

```bash
git add src/config.rs src/app.rs src/ui/reader.rs src/main.rs
git commit -m "✨ feat(config): add history_size key and wire History into App"
```

---

### Task 3: 打开时恢复、关闭时保存

**Files:**
- Modify: `src/app.rs:84-97`（open_reader）、`src/app.rs:288-294` 附近（新增 save_position）、`src/app.rs:427-446`（reader_key 三个分支）

- [ ] **Step 1: 写失败测试**

`src/app.rs` 的 `mod tests` 中追加 helper 与用例：

```rust
    /// 造一个临时 md 文件（lines 个单段段落），返回 (目录, 文件路径)。
    fn temp_doc(tag: &str, lines: usize) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdview-app-hist-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.md");
        let text = (0..lines)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        std::fs::write(&file, text).unwrap();
        (dir, file)
    }

    #[test]
    fn open_reader_restores_cursor_from_history() {
        let (dir, file) = temp_doc("restore", 40);
        let mut app = test_app(10, 24);
        app.history_size = 200;
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 30, 200);
        app.history = h;
        app.open_reader(file, 80, 0);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 30);
        assert_eq!(r.scroll, 30 - 24 / 2, "光标大致居中（view_height 占位 24）");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_reader_without_record_starts_at_top() {
        let (dir, file) = temp_doc("fresh", 10);
        let mut app = test_app(10, 24);
        app.history_size = 200;
        app.open_reader(file, 80, 0);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 0);
        assert_eq!(r.scroll, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_reader_clamps_recorded_line() {
        let (dir, file) = temp_doc("clamp", 3);
        let mut app = test_app(10, 24);
        app.history_size = 200;
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 999, 200);
        app.history = h;
        app.open_reader(file, 80, 0);
        let r = app.reader.as_ref().unwrap();
        let last = r.rendered.lines.len() - 1;
        assert_eq!(r.cursor, last);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn esc_saves_cursor_position() {
        let (dir, file) = temp_doc("save", 40);
        let hist_path = dir.join("history.toml");
        let mut app = test_app(10, 24);
        app.history_size = 200;
        app.history = History::load_from(&hist_path);
        app.open_reader(file.clone(), 80, 0);
        app.reader.as_mut().unwrap().cursor = 7;
        handle_key(&mut app, KeyEvent::from(KeyCode::Esc));
        let h = History::load_from(&hist_path);
        assert_eq!(h.get(&file), Some(7));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn history_size_zero_disables_save_and_restore() {
        let (dir, file) = temp_doc("disabled", 40);
        let hist_path = dir.join("history.toml");
        let mut app = test_app(10, 24); // test_app 中 history_size = 0
        app.history = History::load_from(&hist_path);
        app.open_reader(file.clone(), 80, 0);
        app.reader.as_mut().unwrap().cursor = 7;
        handle_key(&mut app, KeyEvent::from(KeyCode::Esc));
        let h = History::load_from(&hist_path);
        assert_eq!(h.get(&file), None);
        std::fs::remove_dir_all(&dir).ok();
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test app"`
Expected: `esc_saves_cursor_position` 等 FAIL（尚不保存/不恢复）。

- [ ] **Step 3: 实现**

`src/app.rs`：

1. `open_reader` 改为：

```rust
    pub fn open_reader(&mut self, path: PathBuf, width: u16, offset: u16) {
        let rendered = self.render_file(&path, width, offset);
        let mut reader = Reader {
            path,
            rendered,
            width,
            offset,
            scroll: 0,
            cursor: 0,
            view_height: 24,
        };
        // 恢复上次关闭时的光标行（history_size = 0 禁用）。
        if self.history_size > 0 {
            if let Some(line) = self.history.get(&reader.path) {
                let last = reader.rendered.lines.len().saturating_sub(1);
                reader.cursor = line.min(last);
                reader.scroll = reader.cursor.saturating_sub(reader.view_height / 2);
            }
        }
        self.reader = Some(reader);
        self.search_matches.clear();
        self.mode = Mode::Reader;
    }
```

2. `follow_cursor` 之后新增：

```rust
/// 关闭阅读器时记录当前光标行（best-effort；history_size = 0 禁用）。
fn save_position(app: &mut App) {
    if app.history_size == 0 {
        return;
    }
    let Some((path, line)) = app.reader.as_ref().map(|r| (r.path.clone(), r.cursor)) else {
        return;
    };
    app.history.record(&path, line, app.history_size);
}
```

3. `reader_key` 的三个关闭分支改为：

```rust
        KeyCode::Char('q') => {
            save_position(app);
            app.quit = true;
        }
        KeyCode::Esc => {
            save_position(app);
            app.mode = Mode::Browser;
        }
```

以及：

```rust
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            save_position(app);
            app.quit = true;
        }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全量 PASS，零警告。

- [ ] **Step 5: 提交**

```bash
git add src/app.rs
git commit -m "✨ feat(reader): restore and save cursor position across opens"
```

---

### Task 4: 文档与分发配置

**Files:**
- Modify: `build.bat:44-58`（`:write_config` 段）
- Modify: `bin/config.toml`
- Modify: `README.md:45-50`
- Modify: `README.zh-CN.md:70-75`
- Modify: `AGENTS.md`

- [ ] **Step 1: 更新 `build.bat` 与 `bin/config.toml`**

`build.bat` 的 `:write_config` 段在 `# align` 两行之后、`# mouse` 之前插入：

```bat
>> "%CFG%" echo # Reading position history: remember cursor line per file (0 disables)
>> "%CFG%" echo # history_size = 200
>> "%CFG%" echo.
```

`bin/config.toml`（已存在，build.bat 不会覆盖）追加：

```toml

# Reading position history: remember cursor line per file (0 disables)
# history_size = 200
```

- [ ] **Step 2: 更新 `README.md`**

Config 代码块（第 45-50 行）改为：

```toml
theme = "gruvbox-dark"  # written automatically when the picker closes
max_width = 100
mouse = true
align = "center"        # or "left"; written automatically on 'a' toggle
history_size = 200      # remember cursor line per file (history.toml; 0 disables)
```

- [ ] **Step 3: 更新 `README.zh-CN.md`**

配置代码块（第 70-75 行）改为：

```toml
theme = "gruvbox-dark"  # 主题选择器关闭时自动写入
max_width = 100
mouse = true
align = "center"        # 或 "left"；阅读器内按 a 切换时自动写入
history_size = 200      # 记住每个文件的光标行（history.toml；0 禁用）
```

- [ ] **Step 4: 更新 `AGENTS.md`**

Architecture 的文件清单中 `src/app.rs, src/ui/` 一行之前插入：

```
- `src/history.rs` — per-file cursor position history (LRU, exe-adjacent `history.toml`)
```

Conventions 中的 Config 条目改为：

```
- Default theme: `gruvbox-dark`. Config: `config.toml` next to the
  executable (theme persisted on picker close, `align` persisted on
  reader `a` toggle; preserve other keys when writing). Reading
  positions: `history.toml` next to the executable (LRU capped by
  `history_size`, 0 disables).
```

- [ ] **Step 5: 全量测试 + 提交**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全量 PASS，零警告。

```bash
git add build.bat bin/config.toml README.md README.zh-CN.md AGENTS.md
git commit -m "📝 docs: document history_size and cursor position restore"
```

---

## 自审记录

- **规格覆盖**：history.toml 数据格式 ✓（Task 1，`[[history]]` 数组 MRU 在前）；load/get/record + canonicalize ✓（Task 1）；LRU cap ✓（Task 1 truncate + Task 3 传 `history_size`）；`history_size` 配置键默认 200、0 禁用 ✓（Task 2 main.rs 回退 + Task 3 分支守卫）；open_reader 恢复 cursor + scroll 居中 ✓（Task 3 Step 3.1）；Esc/q/Ctrl+C 三个落盘时机 ✓（Task 3 Step 3.3）；best-effort IO ✓（Task 1 save）；测试清单 ✓（各 Task Step 1）；pipe 模式不涉及 ✓（无任务触碰管道分支）。
- **占位符扫描**：无 TBD/TODO；所有代码步骤含完整代码。
- **类型一致性**：`History::{load, load_from, get, record}`、`canonical`、`DEFAULT_HISTORY_SIZE`、`Entry { path, line }`、`App::new(scheme, level, max_width, align, history_size)`、`run(..., mouse, align, history_size)`、`save_position(app)` 在各 Task 间一致；Task 3 的测试用 `History::load_from`/`record` 均在 Task 1 定义，`test_app` 五参形式在 Task 2 定义。
