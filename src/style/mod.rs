//! Style system: colors, CSS subset parsing, scheme registry.

pub mod color;
pub mod css;
pub mod scheme;
pub mod syntax;

pub use color::{ColorLevel, Rgb};
pub use scheme::{Computed, Scheme, DEFAULT_THEME};
pub use syntax::SyntaxTheme;
