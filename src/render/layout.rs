//! Layout engine: Document IR + Scheme -> wrapped styled lines.

use super::{plain_of, Rendered, SLine, SSpan};
use crate::highlight::Highlighter;
use crate::markdown::{Align, Block, Document, Inline};
use crate::math;
use crate::style::{Computed, Scheme};
use unicode_width::UnicodeWidthChar;

const MAX_CELL_WIDTH: usize = 32;

pub struct Renderer<'a> {
    scheme: &'a Scheme,
    highlighter: Highlighter,
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

pub fn render_document(doc: &Document, scheme: &Scheme, width: usize) -> Rendered {
    let mut r = Renderer {
        scheme,
        highlighter: Highlighter::new(scheme),
        width: width.max(20),
        out: Vec::new(),
        cur: Vec::new(),
        col: 0,
        links: Vec::new(),
    };
    r.render(doc)
}

impl<'a> Renderer<'a> {
    fn render(&mut self, doc: &Document) -> Rendered {
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

        let plain = self.out.iter().map(plain_of).collect();
        Rendered {
            lines: std::mem::take(&mut self.out),
            links: std::mem::take(&mut self.links),
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
            Block::Paragraph(content) => {
                let mut chain = chain.to_vec();
                chain.push("p");
                let segs = self.flatten(content, &chain);
                self.emit_wrapped(segs);
                self.blank();
            }
            Block::Heading { level, content } => {
                let tag: &'static str = match level {
                    1 => "h1",
                    2 => "h2",
                    3 => "h3",
                    4 => "h4",
                    5 => "h5",
                    _ => "h6",
                };
                self.blank();
                let mut chain = chain.to_vec();
                chain.push(tag);
                let segs = self.flatten(content, &chain);
                self.emit_wrapped(segs);
                self.blank();
            }
            Block::CodeBlock { lang, code } => self.code_block(lang, code),
            Block::BlockQuote(inner) => {
                let mut chain = chain.to_vec();
                chain.push("blockquote");
                let lines = self.sub_render(inner, self.width.saturating_sub(2), &chain);
                let qstyle = self.scheme.style_for(&chain);
                let border = Computed {
                    fg: qstyle.border.or(qstyle.fg),
                    ..Computed::default()
                };
                self.blank();
                for line in lines {
                    self.flush_line();
                    let mut l: SLine = vec![SSpan::new("▌ ".to_string(), border)];
                    l.extend(line);
                    self.out.push(l);
                }
                self.blank();
            }
            Block::List {
                ordered,
                start,
                items,
            } => {
                self.list(*ordered, *start, items, chain);
                self.blank();
            }
            Block::Table { head, aligns, rows } => self.table(head, aligns, rows, chain),
            Block::Rule => {
                let style = self.scheme.element("hr");
                let c = Computed {
                    fg: style.border.or(style.fg),
                    ..Computed::default()
                };
                self.blank();
                self.emit_full(SSpan::new("─".repeat(self.width), c));
                self.blank();
            }
            Block::MathBlock(src) => self.math_block(src),
            Block::FootnoteDef { label, blocks } => {
                let style = self.scheme.element("footnote");
                let lines = self.sub_render(blocks, self.width.saturating_sub(6), chain);
                self.blank();
                let marker = format!("[^{label}] ");
                let pad = " ".repeat(text_width(&marker));
                for (i, line) in lines.into_iter().enumerate() {
                    self.flush_line();
                    let mut l: SLine = vec![SSpan::new(
                        if i == 0 { marker.clone() } else { pad.clone() },
                        style,
                    )];
                    l.extend(line);
                    self.out.push(l);
                }
            }
        }
    }

    fn code_block(&mut self, lang: &Option<String>, code: &str) {
        let pre = self.scheme.style_for(&["body", "pre"]);
        let highlighted = self.highlighter.highlight(code, lang.as_deref());
        self.blank();
        for runs in highlighted {
            self.flush_line();
            let mut line: SLine = Vec::new();
            let mut col = 0;
            for (color, text) in runs {
                col += text_width(&text);
                let style = Computed {
                    fg: Some(color),
                    bg: pre.bg,
                    ..Computed::default()
                };
                line.push(SSpan::new(text, style));
            }
            if let Some(bg) = pre.bg {
                if col < self.width {
                    line.push(SSpan::new(
                        " ".repeat(self.width - col),
                        Computed {
                            bg: Some(bg),
                            ..Computed::default()
                        },
                    ));
                }
            }
            self.out.push(line);
        }
        self.blank();
    }

    #[allow(clippy::too_many_arguments)]
    fn list(
        &mut self,
        ordered: bool,
        start: u64,
        items: &[crate::markdown::ListItem],
        chain: &[&'static str],
    ) {
        self.blank();
        let marker_style = self.scheme.element("li");
        let mut chain = chain.to_vec();
        chain.push("li");
        for (i, item) in items.iter().enumerate() {
            let marker = if let Some(checked) = item.checked {
                if checked {
                    "☑ ".to_string()
                } else {
                    "☐ ".to_string()
                }
            } else if ordered {
                format!("{}. ", start + i as u64)
            } else {
                "• ".to_string()
            };
            let indent = text_width(&marker);
            let lines = self.sub_render(&item.blocks, self.width.saturating_sub(indent), &chain);
            let pad = " ".repeat(indent);
            if lines.is_empty() {
                self.flush_line();
                self.out.push(vec![SSpan::new(marker, marker_style)]);
                continue;
            }
            for (j, line) in lines.into_iter().enumerate() {
                self.flush_line();
                let mut l: SLine = if j == 0 {
                    vec![SSpan::new(marker.clone(), marker_style)]
                } else {
                    vec![SSpan::new(pad.clone(), Computed::default())]
                };
                l.extend(line);
                self.out.push(l);
            }
        }
    }

    fn table(
        &mut self,
        head: &[Vec<Inline>],
        aligns: &[Align],
        rows: &[Vec<Vec<Inline>>],
        chain: &[&'static str],
    ) {
        let mut chain = chain.to_vec();
        chain.push("table");
        let tstyle = self.scheme.style_for(&chain);
        let border = Computed {
            fg: tstyle.border.or(tstyle.fg),
            ..Computed::default()
        };
        let th_style = self.scheme.style_for(&{
            let mut c = chain.clone();
            c.push("th");
            c
        });
        let td_style = self.scheme.style_for(&{
            let mut c = chain.clone();
            c.push("td");
            c
        });

        let cols = head.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
        if cols == 0 {
            return;
        }
        let cell_text = |cell: Option<&Vec<Inline>>| -> String {
            cell.map(|c| crate::markdown::plain_text(c))
                .unwrap_or_default()
        };

        // Column widths from content, capped.
        let mut widths = vec![0usize; cols];
        for (i, cell) in head.iter().enumerate() {
            widths[i] = widths[i].max(text_width(&cell_text(Some(cell))));
        }
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(text_width(&cell_text(Some(cell))));
            }
        }
        for w in &mut widths {
            *w = (*w).clamp(1, MAX_CELL_WIDTH);
        }
        // Shrink to fit the available width.
        let total = |ws: &[usize]| ws.iter().sum::<usize>() + 3 * ws.len() + 1;
        while total(&widths) > self.width {
            if let Some(max) = widths.iter_mut().max() {
                if *max <= 1 {
                    break;
                }
                *max -= 1;
            } else {
                break;
            }
        }

        let hline = |left: &str, mid: &str, right: &str, widths: &[usize]| -> SLine {
            let mut s = String::new();
            s.push_str(left);
            for (i, w) in widths.iter().enumerate() {
                s.push_str(&"─".repeat(w + 2));
                if i + 1 < widths.len() {
                    s.push_str(mid);
                }
            }
            s.push_str(right);
            vec![SSpan::new(s, border)]
        };

        let row_line = |cells: &[Vec<Inline>], style: Computed| -> SLine {
            let mut line: SLine = vec![SSpan::new("│".to_string(), border)];
            for (i, w) in widths.iter().enumerate() {
                let align = aligns.get(i).copied().unwrap_or(Align::None);
                let text = truncate(&cell_text(cells.get(i)), *w);
                let tw = text_width(&text);
                let padded = match align {
                    Align::Right => format!(" {}{} ", " ".repeat(w - tw), text),
                    Align::Center => {
                        let l = (w - tw) / 2;
                        format!(" {}{}{} ", " ".repeat(l), text, " ".repeat(w - tw - l))
                    }
                    _ => format!(" {}{} ", text, " ".repeat(w - tw)),
                };
                line.push(SSpan::new(padded, style));
                line.push(SSpan::new("│".to_string(), border));
            }
            line
        };

        self.blank();
        self.push_raw_line(hline("┌", "┬", "┐", &widths));
        self.push_raw_line(row_line(head, th_style));
        self.push_raw_line(hline("├", "┼", "┤", &widths));
        for row in rows {
            self.push_raw_line(row_line(row, td_style));
        }
        self.push_raw_line(hline("└", "┴", "┘", &widths));
        self.blank();
    }

    fn math_block(&mut self, src: &str) {
        let style = self.scheme.element("math");
        self.blank();
        match math::to_unicode(src) {
            Some(line) => {
                let w = text_width(&line);
                let pad = self.width.saturating_sub(w) / 2;
                let mut l: SLine = vec![SSpan::new(" ".repeat(pad), Computed::default())];
                l.push(SSpan::new(line, style));
                self.push_raw_line(l);
            }
            None => {
                let pre = self.scheme.style_for(&["body", "pre"]);
                for line in src.lines() {
                    self.emit_full(SSpan::new(line.to_string(), pre));
                }
            }
        }
        self.blank();
    }

    // ----- inlines -----

    /// Register a link target, returning its 1-based display index.
    fn register_link(&mut self, url: &str) -> usize {
        if let Some(pos) = self.links.iter().position(|u| u == url) {
            pos + 1
        } else {
            self.links.push(url.to_string());
            self.links.len()
        }
    }

    fn flatten(&mut self, inlines: &[Inline], chain: &[&'static str]) -> Vec<Seg> {
        let mut out = Vec::new();
        self.flatten_into(inlines, chain, &mut out);
        out
    }

    fn flatten_into(&mut self, inlines: &[Inline], chain: &[&'static str], out: &mut Vec<Seg>) {
        for inl in inlines {
            match inl {
                Inline::Text(t) => out.push(Seg {
                    text: t.clone(),
                    style: self.scheme.style_for(chain),
                    link: None,
                }),
                Inline::Code(t) => {
                    let style = self.scheme.style_for(&{
                        let mut c = chain.to_vec();
                        c.push("code");
                        c
                    });
                    out.push(Seg {
                        text: t.clone(),
                        style,
                        link: None,
                    });
                }
                Inline::Strong(c) => self.flatten_styled(c, chain, "strong", out),
                Inline::Em(c) => self.flatten_styled(c, chain, "em", out),
                Inline::Del(c) => self.flatten_styled(c, chain, "del", out),
                Inline::Link { url, content } => {
                    let mut c = chain.to_vec();
                    c.push("a");
                    let a_style = self.scheme.style_for(&c);
                    let marker_style = self.scheme.element("footnote");
                    let idx = self.register_link(url);
                    let start = out.len();
                    self.flatten_into(content, &c, out);
                    // Mark every seg of this link with the URL and style.
                    let text: String = out[start..].iter().map(|s| s.text.clone()).collect();
                    for seg in &mut out[start..] {
                        seg.link = Some(url.clone());
                        seg.style = a_style;
                    }
                    // Numbered marker unless the text already shows the URL.
                    if text.trim() != url.as_str() {
                        out.push(Seg {
                            text: format!("[{idx}]"),
                            style: marker_style,
                            link: Some(url.clone()),
                        });
                    }
                }
                Inline::Image { url, alt } => {
                    let style = self.scheme.style_for(&{
                        let mut c = chain.to_vec();
                        c.push("img");
                        c
                    });
                    self.register_link(url);
                    let text = if alt.is_empty() {
                        format!("🖼 {url}")
                    } else {
                        format!("🖼 {alt}")
                    };
                    out.push(Seg {
                        text,
                        style,
                        link: Some(url.clone()),
                    });
                }
                Inline::Math(src) => {
                    let style = self.scheme.style_for(&{
                        let mut c = chain.to_vec();
                        c.push("math");
                        c
                    });
                    let text = math::to_unicode(src).unwrap_or_else(|| src.clone());
                    out.push(Seg {
                        text,
                        style,
                        link: None,
                    });
                }
                Inline::FootnoteRef(label) => {
                    let style = self.scheme.style_for(&{
                        let mut c = chain.to_vec();
                        c.push("footnote");
                        c
                    });
                    out.push(Seg {
                        text: format!("[^{label}]"),
                        style,
                        link: None,
                    });
                }
                Inline::SoftBreak => out.push(Seg {
                    text: " ".to_string(),
                    style: Computed::default(),
                    link: None,
                }),
                Inline::HardBreak => out.push(Seg {
                    text: "\n".to_string(),
                    style: Computed::default(),
                    link: None,
                }),
            }
        }
    }

    fn flatten_styled(
        &mut self,
        content: &[Inline],
        chain: &[&'static str],
        tag: &'static str,
        out: &mut Vec<Seg>,
    ) {
        let mut c = chain.to_vec();
        c.push(tag);
        self.flatten_into(content, &c, out);
    }

    // ----- wrapping -----

    fn emit_wrapped(&mut self, segs: Vec<Seg>) {
        for seg in segs {
            for token in tokenize(&seg.text) {
                match token {
                    Token::Newline => self.flush_line(),
                    Token::Space(s) => {
                        let w = text_width(&s);
                        if self.col + w > self.width {
                            self.flush_line();
                        } else if self.col > 0 {
                            self.emit(SSpan {
                                text: s,
                                style: seg.style,
                                link: seg.link.clone(),
                            });
                        }
                    }
                    Token::Word(wd) => {
                        let w = text_width(&wd);
                        if self.col + w > self.width && self.col > 0 {
                            self.flush_line();
                        }
                        if text_width(&wd) <= self.width {
                            self.emit(SSpan {
                                text: wd,
                                style: seg.style,
                                link: seg.link.clone(),
                            });
                        } else {
                            // Hard-break an overlong word.
                            let mut buf = String::new();
                            let mut bw = 0;
                            for ch in wd.chars() {
                                let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                                if bw + cw > self.width && !buf.is_empty() {
                                    self.emit(SSpan {
                                        text: std::mem::take(&mut buf),
                                        style: seg.style,
                                        link: seg.link.clone(),
                                    });
                                    self.flush_line();
                                    bw = 0;
                                }
                                bw += cw;
                                buf.push(ch);
                            }
                            if !buf.is_empty() {
                                self.emit(SSpan {
                                    text: buf,
                                    style: seg.style,
                                    link: seg.link.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        self.flush_line();
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
        let doc = parse_document(src);
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        render_document(&doc, &scheme, width).plain
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
    fn renders_task_list() {
        let lines = render("- [x] done\n- [ ] todo", 40);
        assert!(lines.iter().any(|l| l.contains("☑ done")));
        assert!(lines.iter().any(|l| l.contains("☐ todo")));
    }

    #[test]
    fn link_list_at_bottom() {
        let lines = render("[site](https://example.com)", 60);
        assert!(lines.iter().any(|l| l.contains("[1] https://example.com")));
    }
}
