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
        let canonical = syntax.name.to_lowercase();
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
