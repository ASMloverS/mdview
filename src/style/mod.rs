//! Style system: colors, CSS subset parsing, scheme registry.

pub mod color;
pub mod css;
pub mod scheme;
pub mod syntax;

pub use color::{ColorLevel, Rgb};
pub use scheme::{Computed, Scheme, DEFAULT_THEME};
// 由后续任务（高亮器接入）消费。
#[allow(unused_imports)]
pub use syntax::{SyntaxStyle, SyntaxTheme};
