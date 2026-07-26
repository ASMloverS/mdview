//! TUI application: state machine and event loop.

use crate::markdown::parse_document;
use crate::render::layout::render_document;
use crate::render::Rendered;
use crate::style::{ColorLevel, Scheme};
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub enum Mode {
    Browser,
    Reader,
}

pub struct Reader {
    pub path: PathBuf,
    pub rendered: Rendered,
    pub width: u16,
    pub offset: u16,
    pub scroll: usize,
    pub view_height: usize,
}

pub struct App {
    pub mode: Mode,
    pub files: Vec<PathBuf>,
    pub selected: usize,
    pub preview: Option<(PathBuf, u16, String, Rendered)>,
    pub reader: Option<Reader>,
    pub picker: Option<usize>,
    pub schemes: Vec<String>,
    pub searching: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub scheme: Scheme,
    pub level: ColorLevel,
    pub max_width: usize,
    pub show_help: bool,
    pub status: Option<String>,
    pub quit: bool,
}

impl App {
    pub fn new(scheme: Scheme, level: ColorLevel, max_width: usize) -> App {
        App {
            mode: Mode::Browser,
            files: scan_files(Path::new(".")),
            selected: 0,
            preview: None,
            reader: None,
            picker: None,
            schemes: Scheme::available(),
            searching: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            scheme,
            level,
            max_width,
            show_help: false,
            status: None,
            quit: false,
        }
    }

    pub fn render_file(&self, path: &Path, width: u16, offset: u16) -> Rendered {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| format!("(error: {e})"));
        let doc = parse_document(&text);
        render_document(&doc, &self.scheme, width as usize, offset as usize)
    }

    pub fn open_reader(&mut self, path: PathBuf, width: u16, offset: u16) {
        let rendered = self.render_file(&path, width, offset);
        self.reader = Some(Reader {
            path,
            rendered,
            width,
            offset,
            scroll: 0,
            view_height: 24,
        });
        self.search_matches.clear();
        self.mode = Mode::Reader;
    }

    pub fn reload_reader(&mut self) {
        let Some(reader) = &self.reader else {
            return;
        };
        let scroll = reader.scroll;
        let path = reader.path.clone();
        let width = reader.width;
        let offset = reader.offset;
        let rendered = self.render_file(&path, width, offset);
        if let Some(reader) = self.reader.as_mut() {
            reader.rendered = rendered;
            reader.scroll = scroll.min(reader.rendered.lines.len().saturating_sub(1));
        }
        self.update_search();
    }

    pub fn apply_scheme(&mut self, name: &str) {
        self.scheme = Scheme::load(name);
        self.preview = None;
        self.reload_reader();
        self.status = Some(format!("theme: {}", self.scheme.name));
    }

    pub fn update_search(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }
        if let Some(reader) = &self.reader {
            let q = self.search_query.to_lowercase();
            for (i, line) in reader.rendered.plain.iter().enumerate() {
                if line.to_lowercase().contains(&q) {
                    self.search_matches.push(i);
                }
            }
        }
    }

    fn jump_match(&mut self, forward: bool) {
        if self.search_matches.is_empty() {
            return;
        }
        let Some(reader) = self.reader.as_mut() else { return };
        let cur = reader.scroll;
        let next = if forward {
            self.search_matches
                .iter()
                .copied()
                .find(|&m| m > cur)
                .or_else(|| self.search_matches.first().copied())
        } else {
            self.search_matches
                .iter()
                .copied()
                .rev()
                .find(|&m| m < cur)
                .or_else(|| self.search_matches.last().copied())
        };
        if let Some(line) = next {
            reader.scroll = line;
        }
    }
}

/// Recursively collect markdown files under `dir`, skipping hidden and
/// build directories.
pub fn scan_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 8 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if path.is_dir() {
                walk(&path, depth + 1, out);
            } else if name
                .rsplit('.')
                .next()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
            {
                out.push(path);
            }
        }
    }
    walk(dir, 0, &mut out);
    out.sort();
    out
}

pub fn run(
    start_file: Option<PathBuf>,
    scheme: Scheme,
    level: ColorLevel,
    max_width: usize,
    mouse: bool,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    if mouse {
        execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    } else {
        execute!(stdout, EnterAlternateScreen)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, start_file, scheme, level, max_width);

    disable_raw_mode()?;
    if mouse {
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
    } else {
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    }
    terminal.show_cursor()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    start_file: Option<PathBuf>,
    scheme: Scheme,
    level: ColorLevel,
    max_width: usize,
) -> Result<()> {
    let mut app = App::new(scheme, level, max_width);
    if let Some(path) = start_file {
        let term_w = terminal.size()?.width;
        let width = content_width(term_w, max_width);
        let offset = term_w.saturating_sub(2).saturating_sub(width) / 2;
        app.open_reader(path, width, offset);
    }

    while !app.quit {
        terminal.draw(|frame| crate::ui::draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => handle_key(&mut app, key),
                Event::Mouse(m) => handle_mouse(&mut app, m.kind),
                Event::Resize(_, _) => {
                    app.preview = None;
                    if let Some(reader) = app.reader.as_mut() {
                        reader.width = 0; // force re-render in draw
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub fn content_width(term_width: u16, max_width: usize) -> u16 {
    let w = term_width.saturating_sub(2) as usize;
    w.min(max_width).max(20) as u16
}

fn scroll_reader(app: &mut App, delta: isize) {
    if let Some(reader) = app.reader.as_mut() {
        let max = reader.rendered.lines.len().saturating_sub(reader.view_height);
        let next = reader.scroll as isize + delta;
        reader.scroll = next.clamp(0, max as isize) as usize;
    }
}

/// Full-page scroll distance: one screen minus a 2-line overlap
/// (vim-style context); tiny views degrade to a single line.
fn page_delta(view_height: usize) -> usize {
    view_height.saturating_sub(2).max(1)
}

fn handle_key(app: &mut App, key: KeyEvent) {
    app.status = None;

    // Search input mode swallows everything.
    if app.searching {
        match key.code {
            KeyCode::Esc => {
                app.searching = false;
                app.search_query.clear();
                app.search_matches.clear();
            }
            KeyCode::Enter => {
                app.searching = false;
                app.update_search();
                app.jump_match(true);
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char(c) => app.search_query.push(c),
            _ => {}
        }
        return;
    }

    // Theme picker captures navigation when open.
    if let Some(sel) = app.picker {
        match key.code {
            KeyCode::Esc | KeyCode::Char('t') => app.picker = None,
            KeyCode::Char('j') | KeyCode::Down => {
                let next = (sel + 1) % app.schemes.len();
                app.picker = Some(next);
                let name = app.schemes[next].clone();
                app.apply_scheme(&name);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let next = (sel + app.schemes.len() - 1) % app.schemes.len();
                app.picker = Some(next);
                let name = app.schemes[next].clone();
                app.apply_scheme(&name);
            }
            KeyCode::Enter => app.picker = None,
            _ => {}
        }
        return;
    }

    if key.code == KeyCode::Char('?') {
        app.show_help = !app.show_help;
        return;
    }
    if app.show_help {
        app.show_help = false;
        return;
    }

    match key.code {
        KeyCode::Char('t') => {
            let idx = app
                .schemes
                .iter()
                .position(|n| n == &app.scheme.name)
                .unwrap_or(0);
            app.picker = Some(idx);
        }
        _ => match app.mode {
            Mode::Browser => browser_key(app, key),
            Mode::Reader => reader_key(app, key),
        },
    }
}

fn browser_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.files.is_empty() {
                app.selected = (app.selected + 1).min(app.files.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.selected = app.selected.saturating_sub(1);
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if let Some(path) = app.files.get(app.selected).cloned() {
                app.preview = None;
                app.open_reader(path, app.max_width as u16, 0);
            }
        }
        KeyCode::Char('r') => {
            app.files = scan_files(Path::new("."));
            app.preview = None;
        }
        _ => {}
    }
}

fn reader_key(app: &mut App, key: KeyEvent) {
    let page = app
        .reader
        .as_ref()
        .map(|r| r.view_height / 2)
        .unwrap_or(10) as isize;
    let full = app
        .reader
        .as_ref()
        .map(|r| page_delta(r.view_height))
        .unwrap_or(10) as isize;
    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Esc => app.mode = Mode::Browser,
        KeyCode::Char('j') | KeyCode::Down => scroll_reader(app, 1),
        KeyCode::Char('k') | KeyCode::Up => scroll_reader(app, -1),
        KeyCode::Char('d') | KeyCode::PageDown => scroll_reader(app, page),
        KeyCode::Char('u') | KeyCode::PageUp => scroll_reader(app, -page),
        KeyCode::Char('g') => scroll_reader(app, isize::MIN / 2),
        KeyCode::Char('G') => scroll_reader(app, isize::MAX / 2),
        KeyCode::Char('/') => {
            app.searching = true;
            app.search_query.clear();
        }
        KeyCode::Char('n') => app.jump_match(true),
        KeyCode::Char('N') => app.jump_match(false),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.quit = true,
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            scroll_reader(app, full)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            scroll_reader(app, -full)
        }
        _ => {}
    }
}

fn handle_mouse(app: &mut App, kind: MouseEventKind) {
    match kind {
        MouseEventKind::ScrollDown => match app.mode {
            Mode::Reader => scroll_reader(app, 3),
            Mode::Browser => {
                if !app.files.is_empty() {
                    app.selected = (app.selected + 3).min(app.files.len() - 1);
                }
            }
        },
        MouseEventKind::ScrollUp => match app.mode {
            Mode::Reader => scroll_reader(app, -3),
            Mode::Browser => {
                app.selected = app.selected.saturating_sub(3);
            }
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rendered;
    use crate::style::{ColorLevel, Scheme};

    fn test_app(lines: usize, view_height: usize) -> App {
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let mut app = App::new(scheme, ColorLevel::True, 100);
        app.mode = Mode::Reader;
        app.reader = Some(Reader {
            path: PathBuf::from("test.md"),
            rendered: Rendered {
                lines: vec![Vec::new(); lines],
                plain: vec![String::new(); lines],
            },
            width: 80,
            offset: 0,
            scroll: 0,
            view_height,
        });
        app
    }

    fn ctrl(key: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(key), KeyModifiers::CONTROL)
    }

    #[test]
    fn page_delta_full_screen_minus_overlap() {
        assert_eq!(page_delta(24), 22);
        assert_eq!(page_delta(2), 1, "tiny view degrades to 1 line");
        assert_eq!(page_delta(1), 1);
    }

    #[test]
    fn ctrl_f_scrolls_full_page() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 22);
    }

    #[test]
    fn ctrl_b_scrolls_back_and_clamps_at_top() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 0);
        // Already at top: stays 0.
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn ctrl_f_clamps_at_bottom() {
        let mut app = test_app(100, 24);
        app.reader.as_mut().unwrap().scroll = 70;
        handle_key(&mut app, ctrl('f'));
        assert_eq!(app.reader.as_ref().unwrap().scroll, 76); // 100 - 24
    }
}
