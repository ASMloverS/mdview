//! Two-pane browser: entry list + live preview.

use super::{accent_style, chrome_style, convert, dim_style, status_bar};
use crate::app::App;
use crate::browse::{dir_stats, Entry, Loc};
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

    // Left: entry list. 目录加 ▸ 前缀与 / 后缀；驱动器根无文件名，只加前缀。
    let items: Vec<ListItem> = app
        .browser
        .entries
        .iter()
        .map(|e| match e {
            Entry::Dir(p) if p.file_name().is_some() => {
                ListItem::new(format!("▸ {}/", e.name()))
            }
            Entry::Dir(_) => ListItem::new(format!("▸ {}", e.name())),
            Entry::File(_) => ListItem::new(e.name()),
        })
        .collect();
    let mut state = ListState::default();
    if !app.browser.entries.is_empty() {
        state.select(Some(app.browser.selected));
    }
    let path_text = match &app.browser.loc {
        Loc::Dir(p) => truncate_left(
            &p.display().to_string(),
            panes[0].width.saturating_sub(4) as usize,
        ),
        Loc::Drives => "drives".to_string(),
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {path_text} "))
                .border_style(chrome_style(app)),
        )
        .highlight_style(accent_style(app))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, panes[0], &mut state);

    // Right: preview. 文件 → 渲染预览；目录 → 统计；空目录/驱动器层 → 提示。
    let preview_inner = panes[1].width.saturating_sub(2);
    let selected = app.browser.entries.get(app.browser.selected).cloned();
    let lines: Vec<Line> = match &selected {
        Some(Entry::File(path)) => {
            let valid = app.preview.as_ref().is_some_and(|(p, w, theme, _)| {
                p == path && *w == preview_inner && *theme == app.scheme.name
            });
            if !valid {
                let rendered = app.render_file(path, preview_inner, 0);
                app.preview =
                    Some((path.clone(), preview_inner, app.scheme.name.clone(), rendered));
            }
            app.preview
                .as_ref()
                .map(|(_, _, _, r)| convert(app, r))
                .unwrap_or_default()
        }
        Some(Entry::Dir(path)) => match dir_stats(path) {
            Some((d, f)) => vec![
                Line::from(Span::styled(format!("{d} subdirectories"), dim_style(app))),
                Line::from(Span::styled(format!("{f} markdown files"), dim_style(app))),
            ],
            None => vec![Line::from(Span::styled(
                "cannot read this directory",
                dim_style(app),
            ))],
        },
        None if matches!(app.browser.loc, Loc::Drives) => vec![Line::from(Span::styled(
            "select a drive and press o",
            dim_style(app),
        ))],
        None => vec![Line::from(Span::styled(
            "no markdown files or subdirectories here",
            dim_style(app),
        ))],
    };
    let title = selected
        .as_ref()
        .map(|e| format!(" {} ", e.path().display()))
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
        "o open · ←/Bksp up · r refresh · t theme · ? help",
        "q quit",
    );
}

/// 路径超宽时左侧截断，保留尾部并加 … 前缀。
fn truncate_left(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let tail: String = s.chars().rev().take(keep).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}
