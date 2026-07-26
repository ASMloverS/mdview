//! Color scheme registry: builtin schemes, user CSS loading, style resolution.

use super::color::Rgb;
use super::css::{self, Props, Rule};

/// A fully resolved style for one IR node.
#[derive(Debug, Clone, Copy, Default)]
pub struct Computed {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub border: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

/// Fallback style used when a scheme defines nothing.
fn base_style() -> Computed {
    Computed {
        fg: Some(Rgb(212, 212, 212)),
        ..Computed::default()
    }
}

/// Builtin schemes embedded into the binary.
const BUILTINS: &[(&str, &str)] = &[
    ("tokyo-night", include_str!("../../assets/styles/tokyo-night.css")),
    ("dracula", include_str!("../../assets/styles/dracula.css")),
    ("gruvbox-dark", include_str!("../../assets/styles/gruvbox-dark.css")),
    ("nord", include_str!("../../assets/styles/nord.css")),
    ("solarized-dark", include_str!("../../assets/styles/solarized-dark.css")),
    ("github-light", include_str!("../../assets/styles/github-light.css")),
    ("solarized-light", include_str!("../../assets/styles/solarized-light.css")),
    ("gruvbox-light", include_str!("../../assets/styles/gruvbox-light.css")),
];

pub const DEFAULT_THEME: &str = "tokyo-night";

#[derive(Debug, Clone)]
pub struct Scheme {
    pub name: String,
    pub rules: Vec<Rule>,
}

impl Scheme {
    pub fn builtin_names() -> Vec<&'static str> {
        BUILTINS.iter().map(|(n, _)| *n).collect()
    }

    /// Resolve a scheme by name: builtin first, then `md-styles/<name>.css`
    /// under the current directory. A user file with the same name as a
    /// builtin overrides the builtin. Unknown names fall back to default.
    pub fn load(name: &str) -> Scheme {
        let user_path = std::path::Path::new("md-styles").join(format!("{name}.css"));
        if let Ok(css_text) = std::fs::read_to_string(&user_path) {
            return Scheme {
                name: name.to_string(),
                rules: css::parse(&css_text),
            };
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

    /// All loadable scheme names: builtins plus user CSS files in md-styles/.
    pub fn available() -> Vec<String> {
        let mut names: Vec<String> = BUILTINS.iter().map(|(n, _)| n.to_string()).collect();
        if let Ok(dir) = std::fs::read_dir("md-styles") {
            for entry in dir.flatten() {
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
        names.sort();
        names
    }

    /// Compute the style for a node given its ancestor chain, root first,
    /// leaf last, e.g. `["body", "blockquote", "p", "strong"]`.
    pub fn style_for(&self, chain: &[&str]) -> Computed {
        let mut computed = base_style();
        // Collect (specificity, rule index, props) of matching rules.
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
        matches.sort_by_key(|(spec, idx, _)| (*spec, *idx));
        for (_, _, props) in matches {
            apply(props, &mut computed);
        }
        computed
    }

    /// Convenience: style of a single element under body,
    /// e.g. `element("h1")`.
    pub fn element(&self, tag: &str) -> Computed {
        self.style_for(&["body", tag])
    }

    /// Syntax highlighting palette from the `syntax-*` rules.
    pub fn syntax_color(&self, class: &str) -> Option<Rgb> {
        self.style_for(&["body", &format!("syntax-{class}")]).fg
    }
}

fn apply(props: &Props, c: &mut Computed) {
    if let Some(v) = props.color {
        c.fg = Some(v);
    }
    if let Some(v) = props.background {
        c.bg = Some(v);
    }
    if let Some(v) = props.border_color {
        c.border = Some(v);
    }
    if let Some(v) = props.bold {
        c.bold = v;
    }
    if let Some(v) = props.italic {
        c.italic = v;
    }
    if let Some(v) = props.underline {
        c.underline = v;
    }
    if let Some(v) = props.strike {
        c.strike = v;
    }
}

/// A selector matches when its last element equals the leaf and the
/// remaining parts appear, in order, among the ancestors.
fn selector_matches(sel: &[String], chain: &[&str]) -> bool {
    if sel.is_empty() || chain.is_empty() {
        return false;
    }
    if sel.last().unwrap() != chain.last().unwrap() {
        return false;
    }
    let mut ancestors = &chain[..chain.len() - 1];
    for part in &sel[..sel.len() - 1] {
        match ancestors.iter().position(|a| a == part) {
            Some(pos) => ancestors = &ancestors[pos + 1..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme(css_text: &str) -> Scheme {
        Scheme {
            name: "test".into(),
            rules: css::parse(css_text),
        }
    }

    #[test]
    fn element_and_descendant_specificity() {
        let s = scheme("code { color: #111111 } pre code { color: #222222 }");
        let inline = s.style_for(&["body", "p", "code"]);
        let block = s.style_for(&["body", "pre", "code"]);
        assert_eq!(inline.fg, Some(Rgb(0x11, 0x11, 0x11)));
        assert_eq!(block.fg, Some(Rgb(0x22, 0x22, 0x22)));
    }

    #[test]
    fn later_rule_overrides_earlier() {
        let s = scheme("h1 { color: #ff0000 } h1 { color: #00ff00 }");
        assert_eq!(s.element("h1").fg, Some(Rgb(0, 255, 0)));
    }

    #[test]
    fn unmatched_selector_ignored() {
        let s = scheme("table td { color: #123456 }");
        let p = s.style_for(&["body", "p"]);
        assert_eq!(p.fg, base_style().fg);
    }

    #[test]
    fn builtin_schemes_parse() {
        for name in Scheme::builtin_names() {
            let s = Scheme::load(name);
            assert!(!s.rules.is_empty(), "builtin {name} produced no rules");
        }
    }
}
