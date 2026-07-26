//! syntect-based syntax highlighting driven by the active scheme's
//! `syntax-*` CSS colors.

use crate::style::{Rgb, Scheme};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SynColor, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;
use std::str::FromStr;

/// Our 7 syntax classes mapped to sublime scope selectors.
const SCOPE_MAP: &[(&str, &str)] = &[
    ("keyword", "keyword, storage"),
    ("string", "string"),
    ("comment", "comment"),
    ("function", "entity.name.function, support.function, meta.function-call variable.function"),
    ("type", "entity.name.type, entity.name.class, support.type, support.class, storage.type"),
    ("number", "constant.numeric, constant.language"),
    ("operator", "keyword.operator"),
];

fn to_syn(rgb: Rgb) -> SynColor {
    SynColor {
        r: rgb.0,
        g: rgb.1,
        b: rgb.2,
        a: 0xff,
    }
}

fn from_syn(c: SynColor) -> Rgb {
    Rgb(c.r, c.g, c.b)
}

/// Lazily-created highlighter bound to one scheme.
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
    pub default_fg: Rgb,
}

impl Highlighter {
    pub fn new(scheme: &Scheme) -> Highlighter {
        let pre = scheme.style_for(&["body", "pre"]);
        let default_fg = pre.fg.unwrap_or(Rgb(212, 212, 212));

        let mut items = Vec::new();
        for (class, scopes) in SCOPE_MAP {
            if let Some(color) = scheme.syntax_color(class) {
                if let Ok(sel) = ScopeSelectors::from_str(scopes) {
                    items.push(ThemeItem {
                        scope: sel,
                        style: StyleModifier {
                            foreground: Some(to_syn(color)),
                            background: None,
                            font_style: if *class == "comment" {
                                Some(FontStyle::ITALIC)
                            } else {
                                None
                            },
                        },
                    });
                }
            }
        }

        let theme = Theme {
            name: Some(scheme.name.clone()),
            author: None,
            settings: ThemeSettings {
                foreground: Some(to_syn(default_fg)),
                background: pre.bg.map(to_syn),
                ..ThemeSettings::default()
            },
            scopes: items,
        };

        Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme,
            default_fg,
        }
    }

    /// Highlight a code block; each returned line is a list of
    /// `(color, text)` runs. Unknown languages render in `default_fg`.
    pub fn highlight(&self, code: &str, lang: Option<&str>) -> Vec<Vec<(Rgb, String)>> {
        let syntax = lang
            .and_then(|l| self.syntax_set.find_syntax_by_token(l))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let mut hl = HighlightLines::new(syntax, &self.theme);
        let mut out = Vec::new();
        for line in code.lines() {
            let mut runs = Vec::new();
            match hl.highlight_line(line, &self.syntax_set) {
                Ok(regions) => {
                    for (style, text) in regions {
                        runs.push((from_syn(style.foreground), text.to_string()));
                    }
                }
                Err(_) => runs.push((self.default_fg, line.to_string())),
            }
            out.push(runs);
        }
        out
    }
}
