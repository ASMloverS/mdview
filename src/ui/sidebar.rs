//! Sidebar: directory list panel (focusable).

use super::{accent_style, chrome_style, status_bar};
use crate::app::App;
use crate::browse::{Entry, Loc};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(area);

    let Some(sidebar) = &app.sidebar else { return };
    // 目录加 ▸ 前缀与 / 后缀；驱动器根无文件名，只加前缀。
    let items: Vec<ListItem> = sidebar
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
    if !sidebar.browser.entries.is_empty() {
        state.select(Some(sidebar.browser.selected));
    }
    let path_text = match &sidebar.browser.loc {
        Loc::Dir(p) => truncate_left(
            &p.display().to_string(),
            chunks[0].width.saturating_sub(4) as usize,
        ),
        Loc::Drives => "drives".to_string(),
    };
    let border = if focused { accent_style(app) } else { chrome_style(app) };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {path_text} "))
                .border_style(border),
        )
        .highlight_style(accent_style(app))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, chunks[0], &mut state);

    status_bar(
        frame,
        app,
        chunks[1],
        "Enter open · Bksp up · r refresh · Tab focus · ? help",
        "Esc close",
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
