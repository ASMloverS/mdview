//! Theme picker popup with live preview.

use super::{accent_style, centered_rect, chrome_style, dim_style};
use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let sel = app.picker.unwrap_or(0);
    let height = (app.schemes.len() as u16 + 2).min(frame.area().height);
    let area = centered_rect(40, height, frame.area());
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = app
        .schemes
        .iter()
        .map(|name| {
            let style = if name == &app.scheme.name {
                accent_style(app)
            } else {
                dim_style(app)
            };
            ListItem::new(format!(" {name}")).style(style)
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(sel));
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" theme ")
                .border_style(chrome_style(app)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut state);
}
