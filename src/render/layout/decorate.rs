//! Decoration primitives shared by text and block rendering.

use super::text_width;
use crate::render::{SLine, SSpan};
use crate::style::{Computed, Rgb};

/// Display width of a styled line.
#[allow(dead_code)] // used by later decoration tasks
pub fn line_width(line: &SLine) -> usize {
    line.iter().map(|s| text_width(&s.text)).sum()
}

/// Pad a line out to `width` columns with a background-colored span.
#[allow(dead_code)] // used by later decoration tasks
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
