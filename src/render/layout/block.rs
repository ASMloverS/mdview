use super::{MAX_CELL_WIDTH, Renderer, text_width, truncate};
use crate::markdown::{Align, Block, Inline};
use crate::math;
use crate::render::{SLine, SSpan};
use crate::style::Computed;

impl<'a> Renderer<'a> {
    pub(super) fn code_block(&mut self, lang: &Option<String>, code: &str) {
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

    pub(super) fn blockquote(&mut self, inner: &[Block], chain: &[&'static str]) {
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

    pub(super) fn table(
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

    pub(super) fn rule(&mut self) {
        let style = self.scheme.element("hr");
        let c = Computed {
            fg: style.border.or(style.fg),
            ..Computed::default()
        };
        self.blank();
        self.emit_full(SSpan::new("─".repeat(self.width), c));
        self.blank();
    }

    pub(super) fn math_block(&mut self, src: &str) {
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

    pub(super) fn footnote_def(&mut self, label: &str, blocks: &[Block], chain: &[&'static str]) {
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
