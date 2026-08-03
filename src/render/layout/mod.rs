//! Layout engine: Document IR + Scheme -> wrapped styled lines.

mod block;
pub mod decorate;
mod text;

use super::{plain_of, Rendered, SLine, SSpan};
use crate::highlight::Highlighter;
use crate::markdown::{Block, Document};
use crate::style::{Computed, Scheme, SyntaxTheme};
use unicode_width::UnicodeWidthChar;

const MAX_CELL_WIDTH: usize = 32;

pub struct Renderer<'a> {
    scheme: &'a Scheme,
    highlighter: Highlighter<'a>,
    width: usize,
    out: Vec<SLine>,
    cur: SLine,
    col: usize,
    links: Vec<String>,
}

/// One flattened inline segment.
struct Seg {
    text: String,
    style: Computed,
    link: Option<String>,
}

pub fn render_document(
    doc: &Document,
    scheme: &Scheme,
    syntax_theme: &SyntaxTheme,
    width: usize,
    offset: usize,
) -> Rendered {
    let mut r = Renderer {
        scheme,
        highlighter: Highlighter::new(scheme, syntax_theme),
        width: width.max(20),
        out: Vec::new(),
        cur: Vec::new(),
        col: 0,
        links: Vec::new(),
    };
    r.render(doc, offset)
}

impl<'a> Renderer<'a> {
    fn render(&mut self, doc: &Document, offset: usize) -> Rendered {
        if let Some(meta) = &doc.meta {
            let style = self.scheme.element("footnote");
            for line in meta.lines() {
                self.emit_full(SSpan::new(line.to_string(), style));
            }
            self.blank();
        }
        for block in &doc.blocks {
            self.block(block, &["body"]);
        }
        self.flush_line();

        // Trailing link list (w3m style).
        if !self.links.is_empty() {
            self.blank();
            let link_style = self.scheme.element("a");
            let dim = self.scheme.element("footnote");
            let links = self.links.clone();
            for (i, url) in links.iter().enumerate() {
                self.emit(SSpan::new(format!("[{}] ", i + 1), dim));
                self.emit(SSpan::linked(url.clone(), link_style, url.clone()));
                self.flush_line();
            }
        }

        // Trim trailing blank lines.
        while self.out.last().is_some_and(|l| l.is_empty()) {
            self.out.pop();
        }

        // Uniform left offset for horizontal centering.
        if offset > 0 {
            let pad = || SSpan::new(" ".repeat(offset), Computed::default());
            for line in &mut self.out {
                if !line.is_empty() {
                    line.insert(0, pad());
                }
            }
        }

        let plain = self.out.iter().map(plain_of).collect();
        Rendered {
            lines: std::mem::take(&mut self.out),
            plain,
        }
    }

    // ----- line primitives -----

    fn flush_line(&mut self) {
        let line = std::mem::take(&mut self.cur);
        if !line.is_empty() {
            self.out.push(line);
        }
        self.col = 0;
    }

    fn blank(&mut self) {
        self.flush_line();
        if self.out.last().is_some_and(|l| l.is_empty()) || self.out.is_empty() {
            return;
        }
        self.out.push(Vec::new());
    }

    /// Append a span assuming it fits on the current line.
    fn emit(&mut self, span: SSpan) {
        self.col += text_width(&span.text);
        self.cur.push(span);
    }

    /// Emit a full line at once.
    fn emit_full(&mut self, span: SSpan) {
        self.emit(span);
        self.flush_line();
    }

    fn push_raw_line(&mut self, line: SLine) {
        self.flush_line();
        self.out.push(line);
    }

    // ----- blocks -----

    fn block(&mut self, block: &Block, chain: &[&'static str]) {
        match block {
            Block::Paragraph(content) => self.paragraph(content, chain),
            Block::Heading { level, content } => self.heading(*level, content, chain),
            Block::CodeBlock { lang, code } => self.code_block(lang, code),
            Block::BlockQuote(inner) => self.blockquote(inner, chain),
            Block::List {
                ordered,
                start,
                items,
            } => {
                self.list(*ordered, *start, items, chain);
                self.blank();
            }
            Block::Table { head, aligns, rows } => self.table(head, aligns, rows, chain),
            Block::Rule => self.rule(),
            Block::MathBlock(src) => self.math_block(src),
            Block::FootnoteDef { label, blocks } => self.footnote_def(label, blocks, chain),
        }
    }

    /// Render nested blocks into separate lines at a reduced width.
    fn sub_render(&mut self, blocks: &[Block], width: usize, chain: &[&'static str]) -> Vec<SLine> {
        let saved_out = std::mem::take(&mut self.out);
        let saved_cur = std::mem::take(&mut self.cur);
        let saved_col = self.col;
        let saved_width = self.width;
        self.width = width.max(10);
        self.col = 0;
        for b in blocks {
            self.block(b, chain);
        }
        self.flush_line();
        while self.out.last().is_some_and(|l| l.is_empty()) {
            self.out.pop();
        }
        let lines = std::mem::replace(&mut self.out, saved_out);
        self.cur = saved_cur;
        self.col = saved_col;
        self.width = saved_width;
        lines
    }
}

// ----- text helpers -----

enum Token {
    Word(String),
    Space(String),
    Newline,
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut buf = String::new();
    let mut in_space = None::<bool>;
    for ch in text.chars() {
        if ch == '\n' {
            if !buf.is_empty() {
                tokens.push(if in_space == Some(true) {
                    Token::Space(std::mem::take(&mut buf))
                } else {
                    Token::Word(std::mem::take(&mut buf))
                });
            }
            in_space = None;
            tokens.push(Token::Newline);
            continue;
        }
        let sp = ch == ' ' || ch == '\t';
        if in_space == Some(sp) || in_space.is_none() {
            in_space = Some(sp);
            buf.push(ch);
        } else {
            tokens.push(if in_space == Some(true) {
                Token::Space(std::mem::take(&mut buf))
            } else {
                Token::Word(std::mem::take(&mut buf))
            });
            in_space = Some(sp);
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        tokens.push(if in_space == Some(true) {
            Token::Space(buf)
        } else {
            Token::Word(buf)
        });
    }
    tokens
}

pub fn text_width(s: &str) -> usize {
    s.chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

fn truncate(s: &str, max: usize) -> String {
    if text_width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        w += cw;
        out.push(ch);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parse_document;

    fn render(src: &str, width: usize) -> Vec<String> {
        render_off(src, width, 0)
    }

    fn render_off(src: &str, width: usize, offset: usize) -> Vec<String> {
        let doc = parse_document(src);
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let syntax = SyntaxTheme::load(&scheme.name);
        render_document(&doc, &scheme, &syntax, width, offset).plain
    }

    #[test]
    fn centers_with_offset() {
        let lines = render_off("hello\n\nworld", 20, 5);
        let first = lines.iter().find(|l| !l.trim().is_empty()).unwrap();
        assert!(first.starts_with("     hello"), "offset pad: {first:?}");
        let blank = lines.iter().find(|l| l.is_empty()).unwrap();
        assert_eq!(blank, &String::new(), "blank lines stay unpadded");
    }

    #[test]
    fn wraps_paragraphs() {
        let lines = render("hello world this is a long paragraph that should wrap nicely", 20);
        assert!(lines.len() >= 3);
        assert!(lines.iter().all(|l| text_width(l) <= 20));
    }

    #[test]
    fn renders_table_borders() {
        let lines = render("| a | b |\n|---|---|\n| 1 | 2 |", 40);
        assert!(lines.iter().any(|l| l.starts_with('┌')));
        assert!(lines.iter().any(|l| l.starts_with('└')));
    }

    #[test]
    fn table_header_double_separator() {
        let lines = render("| a | b |\n|---|---|\n| 1 | 2 |", 40);
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with('╞') && l.contains('╪') && l.ends_with('╡')),
            "double separator under header: {lines:?}"
        );
    }

    #[test]
    fn renders_task_list() {
        let lines = render("- [x] done\n- [ ] todo", 40);
        assert!(lines.iter().any(|l| l.contains("☑ done")));
        assert!(lines.iter().any(|l| l.contains("☐ todo")));
    }

    #[test]
    fn heading_rules() {
        let lines = render("# Top\n\n## Mid\n\n### Low", 30);
        assert_eq!(
            lines.iter().filter(|l| *l == &"═".repeat(30)).count(),
            1,
            "h1 gets one double rule, lines: {lines:?}"
        );
        assert_eq!(
            lines.iter().filter(|l| *l == &"─".repeat(30)).count(),
            1,
            "h2 gets one single rule, lines: {lines:?}"
        );
    }

    #[test]
    fn link_list_at_bottom() {
        let lines = render("[site](https://example.com)", 60);
        assert!(lines.iter().any(|l| l.contains("[1] https://example.com")));
    }

    #[test]
    fn code_block_gutter_and_lang_tag() {
        let lines = render("```rust\nfn main() {}\nlet x = 1;\n```", 40);
        let body: Vec<&String> = lines.iter().filter(|l| l.contains('│')).collect();
        assert_eq!(body.len(), 2, "one gutter row per code line: {lines:?}");
        assert!(body[0].starts_with("1 │ "), "line numbers: {body:?}");
        assert!(body[1].starts_with("2 │ "));
        assert!(body[0].contains("rust"), "lang tag on first row: {:?}", body[0]);
        let w: usize = body[0].chars().count();
        assert_eq!(w, 40, "tag row painted to full width");
        assert!(body[0].ends_with(" rust "), "tag at right edge: {:?}", body[0]);
    }

    #[test]
    fn code_block_bg_fills_line() {
        let doc = parse_document("```\nhi\n```");
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let syntax = SyntaxTheme::load(&scheme.name);
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
        let pre_bg = scheme.style_for(&["body", "pre"]).bg;
        assert!(pre_bg.is_some());
        let line = r
            .lines
            .iter()
            .find(|l| plain_of(l).contains("hi"))
            .expect("code line");
        assert!(line.iter().all(|s| s.style.bg == pre_bg), "whole row painted");
    }

    #[test]
    fn code_block_rows_full_width() {
        let doc = parse_document("```\nfn main() {}\nlet x = 1;\n```");
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let syntax = SyntaxTheme::load(&scheme.name);
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
        let rows: Vec<_> = r
            .lines
            .iter()
            .filter(|l| plain_of(l).contains('│'))
            .collect();
        assert_eq!(rows.len(), 2);
        for row in rows {
            let w: usize = row.iter().map(|s| text_width(&s.text)).sum();
            assert_eq!(w, 40, "row painted to full width: {row:?}");
        }
    }

    #[test]
    fn heading_rule_after_wrapped_text() {
        let lines = render("# a long heading that wraps over two lines", 20);
        assert_eq!(
            lines.iter().filter(|l| *l == &"═".repeat(20)).count(),
            1,
            "exactly one rule after the wrapped heading: {lines:?}"
        );
    }

    #[test]
    fn blockquote_bar() {
        let lines = render("> hello\n>\n> world", 40);
        assert!(
            lines.iter().filter(|l| l.starts_with("▎ ")).count() >= 2,
            "quote lines start with the bar: {lines:?}"
        );
    }

    #[test]
    fn blockquote_bg_fills_rows() {
        let doc = parse_document("> hi");
        let scheme = Scheme {
            name: "t".into(),
            rules: crate::style::css::parse(
                "blockquote { background: #112233; border-color: #445566 }",
            ),
        };
        let syntax = SyntaxTheme::load(&scheme.name);
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
        let line = r
            .lines
            .iter()
            .find(|l| plain_of(l).contains("hi"))
            .expect("quote line");
        let bg = Some(crate::style::Rgb(0x11, 0x22, 0x33));
        assert!(
            line.iter().all(|s| s.style.bg == bg),
            "every span painted, incl. padding: {line:?}"
        );
        let w: usize = line.iter().map(|s| text_width(&s.text)).sum();
        assert_eq!(w, 40, "padding pinned: {line:?}");
    }

    #[test]
    fn blockquote_preserves_inner_backgrounds() {
        let doc = parse_document("> `code` text");
        let scheme = Scheme {
            name: "t".into(),
            rules: crate::style::css::parse(
                "blockquote { background: #112233 } code { background: #445566 }",
            ),
        };
        let syntax = SyntaxTheme::load(&scheme.name);
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
        let line = r
            .lines
            .iter()
            .find(|l| plain_of(l).contains("code"))
            .expect("quote line");
        let code_span = line
            .iter()
            .find(|s| s.text.contains("code"))
            .expect("code span");
        assert_eq!(code_span.style.bg, Some(crate::style::Rgb(0x44, 0x55, 0x66)));
    }

    #[test]
    fn code_block_comment_is_italic_via_page_fallback() {
        let doc = parse_document("```rust\n// hi\n```");
        let scheme = Scheme {
            name: "t".into(),
            rules: crate::style::css::parse(
                "pre { color: #111111; background: #222222 } syntax-comment { color: #333333; font-style: italic }",
            ),
        };
        let syntax = SyntaxTheme::from_css("t", "");
        let r = render_document(&doc, &scheme, &syntax, 40, 0);
        let line = r
            .lines
            .iter()
            .find(|l| plain_of(l).contains("hi"))
            .expect("code line");
        let span = line.iter().find(|s| s.text.contains("hi")).expect("comment span");
        assert!(span.style.italic, "comment italic reaches the span");
        assert_eq!(span.style.fg, Some(crate::style::Rgb(0x33, 0x33, 0x33)));
    }
}
