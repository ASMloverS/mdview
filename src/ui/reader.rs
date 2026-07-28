//! Full-screen reader view with scroll and search.

use super::{accent_style, chrome_style, convert, cursor_style, dim_style};
use crate::app::{content_offset, content_width, App};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);
    let view = chunks[0];

    let want_width = content_width(view.width, app.max_width);
    let want_offset = content_offset(view.width.saturating_sub(2), want_width, app.align);
    let Some(reader) = app.reader.as_mut() else { return };
    reader.view_height = view.height.saturating_sub(2) as usize;
    if reader.width != want_width || reader.offset != want_offset {
        let path = reader.path.clone();
        let scroll = reader.scroll;
        let rendered = app.render_file(&path, want_width, want_offset);
        let reader = app.reader.as_mut().unwrap();
        reader.rendered = rendered;
        reader.width = want_width;
        reader.offset = want_offset;
        reader.scroll = scroll.min(reader.rendered.lines.len().saturating_sub(1));
        reader.cursor = reader.cursor.min(reader.rendered.lines.len().saturating_sub(1));
    }

    let reader = app.reader.as_ref().unwrap();
    let mut lines = convert(app, &reader.rendered);
    // 光标行高亮：行 style 与各 span 都叠加光标背景（bg-only 补丁保留 fg），
    // 行尾用填充 span 补齐。
    if let Some(style) = cursor_style(app) {
        if let Some(line) = lines.get_mut(reader.cursor) {
            *line = std::mem::take(line).patch_style(style);
            for span in &mut line.spans {
                span.style = span.style.patch(style);
            }
            line.spans.push(Span::styled(" ".repeat(view.width as usize), style));
        }
    }
    let total = reader.rendered.lines.len();
    let pct = if total <= reader.view_height {
        100
    } else {
        (reader.scroll * 100) / (total - reader.view_height).max(1)
    };
    let title = format!(" {} · {}% ", reader.path.display(), pct);
    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(chrome_style(app)),
        )
        .scroll((reader.scroll as u16, 0));
    frame.render_widget(widget, view);

    // Status / search bar.
    let (left, right) = if app.searching {
        (
            format!("/{}", app.search_query),
            String::from("Enter confirm · Esc cancel"),
        )
    } else {
        let matches = if app.search_matches.is_empty() {
            String::new()
        } else {
            format!(" · {} matches", app.search_matches.len())
        };
        (
            format!("Esc back · / search · t theme · ? help{matches}"),
            String::from("q quit"),
        )
    };
    let mut spans = vec![Span::styled(format!(" {left}"), dim_style(app))];
    if let Some(status) = &app.status {
        spans.push(Span::styled(format!(" · {status}"), accent_style(app)));
    }
    spans.push(Span::styled(" ".repeat(view.width as usize), dim_style(app)));
    let bar = Paragraph::new(vec![
        Line::from(spans),
        Line::from(Span::styled(format!(" {right}"), dim_style(app))),
    ]);
    frame.render_widget(bar, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Mode, Reader};
    use crate::config::ContentAlign;
    use crate::render::{Rendered, SSpan};
    use crate::style::{ColorLevel, Computed, Rgb, Scheme};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn test_app(lines: usize, cursor: usize) -> App {
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let mut app = App::new(scheme, ColorLevel::True, 100, ContentAlign::Center);
        app.mode = Mode::Reader;
        app.reader = Some(Reader {
            path: PathBuf::from("test.md"),
            rendered: Rendered {
                lines: vec![Vec::new(); lines],
                plain: vec![String::new(); lines],
            },
            // 与 30 列终端的 want_width/want_offset 一致，避免 draw 触发重排版。
            width: 28,
            offset: 0,
            scroll: 0,
            cursor,
            view_height: 8,
        });
        app
    }

    #[test]
    fn cursor_line_highlighted_full_width() {
        let mut app = test_app(20, 2);
        let want = app.scheme.element("cursor").bg.unwrap();
        let bg = Color::Rgb(want.0, want.1, want.2);
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        // 边框内侧：cursor=2, scroll=0 → 屏幕行 y = 1 + 2 = 3。
        for x in 1..29 {
            assert_eq!(buf.cell((x, 3)).unwrap().bg, bg, "x={x}");
        }
        // 相邻行无高亮。
        assert_ne!(buf.cell((1, 2)).unwrap().bg, bg);
        assert_ne!(buf.cell((1, 4)).unwrap().bg, bg);
    }

    #[test]
    fn cursor_line_keeps_span_foreground() {
        let mut app = test_app(20, 2);
        // 光标行放一个带独立前景的 span：高亮只改 bg。
        app.reader.as_mut().unwrap().rendered.lines[2] = vec![SSpan::new(
            "x",
            Computed {
                fg: Some(Rgb(255, 0, 0)),
                ..Computed::default()
            },
        )];
        let want = app.scheme.element("cursor").bg.unwrap();
        let bg = Color::Rgb(want.0, want.1, want.2);
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let cell = buf.cell((1, 3)).unwrap();
        assert_eq!(cell.symbol(), "x");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, bg);
    }

    #[test]
    fn cursor_line_overrides_span_background() {
        let mut app = test_app(20, 2);
        // 模拟代码块行：span 自带 bg，光标背景必须覆盖它、保留 fg。
        app.reader.as_mut().unwrap().rendered.lines[2] = vec![SSpan::new(
            "x",
            Computed {
                fg: Some(Rgb(255, 0, 0)),
                bg: Some(Rgb(60, 56, 54)),
                ..Computed::default()
            },
        )];
        let want = app.scheme.element("cursor").bg.unwrap();
        let bg = Color::Rgb(want.0, want.1, want.2);
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let cell = buf.cell((1, 3)).unwrap();
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, bg);
    }

    #[test]
    fn no_cursor_element_means_no_highlight() {
        let mut app = test_app(20, 2);
        // 空规则主题：无 cursor 元素。
        app.scheme = Scheme {
            name: "empty".into(),
            rules: Vec::new(),
        };
        let backend = TestBackend::new(30, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        assert_eq!(buf.cell((5, 3)).unwrap().bg, Color::Reset);
    }
}
