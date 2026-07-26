//! Intermediate representation of a parsed markdown document.

/// Table column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// Inline content.
#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    Code(String),
    Strong(Vec<Inline>),
    Em(Vec<Inline>),
    Del(Vec<Inline>),
    Link { url: String, content: Vec<Inline> },
    Image { url: String, alt: String },
    Math(String),
    FootnoteRef(String),
    SoftBreak,
    HardBreak,
}

/// Block content.
#[derive(Debug, Clone)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, content: Vec<Inline> },
    CodeBlock { lang: Option<String>, code: String },
    BlockQuote(Vec<Block>),
    List { ordered: bool, start: u64, items: Vec<ListItem> },
    Table { head: Vec<Vec<Inline>>, aligns: Vec<Align>, rows: Vec<Vec<Vec<Inline>>> },
    Rule,
    MathBlock(String),
    FootnoteDef { label: String, blocks: Vec<Block> },
}

/// One list item; `checked` is `Some` for task list items.
#[derive(Debug, Clone)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A parsed document; `meta` holds raw frontmatter text if present.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub meta: Option<String>,
    pub blocks: Vec<Block>,
}
