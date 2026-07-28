//! Color scheme registry: builtin schemes, user CSS loading, style resolution.

use super::color::Rgb;
use super::css::{self, Props, Rule};
use std::path::PathBuf;

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
    ("catppuccin-mocha", include_str!("../../assets/styles/catppuccin-mocha.css")),
    ("kanagawa", include_str!("../../assets/styles/kanagawa.css")),
    ("rose-pine", include_str!("../../assets/styles/rose-pine.css")),
    ("everforest", include_str!("../../assets/styles/everforest.css")),
    ("one-dark", include_str!("../../assets/styles/one-dark.css")),
    ("monokai", include_str!("../../assets/styles/monokai.css")),
    ("ayu-dark", include_str!("../../assets/styles/ayu-dark.css")),
    ("github-dark", include_str!("../../assets/styles/github-dark.css")),
    ("github-light", include_str!("../../assets/styles/github-light.css")),
    ("solarized-light", include_str!("../../assets/styles/solarized-light.css")),
    ("gruvbox-light", include_str!("../../assets/styles/gruvbox-light.css")),
    ("catppuccin-latte", include_str!("../../assets/styles/catppuccin-latte.css")),
    ("rose-pine-dawn", include_str!("../../assets/styles/rose-pine-dawn.css")),
    ("everforest-light", include_str!("../../assets/styles/everforest-light.css")),
    ("ayu-light", include_str!("../../assets/styles/ayu-light.css")),
];

pub const DEFAULT_THEME: &str = "gruvbox-dark";

/// User CSS theme directory: `md-styles` next to the executable
/// (cwd-relative fallback when the exe location is unavailable).
fn style_dirs() -> Vec<PathBuf> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    vec![dir.join("md-styles")]
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

#[derive(Debug, Clone)]
pub struct Scheme {
    pub name: String,
    pub rules: Vec<Rule>,
}

impl Scheme {
    #[cfg(test)]
    pub fn builtin_names() -> Vec<&'static str> {
        BUILTINS.iter().map(|(n, _)| *n).collect()
    }

    /// Resolve a scheme by name: user `md-styles/<name>.css` next to the
    /// executable first, then builtins. A user file with the same name as
    /// a builtin overrides the builtin. Unknown names fall back to default.
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

    /// All loadable scheme names: builtins plus user CSS files in
    /// `md-styles/` next to the executable.
    pub fn available() -> Vec<String> {
        let mut names = available_in(&style_dirs());
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
        let dirs = [cwd_dir, exe_dir];

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
        assert_eq!(names.iter().filter(|n| *n == "nord").count(), 1);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn style_dirs_only_next_to_executable() {
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(style_dirs(), vec![exe_dir.join("md-styles")]);
    }

    #[test]
    fn default_theme_is_gruvbox_dark() {
        assert_eq!(DEFAULT_THEME, "gruvbox-dark");
        assert!(!Scheme::load(DEFAULT_THEME).rules.is_empty());
    }

    #[test]
    fn new_builtin_schemes_registered() {
        let names = Scheme::builtin_names();
        for expected in [
            "catppuccin-mocha",
            "kanagawa",
            "rose-pine",
            "everforest",
            "one-dark",
            "monokai",
            "ayu-dark",
            "github-dark",
            "catppuccin-latte",
            "rose-pine-dawn",
            "everforest-light",
            "ayu-light",
        ] {
            assert!(names.contains(&expected), "missing builtin {expected}");
        }
    }

    #[test]
    fn builtin_schemes_cover_template_elements() {
        const TAGS: &[&str] = &[
            "body", "h1", "h2", "h3", "h4", "h5", "h6", "p", "strong", "em", "del",
            "code", "pre", "a", "blockquote", "li", "table", "th", "td", "hr", "img",
            "math", "footnote", "cursor",
        ];
        const SYNTAX: &[&str] = &[
            "keyword", "string", "comment", "function", "type", "number", "operator",
        ];
        for name in Scheme::builtin_names() {
            let s = Scheme::load(name);
            for tag in TAGS {
                let c = s.element(tag);
                assert!(
                    c.fg.is_some()
                        || c.bg.is_some()
                        || c.border.is_some()
                        || c.bold
                        || c.italic
                        || c.underline
                        || c.strike,
                    "builtin {name} missing style for {tag}"
                );
            }
            for class in SYNTAX {
                assert!(
                    s.syntax_color(class).is_some(),
                    "builtin {name} missing syntax-{class}"
                );
            }
        }
    }

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
}
