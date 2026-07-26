# mdview VS Code 风格装饰与主题增强 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 mdview 渲染管线上增量实现 VS Code 风格装饰（标题下划线、代码块行号与语言标签、引用块背景、表格双线分隔、内容居中）和 exe 旁 `md-styles/` 主题目录。

**Architecture:** 保持 `IR → render::layout → SLine/SSpan → ANSI/ratatui` 四层管线。`render/layout.rs` 拆分为 `render/layout/{mod,text,block,decorate}.rs`；装饰为内置默认行为，颜色取自现有 CSS 属性；主题查找按 cwd → exe 旁 → 内置顺序。

**Tech Stack:** Rust 2021, ratatui 0.29, crossterm 0.28, pulldown-cmark 0.12, syntect 5, unicode-width 0.2。

**Spec:** `docs/superpowers/specs/2026-07-26-mdview-renderer-design.md`

**构建/测试命令（Windows，必须走 .cargo-vc.bat 以获得 MSVC 链接器）：**

```bash
cmd //c ".cargo-vc.bat test"          # 全部测试
cmd //c ".cargo-vc.bat test <name>"   # 单个测试
cmd //c ".cargo-vc.bat build"         # 构建
```

**当前 baseline 状态：** 构建通过，16 个测试全绿。源码尚未提交进 git（首个 commit 仅含 spec 文档）。

---

### Task 1: 提交现有 baseline 代码

**Files:**
- 全部现有源码：`src/**`、`assets/**`、`Cargo.toml`、`Cargo.lock`、`.cargo-vc.bat`、`.gitignore`

已有实现尚未纳入 git，后续每个 task 的增量提交需要先有干净的 baseline。

- [ ] **Step 1: 提交 baseline**

```bash
git add -A
git commit -m "chore: import mdview baseline (build fixed, 16 tests green)"
```

- [ ] **Step 2: 确认工作区干净**

Run: `git status --short`
Expected: 无输出

---

### Task 2: 拆分 `render/layout.rs` 为模块目录

纯代码移动，不改变任何行为；现有 16 个测试保持全绿。

**Files:**
- Delete: `src/render/layout.rs`
- Create: `src/render/layout/mod.rs`
- Create: `src/render/layout/text.rs`
- Create: `src/render/layout/block.rs`

模块划分（`Renderer` 字段和 `Seg`/`MAX_CELL_WIDTH` 对子模块天然可见；**所有从 mod.rs 的 `block()` 调用的方法必须标 `pub(super)`**）：

- `mod.rs`：模块文档、`pub mod decorate; mod text; mod block;`（decorate 在 Task 4 创建，本任务先不声明）、`MAX_CELL_WIDTH`、`Seg`、`Renderer` 结构体、`render_document`、`render()`、行原语（`flush_line`/`blank`/`emit`/`emit_full`/`push_raw_line`）、`sub_render`、`Token`/`tokenize`/`text_width`/`truncate`、`#[cfg(test)] mod tests`（现有 4 个测试原样移入）
- `text.rs`：`paragraph`、`heading`、`list`、`register_link`、`flatten`、`flatten_into`、`flatten_styled`、`emit_wrapped`（全部 `pub(super)`）
- `block.rs`：`code_block`、`blockquote`、`table`、`rule`、`math_block`、`footnote_def`（全部 `pub(super)`）

- [ ] **Step 1: 把 Paragraph/Heading/Rule 的分支体抽取为方法**

在拆分前先就地重构（降低移动时的出错面）。`src/render/layout.rs` 中 `block()` 改为纯 dispatch，抽取三个新方法（其余分支已有对应方法）：

```rust
fn block(&mut self, block: &Block, chain: &[&'static str]) {
    match block {
        Block::Paragraph(content) => self.paragraph(content, chain),
        Block::Heading { level, content } => self.heading(*level, content, chain),
        Block::CodeBlock { lang, code } => self.code_block(lang, code),
        Block::BlockQuote(inner) => self.blockquote(inner, chain),
        Block::List { ordered, start, items } => {
            self.list(*ordered, *start, items, chain);
            self.blank();
        }
        Block::Table { head, aligns, rows } => self.table(head, aligns, rows, chain),
        Block::Rule => self.rule(),
        Block::MathBlock(src) => self.math_block(src),
        Block::FootnoteDef { label, blocks } => self.footnote_def(label, blocks, chain),
    }
}

fn paragraph(&mut self, content: &[Inline], chain: &[&'static str]) {
    let mut chain = chain.to_vec();
    chain.push("p");
    let segs = self.flatten(content, &chain);
    self.emit_wrapped(segs);
    self.blank();
}

fn heading(&mut self, level: u8, content: &[Inline], chain: &[&'static str]) {
    let tag: &'static str = match level {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        _ => "h6",
    };
    self.blank();
    let mut chain = chain.to_vec();
    chain.push(tag);
    let segs = self.flatten(content, &chain);
    self.emit_wrapped(segs);
    self.blank();
}

fn rule(&mut self) {
    let style = self.scheme.element("hr");
    let c = Computed {
        fg: style.border.or(style.fg),
        ..Computed::default()
    };
    self.blank();
    self.emit_full(SSpan::new("─".repeat(self.width), c));
    self.blank();
}
```

`Block::BlockQuote` 与 `Block::FootnoteDef` 的原分支体同样原样抽取为 `fn blockquote(&mut self, inner: &[Block], chain: &[&'static str])` 和 `fn footnote_def(&mut self, label: &str, blocks: &[Block], chain: &[&'static str])`（代码逐字移动，不改逻辑）。

- [ ] **Step 2: 跑测试确认抽取无行为变化**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 16 passed`

- [ ] **Step 3: 执行物理拆分**

`git mv src/render/layout.rs src/render/layout/mod.rs`，然后：

1. `mod.rs` 顶部加 `mod text; mod block;`，`use` 列表保留两个子模块都需要的公共项；只被子模块用的 `use` 移到对应文件
2. 按上面划分把方法剪到 `text.rs` / `block.rs`，各包一层 `impl<'a> Renderer<'a> { ... }`，方法标 `pub(super)`
3. `text.rs` 需要的 imports：`use super::{Renderer, Seg, Token, text_width, tokenize}; use crate::markdown::{Block, Inline, ListItem}; use crate::render::{SLine, SSpan}; use crate::style::Computed; use unicode_width::UnicodeWidthChar;`（按实际用到的项裁剪，编译器会提示）
4. `block.rs` 需要的 imports：`use super::{Renderer, MAX_CELL_WIDTH, text_width, truncate}; use crate::markdown::{Align, Block, Inline}; use crate::math; use crate::render::{SLine, SSpan}; use crate::style::Computed;`

- [ ] **Step 4: 跑测试确认拆分无行为变化**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 16 passed`（编译错误多半是 import 遗漏，按编译器提示补）

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: split render/layout.rs into layout/{mod,text,block}.rs"
```

---

### Task 3: 主题查找支持 exe 旁 `md-styles/` 目录

**Files:**
- Modify: `src/style/scheme.rs`

- [ ] **Step 1: 写失败测试**

在 `src/style/scheme.rs` 的 `mod tests` 中追加：

```rust
#[test]
fn user_dirs_priority_and_fallback() {
    let base = std::env::temp_dir().join(format!("mdview-scheme-{}", std::process::id()));
    let cwd_dir = base.join("cwd");
    let exe_dir = base.join("exe");
    std::fs::create_dir_all(&cwd_dir).unwrap();
    std::fs::create_dir_all(&exe_dir).unwrap();
    std::fs::write(exe_dir.join("nord.css"), "h1 { color: #010203 }").unwrap();
    std::fs::write(cwd_dir.join("nord.css"), "h1 { color: #040506 }").unwrap();
    std::fs::write(exe_dir.join("exe-only.css"), "h1 { color: #070809 }").unwrap();
    let dirs = [cwd_dir.clone(), exe_dir.clone()];

    // cwd 目录优先于 exe 目录。
    let s = load_from_dirs(&dirs, "nord").unwrap();
    assert_eq!(s.element("h1").fg, Some(Rgb(0x04, 0x05, 0x06)));
    // 只在 exe 目录存在的主题可以 fallback 加载。
    let s = load_from_dirs(&dirs, "exe-only").unwrap();
    assert_eq!(s.element("h1").fg, Some(Rgb(0x07, 0x08, 0x09)));
    // 两个目录都没有时返回 None。
    assert!(load_from_dirs(&dirs, "missing").is_none());
    // available_in 合并两个目录的 stem 并含内置主题。
    let names = available_in(&dirs);
    assert!(names.iter().any(|n| n == "nord"));
    assert!(names.iter().any(|n| n == "exe-only"));
    assert!(names.iter().any(|n| n == "tokyo-night"));

    std::fs::remove_dir_all(&base).ok();
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test user_dirs_priority_and_fallback"`
Expected: 编译错误 `cannot find function load_from_dirs in this scope`

- [ ] **Step 3: 实现**

在 `src/style/scheme.rs` 顶部把 `use super::color::Rgb;` 之后加 `use std::path::PathBuf;`，并新增三个函数（模块级，非 impl 内）：

```rust
/// User CSS theme directories in priority order: `./md-styles` first,
/// then `md-styles` next to the executable.
fn style_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("md-styles")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("md-styles"));
        }
    }
    dirs
}

/// Load `<name>.css` from the first directory that contains it.
fn load_from_dirs(dirs: &[PathBuf], name: &str) -> Option<Scheme> {
    for dir in dirs {
        if let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.css"))) {
            return Some(Scheme {
                name: name.to_string(),
                rules: css::parse(&text),
            });
        }
    }
    None
}

/// Builtin names plus CSS file stems from the given directories, deduped.
fn available_in(dirs: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = BUILTINS.iter().map(|(n, _)| n.to_string()).collect();
    for dir in dirs {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "css") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !names.iter().any(|n| n == stem) {
                            names.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    names
}
```

`Scheme::load` 与 `Scheme::available` 改为委托（替换原有函数体）：

```rust
pub fn load(name: &str) -> Scheme {
    if let Some(s) = load_from_dirs(&style_dirs(), name) {
        return s;
    }
    if let Some((n, text)) = BUILTINS.iter().find(|(n, _)| *n == name) {
        return Scheme {
            name: n.to_string(),
            rules: css::parse(text),
        };
    }
    if name != DEFAULT_THEME {
        return Scheme::load(DEFAULT_THEME);
    }
    Scheme {
        name: name.to_string(),
        rules: Vec::new(),
    }
}

pub fn available() -> Vec<String> {
    let mut names = available_in(&style_dirs());
    names.sort();
    names
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 17 passed`

- [ ] **Step 5: Commit**

```bash
git add src/style/scheme.rs
git commit -m "feat: look up user themes in exe-adjacent md-styles/ after cwd"
```

---

### Task 4: 标题下划线装饰（h1 `═` / h2 `─`）

**Files:**
- Create: `src/render/layout/decorate.rs`
- Modify: `src/render/layout/mod.rs`（声明 decorate 模块、加测试）
- Modify: `src/render/layout/text.rs`（`heading()` 加规则线）

- [ ] **Step 1: 写失败测试**

在 `src/render/layout/mod.rs` 的 `mod tests` 中追加：

```rust
#[test]
fn heading_rules() {
    let lines = render("# Top\n\n## Mid\n\n### Low", 30);
    assert_eq!(
        lines.iter().filter(|l| *l == &"═".repeat(30)).count(),
        1,
        "h1 gets one double rule, lines: {lines:?}"
    );
    assert_eq!(
        lines.iter().filter(|l| *l == &"─".repeat(30)).count(),
        1,
        "h2 gets one single rule, lines: {lines:?}"
    );
}
```

（h3 不产生任何规则线——若 h3 也有 `─`，第二个断言会数出 2 而失败。）

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test heading_rules"`
Expected: FAIL（两个 count 都是 0）

- [ ] **Step 3: 创建 decorate.rs 并实现**

创建 `src/render/layout/decorate.rs`：

```rust
//! Decoration primitives shared by text and block rendering.

use super::text_width;
use crate::render::{SLine, SSpan};
use crate::style::{Computed, Rgb};

/// Display width of a styled line.
pub fn line_width(line: &SLine) -> usize {
    line.iter().map(|s| text_width(&s.text)).sum()
}

/// Pad a line out to `width` columns with a background-colored span.
pub fn bg_fill(line: &mut SLine, width: usize, bg: Option<Rgb>) {
    if let Some(bg) = bg {
        let w = line_width(line);
        if w < width {
            line.push(SSpan::new(
                " ".repeat(width - w),
                Computed {
                    bg: Some(bg),
                    ..Computed::default()
                },
            ));
        }
    }
}

/// A horizontal rule line (`ch` repeated to `width`) in the given style.
pub fn rule_line(ch: char, width: usize, style: Computed) -> SLine {
    vec![SSpan::new(ch.to_string().repeat(width), style)]
}
```

`mod.rs` 顶部加 `pub mod decorate;`。

`text.rs` 的 `heading()` 在 `self.emit_wrapped(segs);` 之后、`self.blank();` 之前插入：

```rust
if level <= 2 {
    let style = self.scheme.style_for(&chain);
    let ch = if level == 1 { '═' } else { '─' };
    let rule = Computed {
        fg: style.border.or(style.fg),
        ..Computed::default()
    };
    self.push_raw_line(super::decorate::rule_line(ch, self.width, rule));
}
```

（`text.rs` 顶部需有 `use crate::style::Computed;`——已有。）

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 17 passed`

- [ ] **Step 5: Commit**

```bash
git add src/render/layout/
git commit -m "feat: h1/h2 underline rules (vscode style)"
```

---

### Task 5: 代码块 chrome（行号 + 语言标签 + 整行背景）

**Files:**
- Modify: `src/render/layout/block.rs`（重写 `code_block`）
- Modify: `src/render/layout/mod.rs`（加测试）

- [ ] **Step 1: 写失败测试**

在 `mod.rs` 的 `mod tests` 中追加：

```rust
#[test]
fn code_block_gutter_and_lang_tag() {
    let lines = render("```rust\nfn main() {}\nlet x = 1;\n```", 40);
    let body: Vec<&String> = lines.iter().filter(|l| l.contains('│')).collect();
    assert_eq!(body.len(), 2, "one gutter row per code line: {lines:?}");
    assert!(body[0].starts_with("1 │ "), "line numbers: {body:?}");
    assert!(body[1].starts_with("2 │ "));
    assert!(body[0].contains("rust"), "lang tag on first row: {:?}", body[0]);
}

#[test]
fn code_block_bg_fills_line() {
    let doc = parse_document("```\nhi\n```");
    let scheme = Scheme::load(crate::style::DEFAULT_THEME);
    let r = render_document(&doc, &scheme, 40, 0);
    let pre_bg = scheme.style_for(&["body", "pre"]).bg;
    assert!(pre_bg.is_some());
    let line = r
        .lines
        .iter()
        .find(|l| plain_of(l).contains("hi"))
        .expect("code line");
    assert!(line.iter().all(|s| s.style.bg == pre_bg), "whole row painted");
}
```

（`plain_of` 通过 `use super::*;` 已可在 tests 中用到——它由 `crate::render` re-export 进 layout/mod.rs 的 `use super::{plain_of, ...}`，tests 在 mod.rs 内，可直接用。）

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test code_block"`
Expected: FAIL（现有代码块无行号）

- [ ] **Step 3: 实现——整体替换 `block.rs` 中的 `code_block`**

```rust
fn code_block(&mut self, lang: &Option<String>, code: &str) {
    let pre = self.scheme.style_for(&["body", "pre"]);
    let dim = self.scheme.element("footnote");
    let highlighted = self.highlighter.highlight(code, lang.as_deref());

    // Gutter: right-aligned line numbers + separator, painted with pre bg.
    let num_w = highlighted.len().to_string().len();
    let gutter_w = num_w + 3; // "N │ "
    let code_w = self.width.saturating_sub(gutter_w).max(10);
    let gutter_style = Computed {
        fg: dim.fg,
        bg: pre.bg,
        ..Computed::default()
    };

    self.blank();
    for (i, runs) in highlighted.iter().enumerate() {
        self.flush_line();
        let mut line: SLine = vec![SSpan::new(
            format!("{:>num_w$} │ ", i + 1),
            gutter_style,
        )];
        let mut col = 0;
        for (color, text) in runs {
            col += text_width(text);
            line.push(SSpan::new(
                text.clone(),
                Computed {
                    fg: Some(*color),
                    bg: pre.bg,
                    ..Computed::default()
                },
            ));
        }
        // Language tag right-aligned on the first row, inside the padding.
        let tag = if i == 0 {
            lang.as_ref().map(|l| format!(" {l} "))
        } else {
            None
        };
        let tag_w = tag.as_ref().map(|t| text_width(t)).unwrap_or(0);
        let pad_w = code_w.saturating_sub(col);
        if let Some(tag) = tag.filter(|_| tag_w + 1 <= pad_w) {
            line.push(SSpan::new(
                " ".repeat(pad_w - tag_w),
                Computed {
                    bg: pre.bg,
                    ..Computed::default()
                },
            ));
            line.push(SSpan::new(tag, gutter_style));
        } else {
            super::decorate::bg_fill(&mut line, code_w, pre.bg);
        }
        self.out.push(line);
    }
    self.blank();
}
```

注意：行内容超过 `code_w` 时不换行不截断（保持现状行为），此时 `pad_w` 为 0，标签自动省略。

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 19 passed`

- [ ] **Step 5: Commit**

```bash
git add src/render/layout/
git commit -m "feat: code block gutter, line numbers, lang tag, full-row background"
```

---

### Task 6: 引用块 `▎` 竖线 + 背景连续成块

**Files:**
- Modify: `src/render/layout/block.rs`（重写 `blockquote`）
- Modify: `src/render/layout/mod.rs`（加测试）

- [ ] **Step 1: 写失败测试**

在 `mod.rs` 的 `mod tests` 中追加：

```rust
#[test]
fn blockquote_bar() {
    let lines = render("> hello\n> world", 40);
    assert!(
        lines.iter().filter(|l| l.starts_with("▎ ")).count() >= 2,
        "quote lines start with the bar: {lines:?}"
    );
}

#[test]
fn blockquote_bg_fills_rows() {
    let doc = parse_document("> hi");
    let scheme = Scheme {
        name: "t".into(),
        rules: crate::style::css::parse(
            "blockquote { background: #112233; border-color: #445566 }",
        ),
    };
    let r = render_document(&doc, &scheme, 40, 0);
    let line = r
        .lines
        .iter()
        .find(|l| plain_of(l).contains("hi"))
        .expect("quote line");
    let bg = Some(crate::style::Rgb(0x11, 0x22, 0x33));
    assert!(
        line.iter().all(|s| s.style.bg == bg),
        "every span painted, incl. padding: {line:?}"
    );
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test blockquote"`
Expected: FAIL（现有竖线是 `▌` 且无背景）

- [ ] **Step 3: 实现——整体替换 `block.rs` 中的 `blockquote`**

```rust
fn blockquote(&mut self, inner: &[Block], chain: &[&'static str]) {
    let mut chain = chain.to_vec();
    chain.push("blockquote");
    let qstyle = self.scheme.style_for(&chain);
    let lines = self.sub_render(inner, self.width.saturating_sub(2), &chain);
    let border = Computed {
        fg: qstyle.border.or(qstyle.fg),
        bg: qstyle.bg,
        ..Computed::default()
    };
    self.blank();
    for line in lines {
        self.flush_line();
        let mut l: SLine = vec![SSpan::new("▎ ".to_string(), border)];
        for mut span in line {
            if span.style.bg.is_none() {
                span.style.bg = qstyle.bg;
            }
            l.push(span);
        }
        // Pad to full width so multi-line quotes form a continuous block.
        super::decorate::bg_fill(&mut l, self.width, qstyle.bg);
        self.out.push(l);
    }
    self.blank();
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 21 passed`

- [ ] **Step 5: Commit**

```bash
git add src/render/layout/
git commit -m "feat: blockquote bar + continuous background"
```

---

### Task 7: 表格表头双线分隔

**Files:**
- Modify: `src/render/layout/block.rs`（`table` 的 `hline` 闭包与表头分隔行）
- Modify: `src/render/layout/mod.rs`（加测试）

- [ ] **Step 1: 写失败测试**

在 `mod.rs` 的 `mod tests` 中追加：

```rust
#[test]
fn table_header_double_separator() {
    let lines = render("| a | b |\n|---|---|\n| 1 | 2 |", 40);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with('╞') && l.contains('╪') && l.ends_with('╡')),
        "double separator under header: {lines:?}"
    );
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test table_header_double_separator"`
Expected: FAIL

- [ ] **Step 3: 实现**

`block.rs` 的 `table()` 中，把 `hline` 闭包加一个 `fill` 参数：

```rust
let hline = |left: &str, mid: &str, right: &str, fill: &str, widths: &[usize]| -> SLine {
    let mut s = String::new();
    s.push_str(left);
    for (i, w) in widths.iter().enumerate() {
        s.push_str(&fill.repeat(w + 2));
        if i + 1 < widths.len() {
            s.push_str(mid);
        }
    }
    s.push_str(right);
    vec![SSpan::new(s, border)]
};
```

四处调用改为：

```rust
self.push_raw_line(hline("┌", "┬", "┐", "─", &widths));
self.push_raw_line(row_line(head, th_style));
self.push_raw_line(hline("╞", "╪", "╡", "═", &widths));
for row in rows {
    self.push_raw_line(row_line(row, td_style));
}
self.push_raw_line(hline("└", "┴", "┘", "─", &widths));
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 22 passed`

- [ ] **Step 5: Commit**

```bash
git add src/render/layout/
git commit -m "feat: double-line separator under table header"
```

---

### Task 8: 内容水平居中（offset 参数贯通 TUI 与管道模式）

**Files:**
- Modify: `src/render/layout/mod.rs`（`render_document` 加 `offset` 参数 + 测试）
- Modify: `src/main.rs`（管道模式计算 offset）
- Modify: `src/app.rs`（`render_file`/`open_reader`/`Reader`/`run` 加 offset）
- Modify: `src/ui/reader.rs`（按视图宽度计算 offset，变化时重渲染）
- Modify: `src/ui/browser.rs:48`（预览传 0）

- [ ] **Step 1: 写失败测试**

`mod.rs` tests 的辅助函数改为：

```rust
fn render(src: &str, width: usize) -> Vec<String> {
    render_off(src, width, 0)
}

fn render_off(src: &str, width: usize, offset: usize) -> Vec<String> {
    let doc = parse_document(src);
    let scheme = Scheme::load(crate::style::DEFAULT_THEME);
    render_document(&doc, &scheme, width, offset).plain
}
```

追加测试：

```rust
#[test]
fn centers_with_offset() {
    let lines = render_off("hello", 20, 5);
    let first = lines.iter().find(|l| !l.trim().is_empty()).unwrap();
    assert!(first.starts_with("     hello"), "offset pad: {first:?}");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test centers_with_offset"`
Expected: 编译错误（`render_document` 只收 3 个参数）

- [ ] **Step 3: 实现 `render_document` 的 offset**

`mod.rs` 中：

```rust
pub fn render_document(doc: &Document, scheme: &Scheme, width: usize, offset: usize) -> Rendered {
    let mut r = Renderer {
        scheme,
        highlighter: Highlighter::new(scheme),
        width: width.max(20),
        out: Vec::new(),
        cur: Vec::new(),
        col: 0,
        links: Vec::new(),
    };
    r.render(doc, offset)
}
```

`render()` 加 `offset` 参数，在 "Trim trailing blank lines" 之后、计算 `plain` 之前插入：

```rust
// Uniform left offset for horizontal centering.
if offset > 0 {
    let pad = || SSpan::new(" ".repeat(offset), Computed::default());
    for line in &mut self.out {
        if !line.is_empty() {
            line.insert(0, pad());
        }
    }
}
```

- [ ] **Step 4: 贯通调用点**

`src/main.rs` 管道模式（约 62-65 行）改为：

```rust
let doc = markdown::parse_document(&text);
let term = terminal_width();
let width = term.min(max_width);
let offset = (term - width) / 2;
let rendered = render::layout::render_document(&doc, &scheme, width, offset);
print!("{}", render::ansi::render_ansi(&rendered.lines, level));
```

`src/app.rs`：

```rust
// Reader 结构体加字段：
pub struct Reader {
    pub path: PathBuf,
    pub rendered: Rendered,
    pub width: u16,
    pub offset: u16,
    pub scroll: usize,
    pub view_height: usize,
}

// render_file 加 offset 参数：
pub fn render_file(&self, path: &Path, width: u16, offset: u16) -> Rendered {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| format!("(error: {e})"));
    let doc = parse_document(&text);
    render_document(&doc, &self.scheme, width as usize, offset as usize)
}

// open_reader 加 offset 参数：
pub fn open_reader(&mut self, path: PathBuf, width: u16, offset: u16) {
    let rendered = self.render_file(&path, width, offset);
    self.reader = Some(Reader {
        path,
        rendered,
        width,
        offset,
        scroll: 0,
        view_height: 24,
    });
    self.search_matches.clear();
    self.mode = Mode::Reader;
}

// reload_reader 改用存储的 offset：
let rendered = self.render_file(&path, width, offset);
// 其中开头改为：
let Some(reader) = &self.reader else { return };
let scroll = reader.scroll;
let path = reader.path.clone();
let width = reader.width;
let offset = reader.offset;

// run() 中（约 228-231 行）：
if let Some(path) = start_file {
    let term_w = terminal.size()?.width;
    let width = content_width(term_w, max_width);
    let offset = term_w.saturating_sub(2).saturating_sub(width) / 2;
    app.open_reader(path, width, offset);
}

// Enter 打开文件处（约 351 行，此刻不知道视图宽度，传 0，首帧 draw 会修正）：
app.open_reader(path, app.max_width as u16, 0);
```

`src/ui/reader.rs`（16-27 行区域）改为：

```rust
let want_width = content_width(view.width, app.max_width);
let want_offset = view.width.saturating_sub(2).saturating_sub(want_width) / 2;
let Some(reader) = app.reader.as_mut() else { return };
reader.view_height = view.height.saturating_sub(2) as usize;
if reader.width != want_width || reader.offset != want_offset {
    let path = reader.path.clone();
    let scroll = reader.scroll;
    let rendered = app.render_file(&path, want_width, want_offset);
    let reader = app.reader.as_mut().unwrap();
    reader.rendered = rendered;
    reader.width = want_width;
    reader.offset = want_offset;
    reader.scroll = scroll.min(reader.rendered.lines.len().saturating_sub(1));
}
```

`src/ui/browser.rs:48`：`app.render_file(&path, preview_inner, 0)`。

- [ ] **Step 5: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 23 passed`

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: center content horizontally (tui + pipe mode)"
```

---

### Task 9: 内置 8 个主题补 `border-color` 与引用背景

**Files:**
- Modify: `assets/styles/tokyo-night.css`、`dracula.css`、`gruvbox-dark.css`、`gruvbox-light.css`、`nord.css`、`solarized-dark.css`、`solarized-light.css`、`github-light.css`

变换规则（对每个文件机械执行，以 nord.css 为示例）：

1. `h1` 行追加 `border-color: <该行 color 值>`：`h1 { color: #81a1c1; font-weight: bold; border-color: #81a1c1; }`
2. `h2` 行同样处理：`h2 { color: #88c0d0; font-weight: bold; border-color: #88c0d0; }`
3. `blockquote` 行追加 `background: <同文件 pre 行的 background-color 值>`：`blockquote { color: #9da8bb; border-color: #434c5e; background: #3b4252; }`

- [ ] **Step 1: 逐文件应用上述三条变换**

- [ ] **Step 2: 跑测试确认内置主题仍全部可解析**

Run: `cmd //c ".cargo-vc.bat test builtin_schemes_parse"`
Expected: PASS

- [ ] **Step 3: 管道模式冒烟测试**

```bash
printf '# Title\n\n> quote\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n' | ./target/debug/mdview.exe | sed 's/\x1b\[[0-9;]*m//g; s/\x1b]8;;[^\x07\x1b]*\x1b\\\\//g'
```

Expected（人工核对）：`Title` 下一行是 `═` 规则线；引用行以 `▎` 开头；代码块有 `1 │ ` 行号和 `rust` 标签；表头下有 `╞═╪═╡` 分隔行。

- [ ] **Step 4: Commit**

```bash
git add assets/styles/
git commit -m "feat: builtin themes declare border-color and blockquote background"
```

---

### Task 10: 全量验证

- [ ] **Step 1: 完整测试**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部通过（23 个）

- [ ] **Step 2: 检查警告**

Run: `cmd //c ".cargo-vc.bat build" 2>&1 | grep -c warning`
Expected: 不引入新警告（baseline 原有 2 个 unused 警告，可顺手清理为 0）

- [ ] **Step 3: TUI 冒烟（人工）**

```bash
./target/debug/mdview.exe --list-themes   # 列出 8+ 个主题
./target/debug/mdview.exe README.md       # 若仓库无 README 用任意 .md
```

人工核对：阅读器内容居中、`t` 主题切换后装饰颜色随主题变化。

---

## Self-Review 记录

- Spec 覆盖：模块拆分(T2)、exe 主题目录(T3)、标题(T4)、代码块(T5)、引用块(T6)、表格(T7)、居中(T8)、内置 CSS(T9)、测试策略（各 task 内嵌）、构建说明（头部）。Spec 范围外项（图片/布局 CSS/开关/文件监听）无任务，正确。
- 无占位符；所有代码步骤含完整代码或精确的逐字移动指令。
- 类型一致性：`render_document(doc, scheme, width, offset)` 在 T8 统一定义并被 main/app 一致调用；`Reader.offset: u16` 与 `render_file(_, _, offset: u16)` 一致；`decorate::{line_width, bg_fill, rule_line}` 在 T4 定义、T5/T6 使用，签名一致。
