//! One-shot ANSI rendering for pipe/stdout mode.

use super::SLine;
use crate::style::{ColorLevel, Computed, Rgb};
use std::fmt::Write as _;

fn fg_escape(buf: &mut String, rgb: Rgb, level: ColorLevel) {
    match level {
        ColorLevel::True => {
            let _ = write!(buf, "\x1b[38;2;{};{};{}m", rgb.0, rgb.1, rgb.2);
        }
        ColorLevel::Ansi256 => {
            let _ = write!(buf, "\x1b[38;5;{}m", rgb.to_ansi256());
        }
        ColorLevel::Ansi16 => {
            let idx = rgb.to_ansi16();
            let code = if idx < 8 { 30 + idx } else { 90 + idx - 8 };
            let _ = write!(buf, "\x1b[{code}m");
        }
    }
}

fn bg_escape(buf: &mut String, rgb: Rgb, level: ColorLevel) {
    match level {
        ColorLevel::True => {
            let _ = write!(buf, "\x1b[48;2;{};{};{}m", rgb.0, rgb.1, rgb.2);
        }
        ColorLevel::Ansi256 => {
            let _ = write!(buf, "\x1b[48;5;{}m", rgb.to_ansi256());
        }
        ColorLevel::Ansi16 => {
            let idx = rgb.to_ansi16();
            let code = if idx < 8 { 40 + idx } else { 100 + idx - 8 };
            let _ = write!(buf, "\x1b[{code}m");
        }
    }
}

fn style_escape(buf: &mut String, style: &Computed, level: ColorLevel) {
    buf.push_str("\x1b[0m");
    if style.bold {
        buf.push_str("\x1b[1m");
    }
    if style.italic {
        buf.push_str("\x1b[3m");
    }
    if style.underline {
        buf.push_str("\x1b[4m");
    }
    if style.strike {
        buf.push_str("\x1b[9m");
    }
    if let Some(fg) = style.fg {
        fg_escape(buf, fg, level);
    }
    if let Some(bg) = style.bg {
        bg_escape(buf, bg, level);
    }
}

/// Render styled lines to an ANSI-escaped string. Links become OSC8
/// hyperlinks (clickable in modern terminals).
pub fn render_ansi(lines: &[SLine], level: ColorLevel) -> String {
    let mut buf = String::new();
    for line in lines {
        for span in line {
            style_escape(&mut buf, &span.style, level);
            if let Some(url) = &span.link {
                let _ = write!(buf, "\x1b]8;;{url}\x1b\\{}\x1b]8;;\x1b\\", span.text);
            } else {
                buf.push_str(&span.text);
            }
        }
        buf.push_str("\x1b[0m\n");
    }
    buf
}
