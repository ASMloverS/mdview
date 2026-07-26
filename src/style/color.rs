//! Terminal color representation, capability detection and degradation.

/// RGB true color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn from_hex(s: &str) -> Option<Rgb> {
        let s = s.strip_prefix('#')?;
        let byte = |h: &str| u8::from_str_radix(h, 16).ok();
        match s.len() {
            3 => {
                let r = byte(&s[0..1].repeat(2))?;
                let g = byte(&s[1..2].repeat(2))?;
                let b = byte(&s[2..3].repeat(2))?;
                Some(Rgb(r, g, b))
            }
            6 | 8 => {
                // Ignore alpha channel if present (#rrggbbaa).
                let r = byte(&s[0..2])?;
                let g = byte(&s[2..4])?;
                let b = byte(&s[4..6])?;
                Some(Rgb(r, g, b))
            }
            _ => None,
        }
    }

    pub fn named(name: &str) -> Option<Rgb> {
        let c = match name.to_ascii_lowercase().as_str() {
            "black" => Rgb(0, 0, 0),
            "white" => Rgb(255, 255, 255),
            "red" => Rgb(255, 0, 0),
            "green" => Rgb(0, 128, 0),
            "blue" => Rgb(0, 0, 255),
            "yellow" => Rgb(255, 255, 0),
            "orange" => Rgb(255, 165, 0),
            "purple" => Rgb(128, 0, 128),
            "pink" => Rgb(255, 192, 203),
            "cyan" | "aqua" => Rgb(0, 255, 255),
            "magenta" | "fuchsia" => Rgb(255, 0, 255),
            "gray" | "grey" => Rgb(128, 128, 128),
            "silver" => Rgb(192, 192, 192),
            "teal" => Rgb(0, 128, 128),
            "lime" => Rgb(0, 255, 0),
            "navy" => Rgb(0, 0, 128),
            "maroon" => Rgb(128, 0, 0),
            "olive" => Rgb(128, 128, 0),
            _ => return None,
        };
        Some(c)
    }

    /// Nearest xterm-256 palette index.
    pub fn to_ansi256(self) -> u8 {
        let Rgb(r, g, b) = self;
        // Grayscale ramp check.
        if r == g && g == b {
            if r < 8 {
                return 16;
            }
            if r > 248 {
                return 231;
            }
            return 232 + ((r - 8) as f32 / 247.0 * 24.0).round() as u8;
        }
        let q = |v: u8| ((v as f32 / 255.0) * 5.0).round() as u8;
        16 + 36 * q(r) + 6 * q(g) + q(b)
    }

    /// Nearest of the 16 basic ANSI colors.
    pub fn to_ansi16(self) -> u8 {
        const PALETTE: [Rgb; 16] = [
            Rgb(0, 0, 0),
            Rgb(128, 0, 0),
            Rgb(0, 128, 0),
            Rgb(128, 128, 0),
            Rgb(0, 0, 128),
            Rgb(128, 0, 128),
            Rgb(0, 128, 128),
            Rgb(192, 192, 192),
            Rgb(128, 128, 128),
            Rgb(255, 0, 0),
            Rgb(0, 255, 0),
            Rgb(255, 255, 0),
            Rgb(0, 0, 255),
            Rgb(255, 0, 255),
            Rgb(0, 255, 255),
            Rgb(255, 255, 255),
        ];
        let dist = |c: Rgb| {
            let dr = c.0 as i32 - self.0 as i32;
            let dg = c.1 as i32 - self.1 as i32;
            let db = c.2 as i32 - self.2 as i32;
            dr * dr + dg * dg + db * db
        };
        (0..16).min_by_key(|&i| dist(PALETTE[i])).unwrap() as u8
    }
}

/// Terminal color capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorLevel {
    True,
    Ansi256,
    Ansi16,
}

impl ColorLevel {
    /// Detect from `COLORTERM` / `TERM`.
    pub fn detect() -> ColorLevel {
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return ColorLevel::True;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        if term.contains("256color") {
            ColorLevel::Ansi256
        } else {
            ColorLevel::Ansi16
        }
    }

    pub fn to_ratatui(self, rgb: Rgb) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            ColorLevel::True => Color::Rgb(rgb.0, rgb.1, rgb.2),
            ColorLevel::Ansi256 => Color::Indexed(rgb.to_ansi256()),
            ColorLevel::Ansi16 => Color::Indexed(rgb.to_ansi16()),
        }
    }
}
