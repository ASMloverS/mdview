//! Syntax highlighting themes: exe 同级 `syntax-styles/<name>.css` 用户主题，
//! 内置主题 embed 自 `assets/syntax-styles/`。与页面主题（md-styles）解耦。

use super::color::Rgb;
use super::css::{self, Props, Rule};
use super::scheme::{exe_dir, selector_matches, Scheme};
use std::path::PathBuf;

/// 16 个 token 类别（顺序即 syntect ThemeItem 顺序：宽泛在前）。
/// 仅完整性测试使用；高亮器迭代自身的 SCOPE_MAP。
#[cfg(test)]
pub const CLASSES: &[&str] = &[
    "keyword", "string", "comment", "function", "type", "number", "operator",
    "variable", "constant", "macro", "attribute", "decorator", "module",
    "namespace", "punctuation", "label",
];

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
            if let Some(st) = match_rules(&self.rules, &chain, 2) {
                return st;
            }
        }
        // 2: 全局类别规则。
        if let Some(st) = match_rules(&self.rules, &[class], 1) {
            return st;
        }
        // 3: 页面主题 syntax-*（body 链；未设 color 时 fg 保持 None）。
        let leaf = format!("syntax-{class}");
        let chain = ["body", leaf.as_str()];
        if let Some(st) = match_rules(&page.rules, &chain, 1) {
            return st;
        }
        // 4: 别名派生（目标类别重新走完整回退链；目标均非别名类，最多一跳）。
        if let Some(target) = alias_of(class) {
            return self.resolve(langs, target, page);
        }
        SyntaxStyle::default()
    }
}

/// 折叠匹配 chain 的规则（按 特异性, 规则序 升序，后者覆盖前者）。
/// `min_spec`：至少一条匹配选择器长度 ≥ min_spec 才视为命中。
fn match_rules(rules: &[Rule], chain: &[&str], min_spec: usize) -> Option<SyntaxStyle> {
    let mut matches: Vec<(usize, usize, &Props)> = Vec::new();
    for (idx, rule) in rules.iter().enumerate() {
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
    fn page_fallback_without_color_keeps_fg_none() {
        let t = SyntaxTheme::from_css("t", "");
        let p = page("syntax-comment { font-style: italic }");
        let st = t.resolve(&["rust".to_string()], "comment", &p);
        assert_eq!(st.fg, None, "页面规则未设 color 时保持默认前景");
        assert!(st.italic);
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
}
