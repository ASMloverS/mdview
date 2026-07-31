# 无参数启动恢复上次文件 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 无参数启动 mdview 时恢复上次阅读的文件（含光标位置）；无可用历史时进浏览器并弹首次使用提示。

**Architecture:** `History` 新增 `latest_valid()` 负责沿 MRU 找第一个可读文件并清理失效条目；`App` 新增 `resume_hint` 字段和 `resume_latest()` 启动分支；UI 复用 help overlay 模式绘制提示浮层。设计文档：`docs/superpowers/specs/2026-07-30-resume-last-file-design.md`。

**Tech Stack:** Rust、pulldown-cmark、ratatui、crossterm、toml/serde。

**分支:** `resume-last-file`（已创建，设计文档已提交于此）。

**构建/测试命令（Windows/MSVC，Git Bash 下）:**
- 全量测试：`cmd //c ".cargo-vc.bat test"`
- 单个测试：`cmd //c ".cargo-vc.bat test <名称过滤>"`（如 `cmd //c ".cargo-vc.bat test latest_valid"`）
- 构建须零警告。

---

### Task 1: `History::latest_valid()`

**Files:**
- Modify: `src/history.rs`（在 `record` 方法后新增方法；测试加在文件内 `mod tests`）

- [ ] **Step 1: 写失败测试**

在 `src/history.rs` 的 `mod tests` 末尾（`record_persists_and_reloads` 之后）追加：

```rust
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test latest_valid"`
Expected: 编译失败，`no method named latest_valid`

- [ ] **Step 3: 实现 `latest_valid`**

在 `src/history.rs` 的 `impl History` 中、`record` 方法之后插入：

```rust
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
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test latest_valid"`
Expected: 4 个测试全部 PASS

- [ ] **Step 5: 提交**

```bash
git add src/history.rs
git commit -m "✨ feat(history): add latest_valid for session resume"
```

---

### Task 2: `App::resume_hint` 字段 + `resume_latest()` 启动分支

**Files:**
- Modify: `src/app.rs`（`App` 结构体、`App::new`、`event_loop`；测试加在文件内 `mod tests`）

- [ ] **Step 1: 写失败测试**

在 `src/app.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn resume_latest_opens_reader_and_restores_cursor() {
        let (dir, file) = temp_doc("resume-open", 40);
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            200,
        );
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 30, 200);
        app.history = h;
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Reader));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 30);
        assert!(!app.resume_hint);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resume_latest_empty_history_shows_hint_in_browser() {
        let dir = std::env::temp_dir().join(format!("mdview-app-hist-{}-resume-hint", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            200,
        );
        app.history = History::load_from(&dir.join("history.toml"));
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Browser));
        assert!(app.resume_hint);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resume_latest_disabled_history_is_silent() {
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            0, // history_size = 0：禁用历史
        );
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Browser));
        assert!(!app.resume_hint, "禁用历史时静默进浏览器");
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test resume_latest"`
Expected: 编译失败，`no method named resume_latest`、`no field resume_hint`

- [ ] **Step 3: 实现字段与方法**

3a. `App` 结构体（`src/app.rs:53` 附近，`show_help: bool,` 之后）加字段：

```rust
    pub show_help: bool,
    pub resume_hint: bool,
```

3b. `App::new` 的初始化列表中、`show_help: false,` 之后加：

```rust
            resume_hint: false,
```

3c. 在 `open_reader` 方法之后（`src/app.rs:117` 之后）新增：

```rust
    /// 无参数启动：恢复最近可读文件（光标由 open_reader 恢复）；
    /// 无可用历史时进浏览器并弹首次使用提示。
    /// history_size = 0（禁用历史）时静默进浏览器。
    pub fn resume_latest(&mut self, width: u16, offset: u16) {
        if self.history_size == 0 {
            return;
        }
        match self.history.latest_valid() {
            Some(path) => self.open_reader(path, width, offset),
            None => self.resume_hint = true,
        }
    }
```

3d. `event_loop`（`src/app.rs:268-274`）把启动分流改为（width/offset 计算提出来共用）：

```rust
    let mut app = App::new(scheme, level, max_width, align, history_size);
    let term_w = terminal.size()?.width;
    let width = content_width(term_w, max_width);
    let offset = content_offset(term_w.saturating_sub(2), width, app.align);
    if let Some(path) = start_file {
        app.open_reader(path, width, offset);
    } else {
        app.resume_latest(width, offset);
    }
```

（原代码里 `term_w`/`width`/`offset` 只在 `if let` 分支内计算，逻辑不变，只是提升作用域。）

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test resume_latest"`
Expected: 3 个测试全部 PASS

- [ ] **Step 5: 提交**

```bash
git add src/app.rs
git commit -m "✨ feat(app): resume last file on no-arg start"
```

---

### Task 3: 首次使用提示浮层（渲染 + 任意键关闭）

**Files:**
- Modify: `src/ui/mod.rs`（`draw` 分发 + 新增 `draw_resume_hint`）
- Modify: `src/app.rs`（`handle_key` 拦截；测试加在文件内 `mod tests`）

- [ ] **Step 1: 写失败测试**

在 `src/app.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn resume_hint_dismissed_by_any_key() {
        let mut app = test_app(10, 24);
        app.resume_hint = true;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert!(!app.resume_hint);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 0, "按键被弹窗拦截，不传给阅读器");
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test resume_hint_dismissed"`
Expected: FAIL（`j` 传入阅读器，cursor 变为 1）

- [ ] **Step 3: 实现按键拦截与浮层渲染**

3a. `src/app.rs` 的 `handle_key` 中（`src/app.rs:399` 附近），在 `?` 帮助判断**之前**插入：

```rust
    if app.resume_hint {
        app.resume_hint = false;
        return;
    }
```

3b. `src/ui/mod.rs` 的 `draw` 中、`if app.show_help { ... }` 之后追加：

```rust
    if app.resume_hint {
        draw_resume_hint(frame, app);
    }
```

3c. `src/ui/mod.rs` 末尾（`draw_help` 之后）新增：

```rust
/// 首次使用提示：无最近文件可恢复时的居中浮层。
fn draw_resume_hint(frame: &mut Frame, app: &App) {
    let lines = vec![
        Line::from(Span::styled(
            "No recent file to resume.",
            chrome_style(app),
        )),
        Line::from(Span::styled(
            "Open one directly: mdview <file>",
            dim_style(app),
        )),
        Line::from(Span::styled(
            "Press any key to browse.",
            dim_style(app),
        )),
    ];
    let area = centered_rect(50, lines.len() as u16 + 2, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" welcome ")
        .border_style(accent_style(app));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test resume_hint_dismissed"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/app.rs src/ui/mod.rs
git commit -m "✨ feat(ui): first-run resume hint overlay"
```

---

### Task 4: README 行为描述同步

**Files:**
- Modify: `README.md:26`
- Modify: `README.zh-CN.md:32`

- [ ] **Step 1: 更新 `README.md`**

`README.md:26` 原行：

```bash
mdview               # file browser
```

改为：

```bash
mdview               # resume last file (file browser on first run)
```

- [ ] **Step 2: 更新 `README.zh-CN.md`**

`README.zh-CN.md:32` 原行：

```bash
mdview                # 打开文件浏览器（扫描当前目录的 .md）
```

改为：

```bash
mdview                # 恢复上次阅读的文件（首次使用进入文件浏览器）
```

- [ ] **Step 3: 提交**

```bash
git add README.md README.zh-CN.md
git commit -m "📝 docs(readme): document resume-on-start behavior"
```

---

### Task 5: 全量回归 + 收尾

**Files:** 无改动，仅验证。

- [ ] **Step 1: 全量测试**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部测试 PASS，零警告

- [ ] **Step 2: 构建验证**

Run: `cmd //c ".cargo-vc.bat build"`
Expected: 编译成功，零警告

- [ ] **Step 3: 手动冒烟（可选）**

```bash
cmd //c ".cargo-vc.bat run -- README.md"   # 打开文件，q 退出
cmd //c ".cargo-vc.bat run"                # 无参数：应直接恢复 README.md 到上次光标行
```

Expected: 无参数启动恢复上次文件；删除 history.toml 后无参数启动 → 浏览器 + welcome 浮层，任意键关闭。

---

## Self-Review 记录

- **Spec 覆盖：** latest_valid 清理策略（Task 1）✓；启动分支与 history_size=0 静默（Task 2）✓；弹窗文案/位置/仅启动一次/任意键关闭（Task 2 置位 + Task 3 渲染与拦截）✓；README 同步（Task 4）✓；pipe 模式与显式传文件不变（无改动，Task 5 回归验证）✓。
- **类型一致性：** `latest_valid(&mut self) -> Option<PathBuf>`（Task 1 定义，Task 2 调用）✓；`resume_latest(&mut self, width: u16, offset: u16)`（Task 2 定义并接线）✓；`resume_hint: bool`（Task 2 定义，Task 3 读写）✓。
- **占位符：** 无。
