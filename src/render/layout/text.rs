use super::{Renderer, Seg, Token, text_width, tokenize};
use crate::markdown::{Inline, ListItem};
use crate::math;
use crate::render::{SLine, SSpan};
use crate::style::Computed;
use unicode_width::UnicodeWidthChar;

impl<'a> Renderer<'a> {
    pub(super) fn paragraph(&mut self, content: &[Inline], chain: &[&'static str]) {
        let mut chain = chain.to_vec();
        chain.push("p");
        let segs = self.flatten(content, &chain);
        self.emit_wrapped(segs);
        self.blank();
    }

    pub(super) fn heading(&mut self, level: u8, content: &[Inline], chain: &[&'static str]) {
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn list(
        &mut self,
        ordered: bool,
        start: u64,
        items: &[ListItem],
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

    // ----- inlines -----

    /// Register a link target, returning its 1-based display index.
    pub(super) fn register_link(&mut self, url: &str) -> usize {
        if let Some(pos) = self.links.iter().position(|u| u == url) {
            pos + 1
        } else {
            self.links.push(url.to_string());
            self.links.len()
        }
    }

    pub(super) fn flatten(&mut self, inlines: &[Inline], chain: &[&'static str]) -> Vec<Seg> {
        let mut out = Vec::new();
        self.flatten_into(inlines, chain, &mut out);
        out
    }

    pub(super) fn flatten_into(
        &mut self,
        inlines: &[Inline],
        chain: &[&'static str],
        out: &mut Vec<Seg>,
    ) {
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

    pub(super) fn flatten_styled(
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

    pub(super) fn emit_wrapped(&mut self, segs: Vec<Seg>) {
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
}
