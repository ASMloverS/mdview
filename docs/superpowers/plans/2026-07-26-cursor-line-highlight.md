# mdview 阅读器光标行高亮 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阅读模式引入光标行：所有键盘移动操作移动光标、滚动跟随，光标行整行（满视口宽度）以主题 `cursor` 元素的背景色高亮。

**Architecture:** 光标是纯 UI 状态（`Reader.cursor`），不进渲染管线；移动键先更新光标再由 `follow_cursor` 调整 scroll；高亮在 `reader::draw` 里 `convert()` 之后对光标行叠加 bg 补丁并追加填充 span，实现整行宽度高亮（ratatui 0.29 的 `Paragraph` 只写 grapheme 覆盖的单元格，不会自动填充行尾）。

**Tech Stack:** Rust, ratatui 0.29, crossterm 0.28

**Spec:** `docs/superpowers/specs/2026-07-26-cursor-line-highlight-design.md`

**构建/测试命令（Windows/MSVC 必须用包装脚本）：**
- 全部测试：`cmd //c ".cargo-vc.bat test"`
- 按名过滤：`cmd //c ".cargo-vc.bat test <过滤词>"`

---

### Task 1: `Reader.cursor` 状态与移动语义（`src/app.rs`）

**Files:**
- Modify: `src/app.rs`（`Reader` 结构、`open_reader`、`reload_reader`、`jump_match`、`reader_key`，新增 `follow_cursor`/`move_cursor`，tests 模块）

**背景：** 现状 `j/k` 等直接改 `reader.scroll`（`scroll_reader`，`src/app.rs:263-269`），`jump_match` 也直接赋 `scroll`（`src/app.rs:153-155`）。本任务改为光标语义。

- [x] **Step 1: 写失败测试（替换 tests 模块中 3 个 ctrl+f/b 旧测试）**

把 `src/app.rs` tests 模块里的 `ctrl_f_scrolls_full_page`、`ctrl_b_scrolls_back_and_clamps_at_top`、`ctrl_f_clamps_at_bottom` 三个测试整体替换为以下测试，并在 `test_app` 的 `Reader { ... }` 字面量中加 `cursor: 0,`（位于 `scroll: 0,` 之后）：

```rust
    #[test]
    fn j_moves_cursor_without_scrolling() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 1);
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn k_clamps_cursor_at_top() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 0);
    }

    #[test]
    fn cursor_past_bottom_edge_scrolls_down() {
        let mut app = test_app(100, 24);
        app.reader.as_mut().unwrap().cursor = 23;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 24);
        assert_eq!(r.scroll, 1);
    }

    #[test]
    fn cursor_past_top_edge_scrolls_up() {
        let mut app = test_app(100, 24);
        let r = app.reader.as_mut().unwrap();
        r.cursor = 10;
        r.scroll = 10;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('k')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 9);
        assert_eq!(r.scroll, 9);
    }

    #[test]
    fn d_moves_cursor_half_page() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('d')));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 12);
    }

    #[test]
    fn ctrl_f_moves_cursor_one_page() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 22);
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn ctrl_b_moves_back_and_clamps() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 0);
        // 已在顶部：保持 0。
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 0);
    }

    #[test]
    fn ctrl_f_clamps_at_last_line_and_scroll_follows() {
        let mut app = test_app(100, 24);
        app.reader.as_mut().unwrap().cursor = 90;
        handle_key(&mut app, ctrl('f'));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 99);
        assert_eq!(r.scroll, 76); // 99 + 1 - 24
    }

    #[test]
    fn g_and_g_upper_jump_to_ends() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('G')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 99);
        assert_eq!(r.scroll, 76);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('g')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 0);
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn search_jump_moves_cursor_and_scroll_follows() {
        let mut app = test_app(100, 24);
        app.search_matches = vec![10, 50];
        app.jump_match(true);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 10);
        assert_eq!(r.scroll, 0);
        app.jump_match(true);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 50);
        assert_eq!(r.scroll, 27); // 50 + 1 - 24
    }
```

- [x] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test app::"`
Expected: 编译失败（`Reader` 无 `cursor` 字段）

- [x] **Step 3: 实现光标状态与移动**

3a. `Reader` 结构体（`src/app.rs:25-32`）在 `scroll` 后加字段：

```rust
pub struct Reader {
    pub path: PathBuf,
    pub rendered: Rendered,
    pub width: u16,
    pub offset: u16,
    pub scroll: usize,
    pub cursor: usize,
    pub view_height: usize,
}
```

3b. `open_reader`（`src/app.rs:83-90`）的 `Reader { ... }` 字面量加 `cursor: 0,`。

3c. `reload_reader`（`src/app.rs:104-107`）中 scroll clamp 之后加 cursor clamp：

```rust
        if let Some(reader) = self.reader.as_mut() {
            reader.rendered = rendered;
            let last = reader.rendered.lines.len().saturating_sub(1);
            reader.scroll = scroll.min(last);
            reader.cursor = reader.cursor.min(last);
        }
```

3d. `jump_match`（`src/app.rs:133-156`）：基准从 `scroll` 改为 `cursor`，命中后赋 `cursor` 并滚动跟随：

```rust
        let Some(reader) = self.reader.as_mut() else { return };
        let cur = reader.cursor;
```

结尾改为：

```rust
        if let Some(line) = next {
            reader.cursor = line;
            follow_cursor(reader);
        }
```

3e. 在 `scroll_reader`（`src/app.rs:263`）之前新增两个函数（`scroll_reader` 本身保留不动，鼠标滚轮继续用它）：

```rust
/// 滚动跟随：调整 scroll 让光标落在可视区内。
fn follow_cursor(reader: &mut Reader) {
    if reader.cursor < reader.scroll {
        reader.scroll = reader.cursor;
    } else if reader.cursor >= reader.scroll + reader.view_height {
        reader.scroll = reader.cursor + 1 - reader.view_height;
    }
}

/// 键盘移动：光标移动 delta 行（clamp 到文档范围），滚动跟随。
fn move_cursor(app: &mut App, delta: isize) {
    let Some(reader) = app.reader.as_mut() else { return };
    let last = reader.rendered.lines.len().saturating_sub(1);
    let next = reader.cursor as isize + delta;
    reader.cursor = next.clamp(0, last as isize) as usize;
    follow_cursor(reader);
}
```

3f. `reader_key`（`src/app.rs:385-407`）键盘移动全部从 `scroll_reader` 换成 `move_cursor`：

```rust
        KeyCode::Char('j') | KeyCode::Down => move_cursor(app, 1),
        KeyCode::Char('k') | KeyCode::Up => move_cursor(app, -1),
        KeyCode::Char('d') | KeyCode::PageDown => move_cursor(app, page),
        KeyCode::Char('u') | KeyCode::PageUp => move_cursor(app, -page),
        KeyCode::Char('g') => move_cursor(app, isize::MIN / 2),
        KeyCode::Char('G') => move_cursor(app, isize::MAX / 2),
```

以及：

```rust
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            move_cursor(app, full)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            move_cursor(app, -full)
        }
```

注意：`handle_mouse`（`src/app.rs:417-435`）继续调用 `scroll_reader`，不要改。

- [x] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test app::"`
Expected: 全部 PASS（含保留的 `page_delta_full_screen_minus_overlap`）

- [x] **Step 5: 跑全量测试确认无回归、零警告**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS，无 warning

- [x] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "✨ reader(feat): cursor line state with scroll-follow movement"
```

---

### Task 2: 内置主题新增 `cursor` 元素（`assets/styles/*.css` + 解析测试）

**Files:**
- Modify: `assets/styles/dracula.css`、`github-light.css`、`gruvbox-dark.css`、`gruvbox-light.css`、`nord.css`、`solarized-dark.css`、`solarized-light.css`、`tokyo-night.css`
- Test: `src/style/scheme.rs`（tests 模块）

- [x] **Step 1: 写失败测试**

在 `src/style/scheme.rs` 的 tests 模块末尾（`default_theme_is_gruvbox_dark` 之后）加：

```rust
    #[test]
    fn builtin_schemes_define_cursor_background() {
        for name in Scheme::builtin_names() {
            let s = Scheme::load(name);
            assert!(
                s.element("cursor").bg.is_some(),
                "builtin {name} lacks cursor background"
            );
        }
    }
```

- [x] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test scheme"`
Expected: FAIL（`builtin dracula lacks cursor background`）

- [x] **Step 3: 给 8 个内置主题各加一条 cursor 规则**

在每个 css 文件的 `footnote { ... }` 行之后追加对应一行（颜色取该主题 body 与 pre 底色之间的中间色，保证在普通行和代码块行上都可见）：

`assets/styles/dracula.css`:
```css
cursor { background-color: #383a4a; }
```

`assets/styles/github-light.css`:
```css
cursor { background-color: #eaeef2; }
```

`assets/styles/gruvbox-dark.css`:
```css
cursor { background-color: #32302f; }
```

`assets/styles/gruvbox-light.css`:
```css
cursor { background-color: #f2e5bc; }
```

`assets/styles/nord.css`:
```css
cursor { background-color: #434c5e; }
```

`assets/styles/solarized-dark.css`:
```css
cursor { background-color: #04313c; }
```

`assets/styles/solarized-light.css`:
```css
cursor { background-color: #f6efdc; }
```

`assets/styles/tokyo-night.css`:
```css
cursor { background-color: #292e42; }
```

- [x] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test scheme"`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add assets/styles/ src/style/scheme.rs
git commit -m "💄 style(feat): cursor element background in builtin themes"
```

---

### Task 3: 光标行高亮绘制（`src/ui/mod.rs` + `src/ui/reader.rs`）

**Files:**
- Modify: `src/ui/mod.rs`（新增 `cursor_style`）
- Modify: `src/ui/reader.rs`（draw 中应用高亮 + tests 模块）

**前提知识：** ratatui 0.29 的 `Line::patch_style` 消费 `self` 并返回新 `Line`，且只补丁 `line.style`、不动各 span；`Paragraph` 渲染时只写 span grapheme 覆盖的单元格，`Line.style` 经 `styled_graphemes` 叠加到各 span（只覆盖已设置字段，因此 bg-only 补丁保留 fg），但不会填充行尾空白。因此整行高亮 = 行 bg 补丁 + 逐 span bg 补丁（覆盖代码块/引用行 span 自带的 bg）+ 追加填充 span 补齐行尾（Step 4 的实现即如此，填充 span 是必需而非备选）。

- [x] **Step 1: `src/ui/mod.rs` 新增 `cursor_style`**

在 `dim_style`（`src/ui/mod.rs:63-70`）之后加：

```rust
/// 光标行背景：主题 cursor 元素的 bg；未定义时不高亮。
pub fn cursor_style(app: &App) -> Option<Style> {
    let bg = app.scheme.element("cursor").bg?;
    Some(Style::default().bg(app.level.to_ratatui(bg)))
}
```

- [x] **Step 2: 写失败测试（TestBackend 验证整行高亮）**

在 `src/ui/reader.rs` 文件末尾新增 tests 模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, Reader};
    use crate::render::Rendered;
    use crate::style::{ColorLevel, Scheme};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn test_app(lines: usize, cursor: usize) -> App {
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let mut app = App::new(scheme, ColorLevel::True, 100);
        app.mode = Mode::Reader;
        app.reader = Some(Reader {
            path: PathBuf::from("test.md"),
            rendered: Rendered {
                lines: vec![Vec::new(); lines],
                plain: vec![String::new(); lines],
            },
            // 与 30 列终端的 want_width/want_offset 一致，避免 draw 触发重排版。
            width: 28,
            offset: 0,
            scroll: 0,
            cursor,
            view_height: 8,
        });
        app
    }

    #[test]
    fn cursor_line_highlighted_full_width() {
        let mut app = test_app(20, 2);
        let want = app.scheme.element("cursor").bg.unwrap();
        let bg = Color::Rgb(want.0, want.1, want.2);
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        // 边框内侧：cursor=2, scroll=0 → 屏幕行 y = 1 + 2 = 3。
        for x in 1..29 {
            assert_eq!(buf.cell((x, 3)).unwrap().bg, bg, "x={x}");
        }
        // 相邻行无高亮。
        assert_ne!(buf.cell((1, 2)).unwrap().bg, bg);
        assert_ne!(buf.cell((1, 4)).unwrap().bg, bg);
    }

    #[test]
    fn no_cursor_element_means_no_highlight() {
        let mut app = test_app(20, 2);
        // 空规则主题：无 cursor 元素。
        app.scheme = Scheme {
            name: "empty".into(),
            rules: Vec::new(),
        };
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        assert_eq!(buf.cell((5, 3)).unwrap().bg, Color::Reset);
    }
}
```

- [x] **Step 3: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test ui::reader"`
Expected: FAIL（`cursor_line_highlighted_full_width` 断言不相等，当前无高亮逻辑）

- [x] **Step 4: 在 `reader::draw` 中应用高亮**

`src/ui/reader.rs:32` 处 `let lines = convert(app, &reader.rendered);` 改为：

```rust
    let mut lines = convert(app, &reader.rendered);
    // 光标行高亮：行 style 与各 span 都叠加光标背景（bg-only 补丁保留 fg），
    // 行尾用填充 span 补齐。
    if let Some(style) = cursor_style(app) {
        if let Some(line) = lines.get_mut(reader.cursor) {
            *line = std::mem::take(line).patch_style(style);
            for span in &mut line.spans {
                span.style = span.style.patch(style);
            }
            line.spans.push(Span::styled(" ".repeat(view.width as usize), style));
        }
    }
```

注意：`Line::patch_style` 消费 `self`，需 `std::mem::take` 回写；`Paragraph` 不填充行尾空白，填充 span 是必需的（不是备选）；代码块/引用行的 span 自带 bg，必须逐 span patch 才能被光标背景覆盖。`view` 变量在 draw 中已存在；`Span` 经 `ratatui::prelude::*` 已在作用域。

并把文件头的 import 从 `use super::{accent_style, chrome_style, convert, dim_style};` 改为 `use super::{accent_style, chrome_style, convert, cursor_style, dim_style};`

- [x] **Step 5: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test ui::reader"`
Expected: 两个测试 PASS

- [x] **Step 6: 跑全量测试确认无回归、零警告**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS，无 warning

- [x] **Step 7: Commit**

```bash
git add src/ui/mod.rs src/ui/reader.rs
git commit -m "✨ reader(feat): highlight cursor line full width"
```

---

### Task 4: 手动冒烟验证

- [x] **Step 1: 构建 release 并手动验证**

Run: `cmd //c ".cargo-vc.bat build"`
然后运行 `target\debug\mdview.exe README.md`，逐项验证：
- 打开即高亮首行；
- `j/k` 逐行移动高亮，到视口底/顶边缘时滚动跟随；
- `d/u`、`Ctrl+f/b`、`g/G` 移动光标；
- `/` 搜索后 `n/N` 跳转，目标行被高亮；
- 鼠标滚轮滚动时高亮行不动；
- `t` 切换几个主题，高亮色随主题变化；
- `Esc`、`q` 正常退出。

- [x] **Step 2: 若一切正常，完结；若发现问题，回到对应 Task 修复**
