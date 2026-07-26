//! pulldown-cmark event stream -> Document IR.

use super::ir::{Align, Block, Document, Inline, ListItem};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn parse_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_GFM
}

pub fn parse_document(src: &str) -> Document {
    let parser = Parser::new_ext(src, parse_options());
    let mut builder = Builder::default();
    builder.run(parser);
    builder.finish()
}

/// Recursive stack-based builder over the event stream.
#[derive(Default)]
struct Builder {
    doc: Document,
    block_stack: Vec<Vec<Block>>,
    inline_stack: Vec<Vec<Inline>>,
    // List state
    list_stack: Vec<(bool, u64, Vec<ListItem>)>,
    item_checked: Option<Option<bool>>,
    // Table state
    table: Option<TableState>,
    // Code block state
    code: Option<(Option<String>, String)>,
    // Metadata state
    in_meta: bool,
    meta_text: String,
    // Footnote state
    footnote: Option<String>,
    // Heading level while inside a heading
    heading: Option<u8>,
    // Link/image url stack (paired with inline_stack pushes)
    link_urls: Vec<String>,
}

struct TableState {
    aligns: Vec<Align>,
    head: Vec<Vec<Inline>>,
    rows: Vec<Vec<Vec<Inline>>>,
    cur_row: Vec<Vec<Inline>>,
    cur_cell: Vec<Inline>,
    in_head: bool,
    in_cell: bool,
}

impl Builder {
    fn blocks(&mut self) -> &mut Vec<Block> {
        self.block_stack.last_mut().expect("block stack")
    }

    fn inlines(&mut self) -> &mut Vec<Inline> {
        self.inline_stack.last_mut().expect("inline stack")
    }

    fn push_inline(&mut self, inl: Inline) {
        if let Some(t) = self.table.as_mut().filter(|t| t.in_cell) {
            t.cur_cell.push(inl);
        } else {
            self.inlines().push(inl);
        }
    }

    fn run(&mut self, parser: Parser) {
        self.block_stack.push(Vec::new());
        self.inline_stack.push(Vec::new());
        for event in parser {
            match event {
                Event::Start(tag) => self.start(tag),
                Event::End(tag) => self.end(tag),
                Event::Text(t) => {
                    if self.in_meta {
                        self.meta_text.push_str(&t);
                    } else if let Some((_, buf)) = self.code.as_mut() {
                        buf.push_str(&t);
                    } else {
                        self.push_inline(Inline::Text(t.into_string()));
                    }
                }
                Event::Code(c) => self.push_inline(Inline::Code(c.into_string())),
                Event::InlineMath(m) => self.push_inline(Inline::Math(m.into_string())),
                Event::DisplayMath(m) => {
                    self.blocks().push(Block::MathBlock(m.into_string()));
                }
                Event::FootnoteReference(name) => {
                    self.push_inline(Inline::FootnoteRef(name.into_string()));
                }
                Event::SoftBreak => self.push_inline(Inline::SoftBreak),
                Event::HardBreak => self.push_inline(Inline::HardBreak),
                Event::Rule => self.blocks().push(Block::Rule),
                Event::TaskListMarker(checked) => self.item_checked = Some(Some(checked)),
                Event::Html(_) | Event::InlineHtml(_) => {}
            }
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {}
            Tag::Item => self.block_stack.push(Vec::new()),
            Tag::Heading { level, .. } => {
                self.heading = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
                self.inline_stack.push(Vec::new());
            }
            Tag::BlockQuote(_) => self.block_stack.push(Vec::new()),
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(l) if !l.is_empty() => {
                        Some(l.into_string())
                    }
                    _ => None,
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(start) => {
                self.list_stack
                    .push((start.is_some(), start.unwrap_or(1), Vec::new()));
            }
            Tag::FootnoteDefinition(label) => {
                self.footnote = Some(label.into_string());
                self.block_stack.push(Vec::new());
            }
            Tag::Table(aligns) => {
                self.table = Some(TableState {
                    aligns: aligns
                        .iter()
                        .map(|a| match a {
                            pulldown_cmark::Alignment::None => Align::None,
                            pulldown_cmark::Alignment::Left => Align::Left,
                            pulldown_cmark::Alignment::Center => Align::Center,
                            pulldown_cmark::Alignment::Right => Align::Right,
                        })
                        .collect(),
                    head: Vec::new(),
                    rows: Vec::new(),
                    cur_row: Vec::new(),
                    cur_cell: Vec::new(),
                    in_head: false,
                    in_cell: false,
                });
            }
            Tag::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.in_head = true;
                }
            }
            Tag::TableRow => {}
            Tag::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    t.in_cell = true;
                    t.cur_cell = Vec::new();
                }
            }
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough => {
                self.inline_stack.push(Vec::new());
            }
            Tag::Link { dest_url, .. } => {
                self.inline_stack.push(Vec::new());
                // stash url on the nested vec via a marker? simpler: keep stack of urls
                self.link_urls.push(dest_url.into_string());
            }
            Tag::Image { dest_url, .. } => {
                self.inline_stack.push(Vec::new());
                self.link_urls.push(format!("img:{}", dest_url));
            }
            Tag::MetadataBlock(_) => {
                self.in_meta = true;
            }
            Tag::HtmlBlock
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition => {}
        }
    }

    /// Flush inline content left by tight containers (no Paragraph wrapper).
    fn flush_pending_inlines(&mut self) {
        let content = std::mem::take(self.inlines());
        if !content.is_empty() {
            self.blocks().push(Block::Paragraph(content));
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_pending_inlines();
            }
            TagEnd::Heading(_) => {
                let content = self.inline_stack.pop().unwrap_or_default();
                let level = self.heading.take().unwrap_or(1);
                // pop the paragraph-level vec stays; heading replaced the top
                self.blocks().push(Block::Heading { level, content });
            }
            TagEnd::BlockQuote(_) => {
                self.flush_pending_inlines();
                let inner = self.block_stack.pop().unwrap_or_default();
                self.blocks().push(Block::BlockQuote(inner));
            }
            TagEnd::CodeBlock => {
                if let Some((lang, code)) = self.code.take() {
                    self.blocks().push(Block::CodeBlock { lang, code });
                }
            }
            TagEnd::List(_) => {
                let (ordered, start, items) = self.list_stack.pop().unwrap_or((false, 1, vec![]));
                self.blocks().push(Block::List { ordered, start, items });
            }
            TagEnd::Item => {
                self.flush_pending_inlines();
                let blocks = self.block_stack.pop().unwrap_or_default();
                let checked = self.item_checked.take().flatten();
                if let Some((_, _, items)) = self.list_stack.last_mut() {
                    items.push(ListItem { checked, blocks });
                }
            }
            TagEnd::FootnoteDefinition => {
                self.flush_pending_inlines();
                let inner = self.block_stack.pop().unwrap_or_default();
                let label = self.footnote.take().unwrap_or_default();
                self.blocks().push(Block::FootnoteDef { label, blocks: inner });
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.blocks().push(Block::Table {
                        head: t.head,
                        aligns: t.aligns,
                        rows: t.rows,
                    });
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.in_head = false;
                    t.head = std::mem::take(&mut t.cur_row);
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = self.table.as_mut() {
                    if !t.in_head {
                        let row = std::mem::take(&mut t.cur_row);
                        t.rows.push(row);
                    }
                }
            }
            TagEnd::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    t.in_cell = false;
                    let cell = std::mem::take(&mut t.cur_cell);
                    t.cur_row.push(cell);
                }
            }
            TagEnd::Emphasis => {
                let inner = self.inline_stack.pop().unwrap_or_default();
                self.push_inline(Inline::Em(inner));
            }
            TagEnd::Strong => {
                let inner = self.inline_stack.pop().unwrap_or_default();
                self.push_inline(Inline::Strong(inner));
            }
            TagEnd::Strikethrough => {
                let inner = self.inline_stack.pop().unwrap_or_default();
                self.push_inline(Inline::Del(inner));
            }
            TagEnd::Link => {
                let inner = self.inline_stack.pop().unwrap_or_default();
                let url = self.link_urls.pop().unwrap_or_default();
                self.push_inline(Inline::Link { url, content: inner });
            }
            TagEnd::Image => {
                let inner = self.inline_stack.pop().unwrap_or_default();
                let url = self
                    .link_urls
                    .pop()
                    .unwrap_or_default()
                    .trim_start_matches("img:")
                    .to_string();
                let alt = plain_text(&inner);
                self.push_inline(Inline::Image { url, alt });
            }
            TagEnd::MetadataBlock(_) => {
                self.in_meta = false;
                self.doc.meta = Some(std::mem::take(&mut self.meta_text));
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Document {
        self.doc.blocks = self.block_stack.pop().unwrap_or_default();
        self.doc
    }
}

/// Extract plain text from inline content (for image alt text).
pub fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inl in inlines {
        match inl {
            Inline::Text(t) | Inline::Code(t) => out.push_str(t),
            Inline::Strong(c) | Inline::Em(c) | Inline::Del(c) => {
                out.push_str(&plain_text(c))
            }
            Inline::Link { content, .. } => out.push_str(&plain_text(content)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::Math(m) => out.push_str(m),
            Inline::FootnoteRef(l) => {
                out.push('[');
                out.push_str(l);
                out.push(']');
            }
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
        }
    }
    out
}
