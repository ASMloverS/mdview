# mdview 阅读器 Ctrl+f/b 整屏翻页 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阅读器新增 vim 风格整屏翻页：Ctrl+f 向前、Ctrl+b 向后，幅度 view_height-2（小窗口退化 1 行），附单元测试。

**Architecture:** 在 `src/app.rs` `reader_key` 增加两个带 CONTROL 修饰符的按键分支；幅度计算抽为 `page_delta()` 纯函数以便测试；帮助面板加一行说明。

**Tech Stack:** Rust 2021, crossterm 0.28 (KeyEvent/KeyCode/KeyModifiers)。

**Spec:** `docs/superpowers/specs/2026-07-26-mdview-ctrl-fb-paging-design.md`

**构建/测试命令（Windows，必须走 .cargo-vc.bat）：**

```bash
cmd //c ".cargo-vc.bat test"          # 全部测试
cmd //c ".cargo-vc.bat test <name>"   # 单个测试
```

**Baseline:** master 分支，27 个测试全绿，0 警告。工作分支 `feature/ctrl-fb-paging`。

---

### Task 1: Ctrl+f/b 整屏翻页 + 测试

**Files:**
- Modify: `src/app.rs`（`page_delta` 函数、`reader_key` 两个分支、新增 `#[cfg(test)] mod tests`）
- Modify: `src/ui/mod.rs`（`draw_help` 加一行）

**背景信息（实现者无需再读其他代码）：**

- `reader_key(app, key)` 现有结构（app.rs:367 起）：先算 `let page = app.reader.as_ref().map(|r| r.view_height / 2).unwrap_or(10) as isize;`，然后 `match key.code`；已有带修饰符的先例 `KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.quit = true`。
- `scroll_reader(app, delta: isize)`（app.rs:262）负责夹取：`max = lines.len().saturating_sub(view_height)`，`clamp(0, max)`。
- `handle_key(app, key)` 是模块内私有函数，搜索模式和帮助模式会优先吞键——测试构造阅读器模式、非搜索状态即可直接调用。
- `App::new(scheme, level, max_width)`；`crate::style::{Scheme, ColorLevel}`，`Scheme::load(crate::style::DEFAULT_THEME)`，`ColorLevel::True`。
- `Reader` 字段（app.rs:24）：`path: PathBuf, rendered: Rendered, width: u16, offset: u16, scroll: usize, view_height: usize`。
- `Rendered`（src/render/mod.rs）derive Default，字段 `lines: Vec<SLine>, plain: Vec<String>`（`SLine = Vec<SSpan>`）。
- `draw_help`（src/ui/mod.rs:108）的 keys 数组现有 10 行，`("d/u, PgDn/PgUp", "half page down/up")` 在其中。

- [ ] **Step 1: 写失败测试**

在 `src/app.rs` 文件末尾新增：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rendered;
    use crate::style::{ColorLevel, Scheme};

    fn test_app(lines: usize, view_height: usize) -> App {
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let mut app = App::new(scheme, ColorLevel::True, 100);
        app.mode = Mode::Reader;
        app.reader = Some(Reader {
            path: PathBuf::from("test.md"),
            rendered: Rendered {
                lines: vec![Vec::new(); lines],
                plain: vec![String::new(); lines],
            },
            width: 80,
            offset: 0,
            scroll: 0,
            view_height,
        });
        app
    }

    fn ctrl(key: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(key), KeyModifiers::CONTROL)
    }

    #[test]
    fn page_delta_full_screen_minus_overlap() {
        assert_eq!(page_delta(24), 22);
        assert_eq!(page_delta(2), 1, "tiny view degrades to 1 line");
        assert_eq!(page_delta(1), 1);
    }

    #[test]
    fn ctrl_f_scrolls_full_page() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 22);
    }

    #[test]
    fn ctrl_b_scrolls_back_and_clamps_at_top() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 0);
        // Already at top: stays 0.
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn ctrl_f_clamps_at_bottom() {
        let mut app = test_app(100, 24);
        app.reader.as_mut().unwrap().scroll = 70;
        handle_key(&mut app, ctrl('f'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 76); // 100 - 24
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test app"`
Expected: 编译错误 `cannot find function page_delta in this scope`

- [ ] **Step 3: 实现**

`src/app.rs`：在 `scroll_reader` 之后加：

```rust
/// Full-page scroll distance: one screen minus a 2-line overlap
/// (vim-style context); tiny views degrade to a single line.
fn page_delta(view_height: usize) -> usize {
    view_height.saturating_sub(2).max(1)
}
```

`reader_key` 中，现有 `let page = ...;` 之后加：

```rust
let full = app
    .reader
    .as_ref()
    .map(|r| page_delta(r.view_height))
    .unwrap_or(10) as isize;
```

match 中，`KeyCode::Char('c') if ...` 那行之前（或任何位置，保持风格）加：

```rust
KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    scroll_reader(app, full)
}
KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    scroll_reader(app, -full)
}
```

`src/ui/mod.rs` `draw_help` 的 keys 数组，在 `("d/u, PgDn/PgUp", "half page down/up"),` 之后加一行：

```rust
("Ctrl+f/b", "page forward / back"),
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 31 passed`（27 + 4 新增）；构建无新警告

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/ui/mod.rs
git commit -m "feat: ctrl+f/b full-page scrolling in reader (vim style)"
```

---

## Self-Review 记录

- Spec 覆盖：按键绑定（Step 3）、page_delta（Step 3）、帮助行（Step 3）、全部测试（Step 1）均有对应步骤；范围外项无任务，正确。
- 无占位符；所有代码完整给出。
- 类型一致性：`page_delta(usize) -> usize` 在 Step 1 测试与 Step 3 实现中签名一致；`full` 为 `isize` 与 `scroll_reader(app, delta: isize)` 匹配；`KeyEvent::new` 是 crossterm 0.28 的正确构造器（等价于 `KeyEvent { code, modifiers, kind: Press, state: NONE }`）。
