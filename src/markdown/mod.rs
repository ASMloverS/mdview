//! Markdown parsing into a document IR.

pub mod ir;
pub mod parse;

pub use ir::{Align, Block, Document, Inline, ListItem};
pub use parse::{parse_document, plain_text};
