//! LaTeX-subset -> Unicode single-line approximation.
//!
//! Returns `None` when the source contains constructs we cannot map; the
//! caller then falls back to displaying the raw source.

/// Known command -> unicode replacement.
const COMMANDS: &[(&str, &str)] = &[
    // Greek lowercase
    ("alpha", "α"), ("beta", "β"), ("gamma", "γ"), ("delta", "δ"),
    ("epsilon", "ε"), ("zeta", "ζ"), ("eta", "η"), ("theta", "θ"),
    ("iota", "ι"), ("kappa", "κ"), ("lambda", "λ"), ("mu", "μ"),
    ("nu", "ν"), ("xi", "ξ"), ("pi", "π"), ("rho", "ρ"),
    ("sigma", "σ"), ("tau", "τ"), ("upsilon", "υ"), ("phi", "φ"),
    ("chi", "χ"), ("psi", "ψ"), ("omega", "ω"),
    // Greek uppercase
    ("Gamma", "Γ"), ("Delta", "Δ"), ("Theta", "Θ"), ("Lambda", "Λ"),
    ("Xi", "Ξ"), ("Pi", "Π"), ("Sigma", "Σ"), ("Phi", "Φ"),
    ("Psi", "Ψ"), ("Omega", "Ω"),
    // Big operators & symbols
    ("sum", "∑"), ("prod", "∏"), ("int", "∫"), ("oint", "∮"),
    ("sqrt", "√"), ("infty", "∞"), ("partial", "∂"), ("nabla", "∇"),
    ("pm", "±"), ("mp", "∓"), ("times", "×"), ("cdot", "·"), ("div", "÷"),
    // Relations
    ("le", "≤"), ("leq", "≤"), ("ge", "≥"), ("geq", "≥"),
    ("ne", "≠"), ("neq", "≠"), ("approx", "≈"), ("equiv", "≡"),
    ("propto", "∝"), ("ll", "≪"), ("gg", "≫"),
    // Sets & logic
    ("in", "∈"), ("notin", "∉"), ("subset", "⊂"), ("supset", "⊃"),
    ("subseteq", "⊆"), ("supseteq", "⊇"), ("cup", "∪"), ("cap", "∩"),
    ("emptyset", "∅"), ("forall", "∀"), ("exists", "∃"), ("neg", "¬"),
    ("land", "∧"), ("wedge", "∧"), ("lor", "∨"), ("vee", "∨"),
    // Arrows
    ("to", "→"), ("rightarrow", "→"), ("leftarrow", "←"),
    ("Rightarrow", "⇒"), ("Leftarrow", "⇐"), ("leftrightarrow", "↔"),
    ("mapsto", "↦"), ("uparrow", "↑"), ("downarrow", "↓"),
    // Misc
    ("ldots", "…"), ("cdots", "⋯"), ("dots", "…"),
    ("prime", "′"), ("degree", "°"), ("hbar", "ℏ"),
    ("ell", "ℓ"), ("Re", "ℜ"), ("Im", "ℑ"), ("aleph", "ℵ"),
    ("%", "%"), ("{", "{"), ("}", "}"), ("&", "&"), ("#", "#"), ("_", "_"),
    (",", " "), (";", " "), (" ", " "), ("quad", "  "), ("qquad", "    "),
];

const SUPERSCRIPTS: &[(char, char)] = &[
    ('0', '⁰'), ('1', '¹'), ('2', '²'), ('3', '³'), ('4', '⁴'),
    ('5', '⁵'), ('6', '⁶'), ('7', '⁷'), ('8', '⁸'), ('9', '⁹'),
    ('+', '⁺'), ('-', '⁻'), ('=', '⁼'), ('n', 'ⁿ'), ('i', 'ⁱ'),
];

const SUBSCRIPTS: &[(char, char)] = &[
    ('0', '₀'), ('1', '₁'), ('2', '₂'), ('3', '₃'), ('4', '₄'),
    ('5', '₅'), ('6', '₆'), ('7', '₇'), ('8', '₈'), ('9', '₉'),
    ('+', '₊'), ('-', '₋'), ('=', '₌'), ('a', 'ₐ'), ('e', 'ₑ'), ('o', 'ₒ'),
    ('x', 'ₓ'), ('i', 'ᵢ'), ('n', 'ₙ'), ('k', 'ₖ'), ('t', 'ₜ'),
];

fn map_char(c: char, table: &[(char, char)]) -> Option<char> {
    table.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// Try to render a LaTeX math snippet as a single unicode line.
pub fn to_unicode(src: &str) -> Option<String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => {
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end].is_ascii_alphabetic() {
                    end += 1;
                }
                if end == start {
                    // Single non-letter escape like \{ \}
                    if start < chars.len() {
                        let name = src_char_range(&chars, start, start + 1);
                        if let Some(rep) = COMMANDS.iter().find(|(k, _)| *k == name) {
                            out.push_str(rep.1);
                            i = start + 1;
                            continue;
                        }
                    }
                    return None;
                }
                let name: String = chars[start..end].iter().collect();
                if let Some(rep) = COMMANDS.iter().find(|(k, _)| *k == name) {
                    out.push_str(rep.1);
                } else if name == "frac" || name == "dfrac" || name == "tfrac" {
                    // \frac{a}{b} -> (a)/(b)
                    let (num, n1) = read_group(&chars, end)?;
                    let (den, n2) = read_group(&chars, n1)?;
                    let num = to_unicode(&num)?;
                    let den = to_unicode(&den)?;
                    out.push_str(&format!("({num})/({den})"));
                    i = n2;
                    continue;
                } else if matches!(
                    name.as_str(),
                    "text" | "mathrm" | "mathbf" | "mathit" | "operatorname"
                ) {
                    let (inner, n1) = read_group(&chars, end)?;
                    out.push_str(&inner);
                    i = n1;
                    continue;
                } else if matches!(name.as_str(), "left" | "right" | "big" | "Big") {
                    i = end;
                    continue;
                } else {
                    return None; // Unknown command: caller falls back to source.
                }
                i = end;
            }
            '^' | '_' => {
                let table = if c == '^' { SUPERSCRIPTS } else { SUBSCRIPTS };
                let (inner, next) = if i + 1 < chars.len() && chars[i + 1] == '{' {
                    read_group(&chars, i + 1)?
                } else if i + 1 < chars.len() {
                    (chars[i + 1].to_string(), i + 2)
                } else {
                    return None;
                };
                for ch in inner.chars() {
                    out.push(map_char(ch, table)?);
                }
                i = next;
            }
            '{' | '}' => {
                i += 1; // Bare grouping braces disappear.
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Some(out)
}

/// Read a `{...}` group (or single token) starting at `chars[i]`; returns
/// the inner text and the index just past the group.
fn read_group(chars: &[char], i: usize) -> Option<(String, usize)> {
    if i >= chars.len() {
        return None;
    }
    if chars[i] == '{' {
        let mut depth = 1;
        let mut j = i + 1;
        let mut inner = String::new();
        while j < chars.len() {
            match chars[j] {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((inner, j + 1));
                    }
                    inner.push('}');
                }
                ch => inner.push(ch),
            }
            j += 1;
        }
        None
    } else {
        Some((chars[i].to_string(), i + 1))
    }
}

fn src_char_range(chars: &[char], a: usize, b: usize) -> String {
    chars[a..b].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greek_and_operators() {
        assert_eq!(to_unicode(r"\alpha + \beta \le \sum_{i=1}^{n} x_i").unwrap(), "α + β ≤ ∑ᵢ₌₁ⁿ xᵢ");
    }

    #[test]
    fn frac() {
        assert_eq!(to_unicode(r"\frac{a}{b}").unwrap(), "(a)/(b)");
        assert_eq!(to_unicode(r"\frac{1}{2}").unwrap(), "(1)/(2)");
    }

    #[test]
    fn unknown_command_falls_back() {
        assert!(to_unicode(r"\matrix{1 & 2}").is_none());
    }

    #[test]
    fn plain_text_passthrough() {
        assert_eq!(to_unicode("E = mc^2").unwrap(), "E = mc²");
    }
}
