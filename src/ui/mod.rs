//! TUI drawing: browser, reader, theme picker, help overlay.

pub mod browser;
pub mod picker;
pub mod reader;

use crate::app::{App, Mode};
use crate::render::to_ratatui_line;
use crate::style::Computed;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn draw(frame: &mut Frame, app: &mut App) {
    // Global background from the scheme's body color.
    let body = app.scheme.style_for(&["body"]);
    if let Some(bg) = body.bg {
        frame.render_widget(
            Block::default().style(Style::default().bg(app.level.to_ratatui(bg))),
            frame.area(),
        );
    }

    match app.mode {
        Mode::Browser => browser::draw(frame, app),
        Mode::Reader => reader::draw(frame, app),
    }

    if app.picker.is_some() {
        picker::draw(frame, app);
    }
    if app.show_help {
        draw_help(frame, app);
    }
}

/// Convert rendered lines to ratatui lines.
pub fn convert(app: &App, rendered: &crate::render::Rendered) -> Vec<Line<'static>> {
    rendered
        .lines
        .iter()
        .map(|l| to_ratatui_line(l, app.level))
        .collect()
}

pub fn chrome_style(app: &App) -> Style {
    let body = app.scheme.style_for(&["body"]);
    let mut style = Style::default();
    if let Some(fg) = body.fg {
        style = style.fg(app.level.to_ratatui(fg));
    }
    style
}

pub fn accent_style(app: &App) -> Style {
    let a = app.scheme.element("a");
    let mut style = Style::default().add_modifier(Modifier::BOLD);
    if let Some(fg) = a.fg {
        style = style.fg(app.level.to_ratatui(fg));
    }
    style
}

pub fn dim_style(app: &App) -> Style {
    let d: Computed = app.scheme.element("footnote");
    let mut style = Style::default();
    if let Some(fg) = d.fg {
        style = style.fg(app.level.to_ratatui(fg));
    }
    style
}

/// Status line rendered at the bottom of a view.
pub fn status_bar(frame: &mut Frame, app: &App, area: Rect, left: &str, right: &str) {
    let style = dim_style(app);
    let text = if let Some(status) = &app.status {
        format!(" {left} · {status}")
    } else {
        format!(" {left}")
    };
    let line = Line::from(vec![
        Span::styled(text, style),
        Span::styled(" ".repeat(area.width as usize), style),
    ]);
    let bar = Paragraph::new(vec![line, Line::from(Span::styled(format!(" {right}"), style))]);
    frame.render_widget(bar, area);
}

pub fn centered_rect(percent_x: u16, lines: u16, area: Rect) -> Rect {
    let height = lines.min(area.height);
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}

fn draw_help(frame: &mut Frame, app: &App) {
    let keys = [
        ("j/k, ↓/↑", "move / scroll"),
        ("Enter, l", "open file (browser)"),
        ("Esc", "back to browser"),
        ("d/u, PgDn/PgUp", "half page down/up"),
        ("Ctrl+f/b", "page forward / back"),
        ("g/G", "top / bottom"),
        ("/, n/N", "search / next match"),
        ("t", "theme picker"),
        ("r", "rescan files (browser)"),
        ("q", "quit"),
        ("?", "toggle this help"),
    ];
    let dim = dim_style(app);
    let accent = chrome_style(app);
    let lines: Vec<Line> = keys
        .iter()
        .map(|(k, desc)| {
            Line::from(vec![
                Span::styled(format!(" {k:<16}"), accent),
                Span::styled(desc.to_string(), dim),
            ])
        })
        .collect();
    let area = centered_rect(50, lines.len() as u16 + 2, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" keys ")
        .border_style(accent_style(app));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
