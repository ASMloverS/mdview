//! Full-screen reader view with scroll and search.

use super::{accent_style, chrome_style, convert, dim_style};
use crate::app::{content_width, App};
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
    let want_offset = view.width.saturating_sub(2).saturating_sub(want_width) / 2;
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
    }

    let reader = app.reader.as_ref().unwrap();
    let lines = convert(app, &reader.rendered);
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
