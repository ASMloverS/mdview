//! Two-pane browser: file list + live preview.

use super::{accent_style, chrome_style, convert, dim_style, status_bar};
use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(chunks[0]);

    // Left: file list.
    let items: Vec<ListItem> = app
        .files
        .iter()
        .map(|p| ListItem::new(p.display().to_string()))
        .collect();
    let mut state = ListState::default();
    if !app.files.is_empty() {
        state.select(Some(app.selected));
    }
    let title = format!(" markdown files ({}) ", app.files.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(chrome_style(app)),
        )
        .highlight_style(accent_style(app))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, panes[0], &mut state);

    // Right: preview.
    let preview_inner = panes[1].width.saturating_sub(2);
    let lines: Vec<Line> = if let Some(path) = app.files.get(app.selected).cloned() {
        let valid = app.preview.as_ref().is_some_and(|(p, w, theme, _)| {
            *p == path && *w == preview_inner && *theme == app.scheme.name
        });
        if !valid {
            let rendered = app.render_file(&path, preview_inner, 0);
            app.preview = Some((path.clone(), preview_inner, app.scheme.name.clone(), rendered));
        }
        app.preview
            .as_ref()
            .map(|(_, _, _, r)| convert(app, r))
            .unwrap_or_default()
    } else {
        vec![Line::from(Span::styled(
            "no markdown files found under the current directory",
            dim_style(app),
        ))]
    };
    let title = app
        .files
        .get(app.selected)
        .map(|p| format!(" {} ", p.display()))
        .unwrap_or_else(|| " preview ".to_string());
    let preview = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(chrome_style(app)),
    );
    frame.render_widget(preview, panes[1]);

    status_bar(
        frame,
        app,
        chunks[1],
        "Enter open · t theme · r rescan · ? help",
        "q quit",
    );
}
