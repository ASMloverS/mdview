//! Style system: colors, CSS subset parsing, scheme registry.

pub mod color;
pub mod css;
pub mod scheme;

pub use color::{ColorLevel, Rgb};
pub use scheme::{Computed, Scheme, DEFAULT_THEME};
