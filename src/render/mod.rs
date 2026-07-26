//! Rendering: layout engine and ANSI one-shot output.

pub mod ansi;
pub mod layout;

use crate::style::{ColorLevel, Computed};

/// A styled span of text; `link` carries an URL for OSC8 output.
#[derive(Debug, Clone)]
pub struct SSpan {
    pub text: String,
    pub style: Computed,
    pub link: Option<String>,
}

impl SSpan {
    pub fn new(text: impl Into<String>, style: Computed) -> SSpan {
        SSpan {
            text: text.into(),
            style,
            link: None,
        }
    }

    pub fn linked(text: impl Into<String>, style: Computed, url: String) -> SSpan {
        SSpan {
            text: text.into(),
            style,
            link: Some(url),
        }
    }
}

/// One terminal line as a list of styled spans.
pub type SLine = Vec<SSpan>;

/// Rendered document: styled lines plus their plain text.
#[derive(Debug, Default)]
pub struct Rendered {
    pub lines: Vec<SLine>,
    /// Plain text of each line, for search.
    pub plain: Vec<String>,
}

/// Convert to ratatui types applying the terminal color level.
pub fn to_ratatui_line(line: &SLine, level: ColorLevel) -> ratatui::text::Line<'static> {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;
    let spans: Vec<Span> = line
        .iter()
        .map(|s| {
            let mut style = Style::default();
            if let Some(fg) = s.style.fg {
                style = style.fg(level.to_ratatui(fg));
            }
            if let Some(bg) = s.style.bg {
                style = style.bg(level.to_ratatui(bg));
            }
            let mut mods = Modifier::empty();
            if s.style.bold {
                mods |= Modifier::BOLD;
            }
            if s.style.italic {
                mods |= Modifier::ITALIC;
            }
            if s.style.underline {
                mods |= Modifier::UNDERLINED;
            }
            if s.style.strike {
                mods |= Modifier::CROSSED_OUT;
            }
            Span::styled(s.text.clone(), style.add_modifier(mods))
        })
        .collect();
    ratatui::text::Line::from(spans)
}

pub fn plain_of(line: &SLine) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}
