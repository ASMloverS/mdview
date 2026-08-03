# syntax-styles 语法高亮主题实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 语法高亮配色独立成 `syntax-styles/*.css`（16 类别、per-language 特化、粗体/斜体/下划线生效），内置 20 套与页面主题同名的语法主题。

**Architecture:** 新增 `src/style/syntax.rs`（SyntaxTheme 加载 + 逐类别回退解析），重写 `src/highlight.rs`（每语言懒构建 syntect Theme 并缓存，输出携带字体样式的 HSpan），`render_document` 增加 `syntax_theme` 参数，config/CLI/TUI 接线。Spec：`docs/superpowers/specs/2026-08-04-syntax-styles-design.md`。

**Tech Stack:** Rust, syntect 5, 现有 css/scheme 迷你 CSS 子系统。

**与 spec 的偏差：** 不实现 `Config::save_syntax_theme`（TUI 无语法主题写入口，YAGNI；`syntax_theme` 字段只读）。

**前置：** 从 master 切短生命周期分支 `git checkout -b syntax-styles`。

**构建/测试命令（Windows/MSVC，Git Bash）：** `cmd //c ".cargo-vc.bat test"`；单测过滤 `cmd //c ".cargo-vc.bat test <过滤串>"`。

---

### Task 1: scheme.rs 辅助改造 + src/style/syntax.rs（加载与回退解析）

**Files:**
- Modify: `src/style/scheme.rs`（`style_dirs` 拆出 `exe_dir`，`selector_matches` 改 `pub(crate)`，新增 `has_syntax_rule`）
- Create: `src/style/syntax.rs`
- Modify: `src/style/mod.rs`（导出）

- [ ] **Step 1: 写失败测试（syntax.rs 先只放测试模块骨架）**

创建 `src/style/syntax.rs`，先只写测试期望的 API 形状（实现留空会导致编译错，先把测试写出来）：

```rust
//! Syntax highlighting themes: exe 同级 `syntax-styles/<name>.css` 用户主题，
//! 内置主题 embed 自 `assets/syntax-styles/`。与页面主题（md-styles）解耦。

use super::color::Rgb;
use super::css::{self, Props, Rule};
use super::scheme::{exe_dir, selector_matches, Scheme};
use std::path::PathBuf;

/// 16 个 token 类别（顺序即 syntect ThemeItem 顺序：宽泛在前）。
pub const CLASSES: &[&str] = &[
    "keyword", "string", "comment", "function", "type", "number", "operator",
    "variable", "constant", "macro", "attribute", "decorator", "module",
    "namespace", "punctuation", "label",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn page(css_text: &str) -> Scheme {
        Scheme { name: "p".into(), rules: css::parse(css_text) }
    }

    #[test]
    fn resolve_global_rule() {
        let t = SyntaxTheme::from_css(
            "t",
            "keyword { color: #fb4934; font-weight: bold }",
        );
        let p = page("");
        let st = t.resolve(&["rust".to_string()], "keyword", &p);
        assert_eq!(st.fg, Some(Rgb(0xfb, 0x49, 0x34)));
        assert!(st.bold);
        assert!(!st.italic);
    }

    #[test]
    fn lang_specific_overrides_and_merges_global() {
        let t = SyntaxTheme::from_css(
            "t",
            "keyword { color: #111111 } rust keyword { font-weight: bold }",
        );
        let p = page("");
        let rust = t.resolve(&["rust".to_string()], "keyword", &p);
        assert_eq!(rust.fg, Some(Rgb(0x11, 0x11, 0x11)), "全局色并入");
        assert!(rust.bold, "语言特化 font-weight 生效");
        let python = t.resolve(&["python".to_string()], "keyword", &p);
        assert_eq!(python.fg, Some(Rgb(0x11, 0x11, 0x11)));
        assert!(!python.bold);
    }

    #[test]
    fn second_lang_candidate_matches() {
        // fence 写 rs，规则写 rust：第二个候选命中。
        let t = SyntaxTheme::from_css("t", "rust macro { color: #222222 }");
        let p = page("");
        let st = t.resolve(&["rs".to_string(), "rust".to_string()], "macro", &p);
        assert_eq!(st.fg, Some(Rgb(0x22, 0x22, 0x22)));
    }

    #[test]
    fn global_rule_does_not_shadow_other_lang_specific() {
        // fence=rs 无 rs 特化但有全局规则时，仍应继续尝试候选 rust 的特化。
        let t = SyntaxTheme::from_css(
            "t",
            "macro { color: #111111 } rust macro { color: #222222 }",
        );
        let p = page("");
        let st = t.resolve(&["rs".to_string(), "rust".to_string()], "macro", &p);
        assert_eq!(st.fg, Some(Rgb(0x22, 0x22, 0x22)));
    }

    #[test]
    fn page_theme_fallback_with_italic() {
        let t = SyntaxTheme::from_css("t", "");
        let p = page("syntax-comment { color: #928374; font-style: italic }");
        let st = t.resolve(&["rust".to_string()], "comment", &p);
        assert_eq!(st.fg, Some(Rgb(0x92, 0x83, 0x74)));
        assert!(st.italic, "页面主题 syntax-comment 的 italic 经回退生效");
    }

    #[test]
    fn alias_derivation() {
        // constant → number（语法主题内）。
        let t = SyntaxTheme::from_css("t", "number { color: #010203 }");
        let p = page("");
        assert_eq!(
            t.resolve(&["rust".to_string()], "constant", &p).fg,
            Some(Rgb(0x01, 0x02, 0x03))
        );
        // 别名目标可继续回退到页面主题。
        let p2 = page("syntax-number { color: #040506 }");
        let t2 = SyntaxTheme::from_css("t", "");
        assert_eq!(
            t2.resolve(&["rust".to_string()], "constant", &p2).fg,
            Some(Rgb(0x04, 0x05, 0x06))
        );
        // variable 无别名：全部缺失时 fg=None（默认前景）。
        assert_eq!(t2.resolve(&["rust".to_string()], "variable", &p2).fg, None);
    }

    #[test]
    fn unknown_theme_loads_empty() {
        let t = SyntaxTheme::load("no-such-syntax-theme-xyz");
        assert!(t.rules.is_empty());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test syntax"`
Expected: 编译失败（`SyntaxTheme`、`exe_dir`、`has_syntax_rule` 等不存在）。

- [ ] **Step 3: 实现 scheme.rs 辅助改动**

`src/style/scheme.rs`：

1. 把 `style_dirs()` 改为复用新的 `exe_dir()`，并新增 `has_syntax_rule`：

```rust
/// exe 所在目录（定位失败时为空路径）。
pub(crate) fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default()
}

/// User CSS theme directory: `md-styles` next to the executable
/// (cwd-relative fallback when the exe location is unavailable).
fn style_dirs() -> Vec<PathBuf> {
    vec![exe_dir().join("md-styles")]
}
```

2. `selector_matches` 签名改为 `pub(crate) fn selector_matches(...)`（其余不变）。

3. `impl Scheme` 中新增：

```rust
    /// 页面主题是否定义了 `syntax-<class>` 规则（区别于 style_for 的默认前景）。
    pub fn has_syntax_rule(&self, class: &str) -> bool {
        let leaf = format!("syntax-{class}");
        self.rules
            .iter()
            .any(|r| r.selectors.iter().any(|sel| sel.last().is_some_and(|t| *t == leaf)))
    }
```

- [ ] **Step 4: 实现 syntax.rs 主体**

在 `src/style/syntax.rs` 的 `CLASSES` 之后、测试模块之前插入：

```rust
/// 别名派生：类别缺省时回落到的目标类别（variable 无别名，走默认前景）。
fn alias_of(class: &str) -> Option<&'static str> {
    match class {
        "constant" => Some("number"),
        "macro" | "decorator" => Some("function"),
        "attribute" | "module" | "namespace" => Some("type"),
        "punctuation" => Some("operator"),
        "label" => Some("keyword"),
        _ => None,
    }
}

/// 一个 token 类别的解析结果；`fg = None` 表示用代码块默认前景色。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyntaxStyle {
    pub fg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// 一套语法主题：内置 + `syntax-styles/<name>.css`。
#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    pub name: String,
    pub rules: Vec<Rule>,
}

/// 内置语法主题（assets/syntax-styles/，与页面主题同名配对）。
const BUILTINS: &[(&str, &str)] = &[
    ("tokyo-night", include_str!("../../assets/syntax-styles/tokyo-night.css")),
    ("dracula", include_str!("../../assets/syntax-styles/dracula.css")),
    ("gruvbox-dark", include_str!("../../assets/syntax-styles/gruvbox-dark.css")),
    ("nord", include_str!("../../assets/syntax-styles/nord.css")),
    ("solarized-dark", include_str!("../../assets/syntax-styles/solarized-dark.css")),
    ("catppuccin-mocha", include_str!("../../assets/syntax-styles/catppuccin-mocha.css")),
    ("kanagawa", include_str!("../../assets/syntax-styles/kanagawa.css")),
    ("rose-pine", include_str!("../../assets/syntax-styles/rose-pine.css")),
    ("everforest", include_str!("../../assets/syntax-styles/everforest.css")),
    ("one-dark", include_str!("../../assets/syntax-styles/one-dark.css")),
    ("monokai", include_str!("../../assets/syntax-styles/monokai.css")),
    ("ayu-dark", include_str!("../../assets/syntax-styles/ayu-dark.css")),
    ("github-dark", include_str!("../../assets/syntax-styles/github-dark.css")),
    ("github-light", include_str!("../../assets/syntax-styles/github-light.css")),
    ("solarized-light", include_str!("../../assets/syntax-styles/solarized-light.css")),
    ("gruvbox-light", include_str!("../../assets/syntax-styles/gruvbox-light.css")),
    ("catppuccin-latte", include_str!("../../assets/syntax-styles/catppuccin-latte.css")),
    ("rose-pine-dawn", include_str!("../../assets/syntax-styles/rose-pine-dawn.css")),
    ("everforest-light", include_str!("../../assets/syntax-styles/everforest-light.css")),
    ("ayu-light", include_str!("../../assets/syntax-styles/ayu-light.css")),
];

/// 用户语法主题目录：exe 同级 `syntax-styles/`。
fn dirs() -> Vec<PathBuf> {
    vec![exe_dir().join("syntax-styles")]
}

impl SyntaxTheme {
    #[cfg(test)]
    pub fn builtin_names() -> Vec<&'static str> {
        BUILTINS.iter().map(|(n, _)| *n).collect()
    }

    /// 测试用：直接从 CSS 文本构造。
    #[cfg(test)]
    pub fn from_css(name: &str, text: &str) -> SyntaxTheme {
        SyntaxTheme {
            name: name.into(),
            rules: css::parse(text),
        }
    }

    /// 解析一套语法主题：用户目录 > 内置 > 空主题（全部走回退）。
    pub fn load(name: &str) -> SyntaxTheme {
        for dir in dirs() {
            if let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.css"))) {
                return SyntaxTheme {
                    name: name.into(),
                    rules: css::parse(&text),
                };
            }
        }
        if let Some((n, text)) = BUILTINS.iter().find(|(n, _)| *n == name) {
            return SyntaxTheme {
                name: n.to_string(),
                rules: css::parse(text),
            };
        }
        SyntaxTheme {
            name: name.into(),
            rules: Vec::new(),
        }
    }

    /// 全部可用语法主题名（内置 + 用户目录，去重排序）。
    pub fn available() -> Vec<String> {
        let mut names: Vec<String> = BUILTINS.iter().map(|(n, _)| n.to_string()).collect();
        for dir in dirs() {
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
        names.sort();
        names
    }

    /// 解析一个 token 类别。`langs`：fence token 与 syntect 规范语言名
    /// （均已小写，按优先级排列）。回退：语言特化 > 全局 > 页面主题
    /// syntax-* > 别名派生 > 默认。
    pub fn resolve(&self, langs: &[String], class: &str, page: &Scheme) -> SyntaxStyle {
        // 1: 语言特化规则（要求至少一条选择器长度 ≥ 2）。
        for lang in langs {
            let chain = [lang.as_str(), class];
            if let Some(st) = self.match_style(&chain, 2) {
                return st;
            }
        }
        // 2: 全局类别规则。
        if let Some(st) = self.match_style(&[class], 1) {
            return st;
        }
        // 3: 页面主题 syntax-*。
        if page.has_syntax_rule(class) {
            let c = page.style_for(&["body", &format!("syntax-{class}")]);
            return SyntaxStyle {
                fg: c.fg,
                bold: c.bold,
                italic: c.italic,
                underline: c.underline,
            };
        }
        // 4: 别名派生（目标类别重新走完整回退链；目标均非别名类，最多一跳）。
        if let Some(target) = alias_of(class) {
            return self.resolve(langs, target, page);
        }
        SyntaxStyle::default()
    }

    /// 折叠匹配 chain 的规则（按 特异性, 规则序 升序，后者覆盖前者）。
    /// `min_spec`：至少一条匹配选择器长度 ≥ min_spec 才视为命中。
    fn match_style(&self, chain: &[&str], min_spec: usize) -> Option<SyntaxStyle> {
        let mut matches: Vec<(usize, usize, &Props)> = Vec::new();
        for (idx, rule) in self.rules.iter().enumerate() {
            let mut best = 0;
            for sel in &rule.selectors {
                if selector_matches(sel, chain) {
                    best = best.max(sel.len());
                }
            }
            if best > 0 {
                matches.push((best, idx, &rule.props));
            }
        }
        if !matches.iter().any(|(spec, _, _)| *spec >= min_spec) {
            return None;
        }
        matches.sort_by_key(|(spec, idx, _)| (*spec, *idx));
        let mut st = SyntaxStyle::default();
        for (_, _, p) in matches {
            if let Some(c) = p.color {
                st.fg = Some(c);
            }
            if let Some(v) = p.bold {
                st.bold = v;
            }
            if let Some(v) = p.italic {
                st.italic = v;
            }
            if let Some(v) = p.underline {
                st.underline = v;
            }
        }
        Some(st)
    }
}
```

`src/style/mod.rs` 改为：

```rust
//! Style system: colors, CSS subset parsing, scheme registry.

pub mod color;
pub mod css;
pub mod scheme;
pub mod syntax;

pub use color::{ColorLevel, Rgb};
pub use scheme::{Computed, Scheme, DEFAULT_THEME};
pub use syntax::{SyntaxStyle, SyntaxTheme};
```

注意：本任务内 `assets/syntax-styles/*.css` 尚不存在，`include_str!` 会编译失败。
先执行 Step 5 的占位文件，再跑测试。

- [ ] **Step 5: 生成占位内置文件（Task 4 会用真实配色覆盖）**

Run: `mkdir -p assets/syntax-styles && for f in assets/styles/*.css; do n=$(basename "$f"); printf '/* placeholder */\nkeyword { color: #ffffff }\n' > "assets/syntax-styles/$n"; done`
Expected: 20 个文件生成。

- [ ] **Step 6: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS（含 syntax 模块 7 个新测试），零警告。

- [ ] **Step 7: Commit**

```bash
git add src/style/
git commit -m "✨ feat(syntax): add SyntaxTheme loader with per-class fallback chain"
```

---

### Task 2: highlight.rs 重写（16 类、per-language 缓存、字体样式）+ render_document 签名 + block.rs

**Files:**
- Modify: `src/highlight.rs`（整体重写）
- Modify: `src/render/layout/mod.rs`（`render_document` 加参、Renderer、测试调用点）
- Modify: `src/render/layout/block.rs`（span 应用 bold/italic/underline）
- Modify: `src/main.rs`（pipe 模式调用点临时接线，Task 3 完善）

- [ ] **Step 1: 写失败测试（追加到 highlight.rs 测试模块；先整体替换为下方最终代码则跳过编译失败步，直接跑红）**

先把 `src/highlight.rs` 的 `SCOPE_MAP`/结构保持原样，仅在文件末尾追加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Scheme, SyntaxTheme};

    fn empty_page() -> Scheme {
        Scheme { name: "p".into(), rules: vec![] }
    }

    #[test]
    fn carries_bold_italic_and_color() {
        let st = SyntaxTheme::from_css(
            "t",
            "keyword { color: #ff0000; font-weight: bold } comment { color: #00ff00; font-style: italic }",
        );
        let page = empty_page();
        let mut hl = Highlighter::new(&page, &st);
        let out = hl.highlight("// hi\nlet x = 1;", Some("rust"));
        let comment = out[0].iter().find(|s| s.text.contains("hi")).expect("comment span");
        assert!(comment.italic, "comment italic");
        assert_eq!(comment.fg, Rgb(0, 255, 0));
        let kw = out[1].iter().find(|s| s.text.contains("let")).expect("keyword span");
        assert!(kw.bold, "keyword bold");
        assert_eq!(kw.fg, Rgb(255, 0, 0));
    }

    #[test]
    fn constant_language_maps_to_constant_not_number() {
        let st = SyntaxTheme::from_css(
            "t",
            "constant { color: #123456 } number { color: #654321 }",
        );
        let page = empty_page();
        let mut hl = Highlighter::new(&page, &st);
        let out = hl.highlight("let x = true; let y = 42;", Some("rust"));
        let c = out[0].iter().find(|s| s.text.contains("true")).expect("constant span");
        assert_eq!(c.fg, Rgb(0x12, 0x34, 0x56));
        let n = out[0].iter().find(|s| s.text.contains("42")).expect("number span");
        assert_eq!(n.fg, Rgb(0x65, 0x43, 0x21));
    }

    #[test]
    fn per_language_override_and_alias_fence() {
        let st = SyntaxTheme::from_css(
            "t",
            "keyword { color: #010101 } rust keyword { color: #020202 }",
        );
        let page = empty_page();
        let mut hl = Highlighter::new(&page, &st);
        let rust = hl.highlight("let x;", Some("rs")); // fence 别名 → rust 规则
        let kw = rust[0].iter().find(|s| s.text.contains("let")).unwrap();
        assert_eq!(kw.fg, Rgb(0x02, 0x02, 0x02), "rs 命中 rust 特化");
        let py = hl.highlight("import os", Some("python"));
        let kw = py[0].iter().find(|s| s.text.contains("import")).unwrap();
        assert_eq!(kw.fg, Rgb(0x01, 0x01, 0x01), "python 走全局规则");
    }

    #[test]
    fn page_theme_fallback_when_syntax_theme_empty() {
        let st = SyntaxTheme::from_css("t", "");
        let page = Scheme {
            name: "p".into(),
            rules: crate::style::css::parse(
                "syntax-keyword { color: #fb4934; font-weight: bold }",
            ),
        };
        let mut hl = Highlighter::new(&page, &st);
        let out = hl.highlight("let x;", Some("rust"));
        let kw = out[0].iter().find(|s| s.text.contains("let")).unwrap();
        assert_eq!(kw.fg, Rgb(0xfb, 0x49, 0x34));
        assert!(kw.bold);
    }

    #[test]
    fn unknown_language_uses_default_fg() {
        let st = SyntaxTheme::from_css("t", "keyword { color: #ff0000 }");
        let page = Scheme {
            name: "p".into(),
            rules: crate::style::css::parse("pre { color: #aabbcc }"),
        };
        let mut hl = Highlighter::new(&page, &st);
        let out = hl.highlight("plain text here", Some("no-such-lang-xyz"));
        assert_eq!(out[0].len(), 1);
        assert_eq!(out[0][0].fg, Rgb(0xaa, 0xbb, 0xcc));
        assert!(!out[0][0].bold && !out[0][0].italic && !out[0][0].underline);
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cmd //c ".cargo-vc.bat test highlight"`
Expected: 编译失败（`HSpan`、`Highlighter::new` 双参等不存在）。

- [ ] **Step 3: 整体重写 `src/highlight.rs`**

```rust
//! syntect-based syntax highlighting driven by a `SyntaxTheme`
//! (`syntax-styles/*.css`)，per-language 懒构建 syntect Theme 并缓存。

use crate::style::{Rgb, Scheme, SyntaxTheme};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;
use std::collections::HashMap;
use std::str::FromStr;

/// 16 类 → sublime scope 选择器（顺序即 ThemeItem 顺序：宽泛在前）。
const SCOPE_MAP: &[(&str, &str)] = &[
    ("keyword", "keyword, storage"),
    ("string", "string"),
    ("comment", "comment"),
    ("function", "entity.name.function, support.function, meta.function-call variable.function"),
    ("type", "entity.name.type, entity.name.class, support.type, support.class, storage.type"),
    ("number", "constant.numeric"),
    ("operator", "keyword.operator"),
    ("variable", "variable"),
    ("constant", "constant.language, constant.other"),
    ("macro", "entity.name.macro, support.macro"),
    ("attribute", "entity.other.attribute-name, entity.name.attribute"),
    ("decorator", "storage.type.annotation, punctuation.definition.annotation"),
    ("module", "support.module, entity.name.module"),
    ("namespace", "entity.name.namespace"),
    ("punctuation", "punctuation"),
    ("label", "entity.name.label"),
];

fn to_syn(rgb: Rgb) -> SynColor {
    SynColor { r: rgb.0, g: rgb.1, b: rgb.2, a: 0xff }
}

fn from_syn(c: SynColor) -> Rgb {
    Rgb(c.r, c.g, c.b)
}

/// 一个高亮 span：前景色 + 字体样式 + 文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HSpan {
    pub fg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub text: String,
}

/// 绑定页面主题与语法主题的高亮器；按语言缓存构建好的 syntect Theme。
pub struct Highlighter<'a> {
    syntax_set: SyntaxSet,
    syntax_theme: &'a SyntaxTheme,
    scheme: &'a Scheme,
    themes: HashMap<String, Theme>,
    pub default_fg: Rgb,
}

impl<'a> Highlighter<'a> {
    pub fn new(scheme: &'a Scheme, syntax_theme: &'a SyntaxTheme) -> Highlighter<'a> {
        let default_fg = scheme
            .style_for(&["body", "pre"])
            .fg
            .unwrap_or(Rgb(212, 212, 212));
        Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            syntax_theme,
            scheme,
            themes: HashMap::new(),
            default_fg,
        }
    }

    /// 为某语言构建 syntect Theme（16 类经 SyntaxTheme::resolve 回退解析）；
    /// 完全未定义的类别跳过（走主题默认前景）。
    fn build_theme(&self, langs: &[String]) -> Theme {
        let pre = self.scheme.style_for(&["body", "pre"]);
        let mut items = Vec::new();
        for (class, scopes) in SCOPE_MAP {
            let st = self.syntax_theme.resolve(langs, class, self.scheme);
            if st.fg.is_none() && !st.bold && !st.italic && !st.underline {
                continue;
            }
            let Ok(sel) = ScopeSelectors::from_str(scopes) else {
                continue;
            };
            let mut font = FontStyle::empty();
            if st.bold {
                font |= FontStyle::BOLD;
            }
            if st.italic {
                font |= FontStyle::ITALIC;
            }
            if st.underline {
                font |= FontStyle::UNDERLINE;
            }
            items.push(ThemeItem {
                scope: sel,
                style: StyleModifier {
                    foreground: st.fg.map(to_syn),
                    background: None,
                    font_style: if font.is_empty() { None } else { Some(font) },
                },
            });
        }
        Theme {
            name: Some(self.syntax_theme.name.clone()),
            author: None,
            settings: ThemeSettings {
                foreground: Some(to_syn(self.default_fg)),
                background: pre.bg.map(to_syn),
                ..ThemeSettings::default()
            },
            scopes: items,
        }
    }

    /// Highlight a code block. Unknown languages render in `default_fg`.
    pub fn highlight(&mut self, code: &str, lang: Option<&str>) -> Vec<Vec<HSpan>> {
        let syntax = lang
            .and_then(|l| self.syntax_set.find_syntax_by_token(l))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        // 语言名候选：fence token 优先，syntect 规范名其次（均小写去重）。
        let canonical = syntax.name().to_lowercase();
        let mut langs: Vec<String> = Vec::new();
        if let Some(l) = lang {
            langs.push(l.to_lowercase());
        }
        if !langs.contains(&canonical) {
            langs.push(canonical);
        }
        let key = langs.join("\u{1}");
        if !self.themes.contains_key(&key) {
            let theme = self.build_theme(&langs);
            self.themes.insert(key.clone(), theme);
        }
        let theme = &self.themes[&key];
        let mut hl = HighlightLines::new(syntax, theme);
        let mut out = Vec::new();
        for line in code.lines() {
            let mut runs = Vec::new();
            match hl.highlight_line(line, &self.syntax_set) {
                Ok(regions) => {
                    for (style, text) in regions {
                        runs.push(HSpan {
                            fg: from_syn(style.foreground),
                            bold: style.font_style.contains(FontStyle::BOLD),
                            italic: style.font_style.contains(FontStyle::ITALIC),
                            underline: style.font_style.contains(FontStyle::UNDERLINE),
                            text: text.to_string(),
                        });
                    }
                }
                Err(_) => runs.push(HSpan {
                    fg: self.default_fg,
                    bold: false,
                    italic: false,
                    underline: false,
                    text: line.to_string(),
                }),
            }
            out.push(runs);
        }
        out
    }
}
```

（Step 1 追加的测试模块保留在文件末尾。）

- [ ] **Step 4: `render_document` 加 `syntax_theme` 参数**

`src/render/layout/mod.rs`：

```rust
use crate::style::{Computed, Scheme, SyntaxTheme};

pub struct Renderer<'a> {
    scheme: &'a Scheme,
    highlighter: Highlighter<'a>,
    width: usize,
    out: Vec<SLine>,
    cur: SLine,
    col: usize,
    links: Vec<String>,
}

pub fn render_document(
    doc: &Document,
    scheme: &Scheme,
    syntax_theme: &SyntaxTheme,
    width: usize,
    offset: usize,
) -> Rendered {
    let mut r = Renderer {
        scheme,
        highlighter: Highlighter::new(scheme, syntax_theme),
        width: width.max(20),
        out: Vec::new(),
        cur: Vec::new(),
        col: 0,
        links: Vec::new(),
    };
    r.render(doc, offset)
}
```

同文件测试辅助与直接调用点同步加参（共 5 处：`render_off` 内 1 处、
`code_block_bg_fills_line`、`code_block_rows_full_width`、`blockquote_bg_fills_rows`、
`blockquote_preserves_inner_backgrounds` 各 1 处）：

```rust
    fn render_off(src: &str, width: usize, offset: usize) -> Vec<String> {
        let doc = parse_document(src);
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let syntax = SyntaxTheme::load(&scheme.name);
        render_document(&doc, &scheme, &syntax, width, offset).plain
    }
```

直接调用处模式（4 处）：

```rust
        let syntax = SyntaxTheme::load(&scheme.name);
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
```

并 `use crate::style::SyntaxTheme;` 进测试模块（`use super::*` 已覆盖则跳过）。

- [ ] **Step 5: block.rs 应用字体样式 + 新增渲染测试**

`src/render/layout/block.rs` `code_block` 中 span 构建改为：

```rust
            let mut col = 0;
            for span in runs {
                col += text_width(&span.text);
                line.push(SSpan::new(
                    span.text.clone(),
                    Computed {
                        fg: Some(span.fg),
                        bg: pre.bg,
                        bold: span.bold,
                        italic: span.italic,
                        underline: span.underline,
                        ..Computed::default()
                    },
                ));
            }
```

在 `src/render/layout/mod.rs` 测试模块追加：

```rust
    #[test]
    fn code_block_comment_is_italic_via_page_fallback() {
        let doc = parse_document("```rust\n// hi\n```");
        let scheme = Scheme {
            name: "t".into(),
            rules: crate::style::css::parse(
                "pre { color: #111111; background: #222222 } syntax-comment { color: #333333; font-style: italic }",
            ),
        };
        let syntax = SyntaxTheme::from_css("t", "");
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
        let line = r
            .lines
            .iter()
            .find(|l| plain_of(l).contains("hi"))
            .expect("code line");
        let span = line.iter().find(|s| s.text.contains("hi")).expect("comment span");
        assert!(span.style.italic, "comment italic reaches the span");
        assert_eq!(span.style.fg, Some(crate::style::Rgb(0x33, 0x33, 0x33)));
    }
```

- [ ] **Step 6: main.rs 临时接线（保持编译）**

`src/main.rs`：import 行改为
`use style::{ColorLevel, Scheme, SyntaxTheme, DEFAULT_THEME};`
pipe 模式调用改为：

```rust
        let rendered = render::layout::render_document(
            &doc,
            &scheme,
            &SyntaxTheme::load(&theme_name),
            width,
            offset,
        );
```

- [ ] **Step 7: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS（highlight 5 个新测试 + comment italic 渲染测试），零警告。

- [ ] **Step 8: Commit**

```bash
git add src/highlight.rs src/render/layout/ src/main.rs
git commit -m "✨ feat(highlight): 16-class per-language highlighting with font styles"
```

---

### Task 3: config / CLI / TUI 接线

**Files:**
- Modify: `src/config.rs`（`syntax_theme` 字段）
- Modify: `src/main.rs`（`--syntax-theme`、`--list-syntax-themes`、名称解析）
- Modify: `src/app.rs`（App 字段、apply_scheme 跟随、run/event_loop 透传）

- [ ] **Step 1: 写失败测试**

`src/config.rs` 测试模块追加：

```rust
    #[test]
    fn parses_syntax_theme() {
        let cfg: Config = toml::from_str("syntax_theme = \"nord\"\n").unwrap();
        assert_eq!(cfg.syntax_theme.as_deref(), Some("nord"));
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.syntax_theme, None);
    }
```

`src/app.rs` 测试模块追加（复用现有 `test_app`，其页面主题为 DEFAULT_THEME）：

```rust
    #[test]
    fn syntax_theme_follows_page_theme_unless_overridden() {
        let mut app = test_app(1, 10);
        assert_eq!(app.syntax_theme.name, crate::style::DEFAULT_THEME);
        app.apply_scheme("nord");
        assert_eq!(app.syntax_theme.name, "nord", "无 override 时跟随页面主题");
        app.syntax_override = Some("gruvbox-dark".to_string());
        app.apply_scheme("dracula");
        assert_eq!(app.syntax_theme.name, "gruvbox-dark", "有 override 时固定");
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 编译失败（`syntax_theme`、`syntax_override` 字段不存在）。

- [ ] **Step 3: config.rs 加字段**

`Config` struct 中 `pub theme: Option<String>,` 之后加：

```rust
    /// Syntax theme name: builtin or `syntax-styles/<name>.css`.
    pub syntax_theme: Option<String>,
```

- [ ] **Step 4: app.rs 接线**

1. import：`use crate::style::{ColorLevel, Scheme, SyntaxTheme};`
2. `App` struct 在 `pub scheme: Scheme,` 后加：

```rust
    pub syntax_theme: SyntaxTheme,
    /// CLI/config 固定的语法主题；None 时跟随页面主题。
    pub syntax_override: Option<String>,
```

3. `App::new` 内 `App {` 之前加 `let syntax_theme = SyntaxTheme::load(&scheme.name);`，
   结构体字面量中加 `syntax_theme,` 与 `syntax_override: None,`。
4. `render_file` 调用改为：

```rust
        render_document(&doc, &self.scheme, &self.syntax_theme, width as usize, offset as usize)
```

5. `apply_scheme` 改为：

```rust
    pub fn apply_scheme(&mut self, name: &str) {
        self.scheme = Scheme::load(name);
        if self.syntax_override.is_none() {
            self.syntax_theme = SyntaxTheme::load(&self.scheme.name);
        }
        self.reload_reader();
        self.status = Some(format!("theme: {}", self.scheme.name));
    }
```

6. `run` 与 `event_loop` 末尾各加形参 `syntax_override: Option<String>`（`run` 透传），
   `event_loop` 内 `App::new(...)` 之后加：

```rust
    if let Some(name) = syntax_override {
        app.syntax_theme = SyntaxTheme::load(&name);
        app.syntax_override = Some(name);
    }
```

- [ ] **Step 5: main.rs CLI 与解析**

`Cli` 在 `list_themes` 后加：

```rust
    /// Syntax theme name: a builtin or `syntax-styles/<name>.css`.
    #[arg(long)]
    syntax_theme: Option<String>,

    /// List all available syntax themes and exit.
    #[arg(long)]
    list_syntax_themes: bool,
```

`main()` 中 `list_themes` 块之后加：

```rust
    if cli.list_syntax_themes {
        for name in SyntaxTheme::available() {
            println!("{name}");
        }
        return Ok(());
    }
```

`theme_name` 解析之后加：

```rust
    let syntax_override = cli.syntax_theme.or(cfg.syntax_theme);
    let syntax_theme = SyntaxTheme::load(syntax_override.as_deref().unwrap_or(&theme_name));
```

pipe 模式改用上面的 `syntax_theme` 变量（替换 Task 2 的临时
`SyntaxTheme::load(&theme_name)`）；TUI 的 `app::run(...)` 调用末尾加实参
`syntax_override`。

- [ ] **Step 6: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS，零警告。

- [ ] **Step 7: 手动冒烟**

Run: `cmd //c ".cargo-vc.bat build"` 然后
`./target/debug/mdview.exe --list-syntax-themes | head -5` 与
`printf '```rust\n// c\nlet x = true;\n```\n' | ./target/debug/mdview.exe --syntax-theme nord`
Expected: 列出 20 个内置语法主题名；pipe 输出含 ANSI 转义且含斜体序列 `\x1b[3m`。

- [ ] **Step 8: Commit**

```bash
git add src/config.rs src/main.rs src/app.rs
git commit -m "✨ feat(cli): syntax_theme config key, --syntax-theme flag, TUI theme follow"
```

---

### Task 4: 20 个内置语法主题 + example.css + build.bat

**Files:**
- Create: `assets/syntax-styles/*.css` × 20（脚本生成，覆盖 Task 1 占位）
- Create: `assets/syntax-styles/example.css`
- Modify: `build.bat`
- Modify: `src/style/syntax.rs`（内置完整性测试）

- [ ] **Step 1: 写失败测试（syntax.rs 测试模块追加）**

```rust
    #[test]
    fn builtin_syntax_themes_parse_and_cover_all_classes() {
        for name in SyntaxTheme::builtin_names() {
            let t = SyntaxTheme::load(name);
            assert!(!t.rules.is_empty(), "builtin {name} produced no rules");
            for &class in CLASSES {
                assert!(
                    t.rules.iter().any(|r| r
                        .selectors
                        .iter()
                        .any(|sel| sel.last().is_some_and(|s| s.as_str() == class))),
                    "builtin {name} missing class {class}"
                );
            }
        }
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cmd //c ".cargo-vc.bat test builtin_syntax"`
Expected: FAIL（占位文件缺类别）。

- [ ] **Step 3: 生成 20 个内置语法主题**

创建一次性脚本 `assets/syntax-styles/gen.py`（生成后删除，不入库）：

```python
#!/usr/bin/env python3
"""一次性：从 assets/styles/*.css 提取 syntax-* 配色，生成 assets/syntax-styles/*.css。
新增 9 类按别名派生取同主题色值；variable 取 p/body 前景色。"""
import pathlib
import re

ORDER = ["keyword", "string", "comment", "function", "type", "number", "operator",
         "variable", "constant", "macro", "attribute", "decorator", "module",
         "namespace", "punctuation", "label"]
ALIAS = {"constant": "number", "macro": "function", "decorator": "function",
         "attribute": "type", "module": "type", "namespace": "type",
         "punctuation": "operator", "label": "keyword"}

src_dir = pathlib.Path("assets/styles")
out_dir = pathlib.Path("assets/syntax-styles")

def rule_body(text, sel):
    m = re.search(rf"^{re.escape(sel)}\s*\{{([^}}]*)\}}", text, re.M)
    return m.group(1) if m else None

def color_of(text, sel):
    body = rule_body(text, sel)
    if not body:
        return None
    m = re.search(r"(?<!-)color:\s*(#[0-9a-fA-F]{3,8})", body)
    return m.group(1) if m else None

for css in sorted(src_dir.glob("*.css")):
    text = css.read_text(encoding="utf-8")
    base = {c: color_of(text, f"syntax-{c}") for c in ORDER if c != "variable"}
    italic = {c: ("font-style: italic" in (rule_body(text, f"syntax-{c}") or ""))
              for c in ORDER if c != "variable"}
    fallback = color_of(text, "p") or color_of(text, "body")
    lines = [f"/* mdview builtin syntax theme: {css.stem} (from md-styles/{css.name}) */"]
    for cls in ORDER:
        if cls == "variable":
            col, ital = fallback, False
        else:
            col = base[cls] or base.get(ALIAS.get(cls, "")) or fallback
            ital = italic[cls]
        decl = f"color: {col};"
        if ital:
            decl += " font-style: italic;"
        lines.append(f"{cls} {{ {decl} }}")
    out_dir.joinpath(css.name).write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("wrote", out_dir / css.name)
```

Run: `python assets/syntax-styles/gen.py && rm assets/syntax-styles/gen.py`
Expected: 打印 20 行 `wrote ...`；每个文件 16 条类别规则。
（环境无 python 时改用 `py` 或手工按同一规则生成。）

- [ ] **Step 4: 抽查生成结果**

Run: `cat assets/syntax-styles/gruvbox-dark.css`
Expected: 16 条规则；`keyword { color: #fb4934; }`、`comment { ... font-style: italic; }`、
`constant { color: #d3869b; }`（= number 色）、`variable { color: #ebdbb2; }`（= p 色）。

- [ ] **Step 5: example.css**

创建 `assets/syntax-styles/example.css`：

```css
/* mdview 语法高亮主题示例
 *
 * 用法：复制为 exe 同级 syntax-styles/<name>.css，修改后在 config.toml 设置
 *   syntax_theme = "<name>"
 * 或命令行 --syntax-theme <name>。未设置时跟随页面主题同名语法主题。
 *
 * 选择器 = 16 个 token 类别；语言名作祖先选择器可做 per-language 特化。
 * 支持属性：color、font-weight、font-style、text-decoration: underline。
 * 未定义类别的回退：页面主题 syntax-* → 别名派生 → 代码块默认前景。
 * 别名：constant→number、macro/decorator→function、attribute/module/namespace→type、
 *       punctuation→operator、label→keyword、variable→默认前景。
 */

/* ---- 基础类别 ---- */
keyword    { color: #fb4934; }              /* 关键字：let fn if match use ... */
string     { color: #b8bb26; }              /* 字符串/字符字面量 */
comment    { color: #928374; font-style: italic; }
function   { color: #b8bb26; }              /* 函数名/调用 */
type       { color: #fabd2f; }              /* 类型/类名 */
number     { color: #d3869b; }              /* 数字字面量 */
operator   { color: #8ec07c; }              /* 运算符 */

/* ---- 扩展类别 ---- */
variable    { color: #ebdbb2; }             /* 变量（一般保持默认前景） */
constant    { color: #d3869b; }             /* true/false/null 等语言常量 */
macro       { color: #fe8019; }             /* 宏 */
attribute   { color: #fabd2f; }             /* HTML 属性等 */
decorator   { color: #8ec07c; }             /* Python decorator / annotation */
module      { color: #fabd2f; }             /* 模块名 */
namespace   { color: #fabd2f; }             /* 命名空间 */
punctuation { color: #8ec07c; }             /* 标点 */
label       { color: #fb4934; }             /* label */

/* ---- 语言特化：语言名作祖先选择器 ----
 * 语言名匹配代码块 info string 原文（小写）与规范语言名（rs 命中 rust）。
 */
rust macro       { color: #fe8019; font-weight: bold; }
python decorator { color: #d3869b; }
```

注意：example.css 会作为名为 `example` 的用户主题出现在
`--list-syntax-themes` 中——刻意为之（即可复制模板又是可用示例）。

- [ ] **Step 6: build.bat 分发**

`build.bat` 第 30 行 `md-styles` mkdir 之后加：

```bat
if not exist "%~dp0bin\syntax-styles\" mkdir "%~dp0bin\syntax-styles" || exit /b 1
copy /y "%~dp0assets\syntax-styles\example.css" "%~dp0bin\syntax-styles\example.css" >NUL || exit /b 1
```

`:print_help` 的 Output 行改为：

```bat
echo Output: bin\mdview.exe + bin\config.toml + bin\md-styles\ + bin\syntax-styles\
```

`:write_config` 在 theme 行之后加：

```bat
>> "%CFG%" echo.
>> "%CFG%" echo # Syntax theme: builtin name or a css file in syntax-styles/ (default: follows page theme)
>> "%CFG%" echo # syntax_theme = "gruvbox-dark"
```

- [ ] **Step 7: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS（含内置完整性测试），零警告。

- [ ] **Step 8: 验证分发构建**

Run: `cmd //c "build.bat -d"`
Expected: `bin/syntax-styles/example.css` 存在；`bin/config.toml`（若不存在则新建）含 syntax_theme 注释。

- [ ] **Step 9: Commit**

```bash
git add assets/syntax-styles/ build.bat src/style/syntax.rs
git commit -m "✨ feat(syntax): 20 builtin syntax themes, example.css and distribution"
```

---

### Task 5: 文档（custom-themes 双语 + AGENTS.md）

**Files:**
- Modify: `docs/custom-themes.zh-CN.md`
- Modify: `docs/custom-themes.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: custom-themes.zh-CN.md 新增章节**

先读文件确认结构（主题目录/查找顺序章节附近），追加章节（标题层级与现有章节一致）：

```markdown
## 语法高亮主题（syntax-styles）

代码块的语法高亮配色独立于页面主题，由 exe 同级 `syntax-styles/<name>.css`
定义；内置 20 套与页面主题同名的语法主题。通过 `config.toml` 的
`syntax_theme` 键或 `--syntax-theme` 参数选择；未设置时自动跟随页面主题的
同名语法主题。`--list-syntax-themes` 列出全部可用语法主题。

CSS 语法与页面主题同一子集，选择器为 16 个 token 类别，可用语言名作祖先
选择器做 per-language 特化：

​```css
keyword { color: #fb4934; font-weight: bold; }
comment { color: #928374; font-style: italic; }
rust macro { color: #fe8019; }      /* 只对 rust 生效 */
​```

类别：keyword / string / comment / function / type / number / operator /
variable / constant / macro / attribute / decorator / module / namespace /
punctuation / label。

支持属性：`color`、`font-weight`、`font-style`、`text-decoration: underline`
（`background` 与 `line-through` 对代码 token 无效）。

逐类别回退顺序：语言特化规则 > 全局类别规则 > 页面主题 `syntax-*` 规则 >
别名派生 > 代码块默认前景。别名：constant→number、macro/decorator→function、
attribute/module/namespace→type、punctuation→operator、label→keyword、
variable→默认前景。

语言名同时匹配代码块 info string 原文（小写）与规范语言名——` ```rs `
也能命中 `rust` 规则。`bin/syntax-styles/example.css` 是带注释的完整模板。
```

- [ ] **Step 2: custom-themes.md 英文镜像**

同一位置追加英文版（内容与 Step 1 一一对应，标题 `## Syntax highlighting themes (syntax-styles)`）。

- [ ] **Step 3: AGENTS.md 更新**

- Architecture 代码块的流水线与目录列表中补一行：
  `- src/style/syntax.rs — syntax theme registry (syntax-styles/*.css, 16 token classes)`
- Conventions 的 Themes 条目后补一条：
  `- Syntax themes: builtins in assets/syntax-styles/*.css (paired with page
    themes); user themes in syntax-styles/ next to the exe. Selected via
    syntax_theme config key or --syntax-theme; falls back to the page theme's
    syntax-* rules, then alias derivation.`

- [ ] **Step 4: 全量测试 + 零警告确认**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS，零警告。

- [ ] **Step 5: Commit**

```bash
git add docs/custom-themes.md docs/custom-themes.zh-CN.md AGENTS.md
git commit -m "📝 docs(themes): document syntax-styles syntax themes"
```

---

## Self-Review 记录

- Spec 覆盖：16 类（Task 2 SCOPE_MAP）、per-language + 别名匹配（Task 1
  resolve + Task 2 langs 候选）、三级回退（Task 1）、字体样式修复（Task 2
  block.rs）、内置 20 套（Task 4）、config/CLI（Task 3）、example.css +
  build.bat（Task 4）、TUI 跟随/override（Task 3 apply_scheme）、文档
  （Task 5）。唯一偏差：`save_syntax_theme` 未实现（无写入口，YAGNI）。
- 类型一致性：`SyntaxStyle`、`SyntaxTheme::{load, available, resolve, from_css,
  builtin_names}`、`HSpan`、`render_document(doc, scheme, syntax_theme, width,
  offset)`、`Highlighter::new(&scheme, &syntax_theme)` 跨任务一致。
- 已知风险点（执行时验证）：`true`/`let` 等 token 在 sublime Rust/Python
  语法中的 scope 归属若与预期不符，调整测试断言的代码片段而非放宽断言。
