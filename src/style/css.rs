//! Minimal CSS subset parser.
//!
//! Only rule sets of the form `selector, selector { prop: value; ... }` are
//! understood. Selectors are whitespace/`>` separated element names
//! (classes, ids and pseudo-classes are stripped). Unknown properties and
//! malformed input are ignored silently.

use super::color::Rgb;

/// Parsed style properties of one rule.
#[derive(Debug, Clone, Default)]
pub struct Props {
    pub color: Option<Rgb>,
    pub background: Option<Rgb>,
    pub border_color: Option<Rgb>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
}

/// One CSS rule: a list of selectors sharing the same declarations.
/// Each selector is a chain of element names, e.g. `["pre", "code"]`.
#[derive(Debug, Clone)]
pub struct Rule {
    pub selectors: Vec<Vec<String>>,
    pub props: Props,
}

/// Normalize one compound selector part: keep only the element name.
fn element_name(part: &str) -> Option<String> {
    let name: String = part
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Parse a selector like `pre code` or `table > td` into a name chain.
fn parse_selector(sel: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = sel
        .split(|c| c == ' ' || c == '>' || c == '\t')
        .filter_map(element_name)
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn parse_color(value: &str) -> Option<Rgb> {
    let v = value.trim();
    if let Some(rgb) = Rgb::from_hex(v) {
        return Some(rgb);
    }
    if let Some(inner) = v.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let nums: Vec<u8> = inner
            .split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        if nums.len() == 3 {
            return Some(Rgb(nums[0], nums[1], nums[2]));
        }
        return None;
    }
    Rgb::named(v)
}

fn parse_declarations(body: &str, props: &mut Props) {
    for decl in body.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        match name.as_str() {
            "color" => props.color = parse_color(&value),
            "background" | "background-color" => props.background = parse_color(&value),
            "border-color" => props.border_color = parse_color(&value),
            "font-weight" => props.bold = Some(matches!(value.as_str(), "bold" | "bolder" | "600" | "700" | "800" | "900")),
            "font-style" => props.italic = Some(value == "italic" || value == "oblique"),
            "text-decoration" | "text-decoration-line" => {
                props.underline = Some(value.contains("underline"));
                props.strike = Some(value.contains("line-through") || value.contains("strikethrough"));
            }
            _ => {}
        }
    }
}

/// Parse a full stylesheet into rules. Malformed chunks are skipped.
pub fn parse(css: &str) -> Vec<Rule> {
    // Strip comments.
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find("*/") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    out.push_str(rest);

    let mut rules = Vec::new();
    let mut s = out.as_str();
    while let Some(open) = s.find('{') {
        let head = &s[..open];
        let Some(close) = s[open..].find('}') else { break };
        let body = &s[open + 1..open + close];
        s = &s[open + close + 1..];

        let selectors: Vec<Vec<String>> = head
            .split(',')
            .filter_map(parse_selector)
            .collect();
        if selectors.is_empty() {
            continue;
        }
        let mut props = Props::default();
        parse_declarations(body, &mut props);
        rules.push(Rule { selectors, props });
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_rule() {
        let rules = parse("h1 { color: #ff0000; font-weight: bold }");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].selectors, vec![vec!["h1".to_string()]]);
        assert_eq!(rules[0].props.color, Some(Rgb(255, 0, 0)));
        assert_eq!(rules[0].props.bold, Some(true));
    }

    #[test]
    fn parses_descendant_and_multi_selector() {
        let rules = parse("pre code, a:hover { color: rgb(1, 2, 3); }");
        assert_eq!(rules[0].selectors.len(), 2);
        assert_eq!(rules[0].selectors[0], vec!["pre", "code"]);
        assert_eq!(rules[0].selectors[1], vec!["a"]);
        assert_eq!(rules[0].props.color, Some(Rgb(1, 2, 3)));
    }

    #[test]
    fn strips_comments_and_ignores_garbage() {
        let rules = parse("/* hi */ p { unknown-prop: 1; text-decoration: underline; } ??? {");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].props.underline, Some(true));
    }

    #[test]
    fn hex_and_named_colors() {
        assert_eq!(Rgb::from_hex("#0f8"), Some(Rgb(0, 255, 136)));
        assert_eq!(Rgb::from_hex("#00ff88ff"), Some(Rgb(0, 255, 136)));
        assert_eq!(parse_color("teal"), Some(Rgb(0, 128, 128)));
    }
}
