//! TUI application: state machine and event loop.

use crate::browse::{Browser, EnterOutcome};
use crate::config::{Config, ContentAlign};
use crate::history::History;
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
    pub cursor: usize,
    pub view_height: usize,
}

pub struct App {
    pub mode: Mode,
    pub browser: Browser,
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
    pub align: ContentAlign,
    pub history: History,
    pub history_size: usize,
    pub show_help: bool,
    pub resume_hint: bool,
    pub status: Option<String>,
    pub quit: bool,
}

impl App {
    pub fn new(
        scheme: Scheme,
        level: ColorLevel,
        max_width: usize,
        align: ContentAlign,
        history_size: usize,
    ) -> App {
        App {
            mode: Mode::Browser,
            browser: Browser::from_cwd(),
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
            align,
            history: History::load(),
            history_size,
            show_help: false,
            resume_hint: false,
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
        // 浏览器同步定位到所打开的文件（best-effort）。
        if let Err(msg) = self.browser.reveal(&path) {
            self.status = Some(msg);
        }
        let rendered = self.render_file(&path, width, offset);
        let mut reader = Reader {
            path,
            rendered,
            width,
            offset,
            scroll: 0,
            cursor: 0,
            view_height: 24,
        };
        // 恢复上次关闭时的光标行（history_size = 0 禁用）。
        if self.history_size > 0 {
            if let Some(line) = self.history.get(&reader.path) {
                let last = reader.rendered.lines.len().saturating_sub(1);
                reader.cursor = line.min(last);
                reader.scroll = reader.cursor.saturating_sub(reader.view_height / 2);
            }
        }
        self.reader = Some(reader);
        self.search_matches.clear();
        self.mode = Mode::Reader;
    }

    /// 无参数启动：恢复最近可读文件（光标由 open_reader 恢复）；
    /// 无可用历史时进浏览器并弹首次使用提示。
    /// history_size = 0（禁用历史）时静默进浏览器。
    pub fn resume_latest(&mut self, width: u16, offset: u16) {
        if self.history_size == 0 {
            return;
        }
        match self.history.latest_valid() {
            Some(path) => self.open_reader(path, width, offset),
            None => self.resume_hint = true,
        }
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
            let last = reader.rendered.lines.len().saturating_sub(1);
            reader.scroll = scroll.min(last);
            reader.cursor = reader.cursor.min(last);
        }
        self.update_search();
    }

    pub fn apply_scheme(&mut self, name: &str) {
        self.scheme = Scheme::load(name);
        self.preview = None;
        self.reload_reader();
        self.status = Some(format!("theme: {}", self.scheme.name));
    }

    /// 切换居中/左对齐并提示；重排版由 draw 的 offset 比较自动触发。
    pub fn toggle_align(&mut self) {
        self.align = self.align.toggle();
        self.status = Some(format!("align: {}", self.align.as_str()));
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
        let cur = reader.cursor;
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
            reader.cursor = line;
            follow_cursor(reader);
        }
    }
}

pub fn run(
    start_file: Option<PathBuf>,
    scheme: Scheme,
    level: ColorLevel,
    max_width: usize,
    mouse: bool,
    align: ContentAlign,
    history_size: usize,
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

    let result = event_loop(&mut terminal, start_file, scheme, level, max_width, align, history_size);

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
    align: ContentAlign,
    history_size: usize,
) -> Result<()> {
    let mut app = App::new(scheme, level, max_width, align, history_size);
    let term_w = terminal.size()?.width;
    let width = content_width(term_w, max_width);
    let offset = content_offset(term_w.saturating_sub(2), width, app.align);
    if let Some(path) = start_file {
        app.open_reader(path, width, offset);
    } else {
        app.resume_latest(width, offset);
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

/// 内容水平偏移：Center 居中，Left 贴左（0）。
/// inner 为去掉边框后的可用宽度。
pub fn content_offset(inner_width: u16, width: u16, align: ContentAlign) -> u16 {
    match align {
        ContentAlign::Center => inner_width.saturating_sub(width) / 2,
        ContentAlign::Left => 0,
    }
}

/// 滚动跟随：调整 scroll 让光标落在可视区内。
fn follow_cursor(reader: &mut Reader) {
    if reader.cursor < reader.scroll {
        reader.scroll = reader.cursor;
    } else if reader.cursor >= reader.scroll + reader.view_height {
        reader.scroll = reader.cursor + 1 - reader.view_height;
    }
}

/// 关闭阅读器时记录当前光标行（best-effort；history_size = 0 禁用）。
fn save_position(app: &mut App) {
    if app.history_size == 0 {
        return;
    }
    let Some((path, line)) = app.reader.as_ref().map(|r| (r.path.clone(), r.cursor)) else {
        return;
    };
    app.history.record(&path, line, app.history_size);
}

/// 键盘移动：光标移动 delta 行（clamp 到文档范围），滚动跟随。
fn move_cursor(app: &mut App, delta: isize) {
    let Some(reader) = app.reader.as_mut() else { return };
    let last = reader.rendered.lines.len().saturating_sub(1);
    let next = reader.cursor as isize + delta;
    reader.cursor = next.clamp(0, last as isize) as usize;
    follow_cursor(reader);
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
            KeyCode::Esc | KeyCode::Char('t') => close_picker(app),
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
            KeyCode::Enter => close_picker(app),
            _ => {}
        }
        return;
    }

    if app.resume_hint {
        app.resume_hint = false;
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
        KeyCode::Char('j') | KeyCode::Down => app.browser.move_sel(1),
        KeyCode::Char('k') | KeyCode::Up => app.browser.move_sel(-1),
        KeyCode::Char('o') => match app.browser.enter() {
            EnterOutcome::OpenFile(path) => {
                app.preview = None;
                app.open_reader(path, app.max_width as u16, 0);
            }
            EnterOutcome::Failed(msg) => app.status = Some(msg),
            EnterOutcome::Entered => app.preview = None,
            EnterOutcome::Noop => {}
        },
        KeyCode::Left | KeyCode::Backspace => {
            if let Err(msg) = app.browser.up() {
                app.status = Some(msg);
            }
            app.preview = None;
        }
        KeyCode::Char('r') => {
            if let Err(msg) = app.browser.refresh() {
                app.status = Some(msg);
            }
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
        KeyCode::Char('q') => {
            save_position(app);
            app.quit = true;
        }
        KeyCode::Esc => {
            save_position(app);
            app.mode = Mode::Browser;
        }
        KeyCode::Char('j') | KeyCode::Down => move_cursor(app, 1),
        KeyCode::Char('k') | KeyCode::Up => move_cursor(app, -1),
        KeyCode::Char('d') | KeyCode::PageDown => move_cursor(app, page),
        KeyCode::Char('u') | KeyCode::PageUp => move_cursor(app, -page),
        KeyCode::Char('g') => move_cursor(app, isize::MIN / 2),
        KeyCode::Char('G') => move_cursor(app, isize::MAX / 2),
        KeyCode::Char('/') => {
            app.searching = true;
            app.search_query.clear();
        }
        KeyCode::Char('n') => app.jump_match(true),
        KeyCode::Char('N') => app.jump_match(false),
        KeyCode::Char('a') => {
            app.toggle_align();
            Config::save_align(app.align.as_str());
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            save_position(app);
            app.quit = true;
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            move_cursor(app, full)
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            move_cursor(app, -full)
        }
        _ => {}
    }
}

/// Close the theme picker and persist the current theme selection.
fn close_picker(app: &mut App) {
    app.picker = None;
    Config::save_theme(&app.scheme.name);
}

fn handle_mouse(app: &mut App, kind: MouseEventKind) {
    match kind {
        MouseEventKind::ScrollDown => match app.mode {
            Mode::Reader => scroll_reader(app, 3),
            Mode::Browser => app.browser.move_sel(3),
        },
        MouseEventKind::ScrollUp => match app.mode {
            Mode::Reader => scroll_reader(app, -3),
            Mode::Browser => app.browser.move_sel(-3),
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::{Browser, Loc};
    use crate::render::Rendered;
    use crate::style::{ColorLevel, Scheme};

    fn test_app(lines: usize, view_height: usize) -> App {
        let scheme = Scheme::load(crate::style::DEFAULT_THEME);
        let mut app = App::new(scheme, ColorLevel::True, 100, ContentAlign::Center, 0);
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
            cursor: 0,
            view_height,
        });
        app
    }

    fn ctrl(key: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(key), KeyModifiers::CONTROL)
    }

    #[test]
    fn content_offset_centers_and_left_aligns() {
        assert_eq!(content_offset(100, 80, ContentAlign::Center), 10);
        assert_eq!(content_offset(100, 80, ContentAlign::Left), 0);
        assert_eq!(content_offset(50, 80, ContentAlign::Center), 0, "窄终端不溢出");
    }

    #[test]
    fn page_delta_full_screen_minus_overlap() {
        assert_eq!(page_delta(24), 22);
        assert_eq!(page_delta(2), 1, "tiny view degrades to 1 line");
        assert_eq!(page_delta(1), 1);
    }

    #[test]
    fn j_moves_cursor_without_scrolling() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 1);
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn k_clamps_cursor_at_top() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 0);
    }

    #[test]
    fn cursor_past_bottom_edge_scrolls_down() {
        let mut app = test_app(100, 24);
        app.reader.as_mut().unwrap().cursor = 23;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 24);
        assert_eq!(r.scroll, 1);
    }

    #[test]
    fn cursor_past_top_edge_scrolls_up() {
        let mut app = test_app(100, 24);
        let r = app.reader.as_mut().unwrap();
        r.cursor = 10;
        r.scroll = 10;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('k')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 9);
        assert_eq!(r.scroll, 9);
    }

    #[test]
    fn d_moves_cursor_half_page() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('d')));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 12);
    }

    #[test]
    fn ctrl_f_moves_cursor_one_page() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 22);
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn ctrl_b_moves_back_and_clamps() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, ctrl('f'));
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 0);
        // 已在顶部：保持 0。
        handle_key(&mut app, ctrl('b'));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 0);
    }

    #[test]
    fn ctrl_f_clamps_at_last_line_and_scroll_follows() {
        let mut app = test_app(100, 24);
        app.reader.as_mut().unwrap().cursor = 90;
        handle_key(&mut app, ctrl('f'));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 99);
        assert_eq!(r.scroll, 76); // 99 + 1 - 24
    }

    #[test]
    fn g_and_g_upper_jump_to_ends() {
        let mut app = test_app(100, 24);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('G')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 99);
        assert_eq!(r.scroll, 76);
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('g')));
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 0);
        assert_eq!(r.scroll, 0);
    }

    #[test]
    fn search_jump_moves_cursor_and_scroll_follows() {
        let mut app = test_app(100, 24);
        app.search_matches = vec![10, 50];
        app.jump_match(true);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 10);
        assert_eq!(r.scroll, 0);
        app.jump_match(true);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 50);
        assert_eq!(r.scroll, 27); // 50 + 1 - 24
    }

    #[test]
    fn reload_clamps_cursor_and_scroll() {
        let mut app = test_app(100, 24);
        let r = app.reader.as_mut().unwrap();
        r.cursor = 90;
        r.scroll = 80;
        app.reload_reader();
        let r = app.reader.as_ref().unwrap();
        // test.md 不存在 → 渲染为单行错误文本，cursor/scroll 都 clamp 到 0。
        assert!(r.rendered.lines.len() < 100);
        let last = r.rendered.lines.len() - 1;
        assert!(r.cursor <= last);
        assert!(r.scroll <= last);
    }

    #[test]
    fn toggle_align_flips_and_sets_status() {
        let mut app = test_app(10, 24);
        assert_eq!(app.align, ContentAlign::Center);
        app.toggle_align();
        assert_eq!(app.align, ContentAlign::Left);
        assert_eq!(app.status.as_deref(), Some("align: left"));
        app.toggle_align();
        assert_eq!(app.align, ContentAlign::Center);
        assert_eq!(app.status.as_deref(), Some("align: center"));
    }

    /// 造一个临时 md 文件（lines 个单段段落），返回 (目录, 文件路径)。
    fn temp_doc(tag: &str, lines: usize) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdview-app-hist-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("note.md");
        let text = (0..lines)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        std::fs::write(&file, text).unwrap();
        (dir, file)
    }

    #[test]
    fn open_reader_restores_cursor_from_history() {
        let (dir, file) = temp_doc("restore", 40);
        let mut app = test_app(10, 24);
        app.history_size = 200;
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 30, 200);
        app.history = h;
        app.open_reader(file, 80, 0);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 30);
        assert_eq!(r.scroll, 30 - 24 / 2, "光标大致居中（view_height 占位 24）");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_reader_without_record_starts_at_top() {
        let (dir, file) = temp_doc("fresh", 10);
        let mut app = test_app(10, 24);
        app.history_size = 200;
        app.history = History::load_from(&dir.join("history.toml"));
        app.open_reader(file, 80, 0);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 0);
        assert_eq!(r.scroll, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_reader_clamps_recorded_line() {
        let (dir, file) = temp_doc("clamp", 3);
        let mut app = test_app(10, 24);
        app.history_size = 200;
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 999, 200);
        app.history = h;
        app.open_reader(file, 80, 0);
        let r = app.reader.as_ref().unwrap();
        let last = r.rendered.lines.len() - 1;
        assert_eq!(r.cursor, last);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn esc_saves_cursor_position() {
        let (dir, file) = temp_doc("save", 40);
        let hist_path = dir.join("history.toml");
        let mut app = test_app(10, 24);
        app.history_size = 200;
        app.history = History::load_from(&hist_path);
        app.open_reader(file.clone(), 80, 0);
        app.reader.as_mut().unwrap().cursor = 7;
        handle_key(&mut app, KeyEvent::from(KeyCode::Esc));
        let h = History::load_from(&hist_path);
        assert_eq!(h.get(&file), Some(7));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn history_size_zero_disables_save_and_restore() {
        let (dir, file) = temp_doc("disabled", 40);
        let hist_path = dir.join("history.toml");
        let mut app = test_app(10, 24); // test_app 中 history_size = 0
        app.history = History::load_from(&hist_path);
        app.open_reader(file.clone(), 80, 0);
        app.reader.as_mut().unwrap().cursor = 7;
        handle_key(&mut app, KeyEvent::from(KeyCode::Esc));
        let h = History::load_from(&hist_path);
        assert_eq!(h.get(&file), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resume_latest_opens_reader_and_restores_cursor() {
        let (dir, file) = temp_doc("resume-open", 40);
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            200,
        );
        let mut h = History::load_from(&dir.join("history.toml"));
        h.record(&file, 30, 200);
        app.history = h;
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Reader));
        assert_eq!(app.reader.as_ref().unwrap().cursor, 30);
        assert!(!app.resume_hint);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resume_latest_empty_history_shows_hint_in_browser() {
        let dir = std::env::temp_dir().join(format!("mdview-app-hist-{}-resume-hint", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            200,
        );
        app.history = History::load_from(&dir.join("history.toml"));
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Browser));
        assert!(app.resume_hint);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resume_latest_disabled_history_is_silent() {
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            0, // history_size = 0：禁用历史
        );
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Browser));
        assert!(!app.resume_hint, "禁用历史时静默进浏览器");
    }

    #[test]
    fn resume_latest_all_stale_shows_hint_in_browser() {
        let dir = std::env::temp_dir().join(format!("mdview-app-hist-{}-resume-stale", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hist_path = dir.join("history.toml");
        let mut app = App::new(
            Scheme::load(crate::style::DEFAULT_THEME),
            ColorLevel::True,
            100,
            ContentAlign::Center,
            200,
        );
        let mut h = History::load_from(&hist_path);
        h.record(&dir.join("gone.md"), 7, 200); // 从不存在的文件
        app.history = h;
        app.resume_latest(80, 0);
        assert!(matches!(app.mode, Mode::Browser));
        assert!(app.resume_hint, "全部失效按空历史处理：弹提示");
        // 清空已写盘：重新加载后 latest_valid 仍为 None。
        let mut h2 = History::load_from(&hist_path);
        assert!(h2.latest_valid().is_none(), "失效条目清除已持久化");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resume_hint_dismissed_by_any_key() {
        let mut app = test_app(10, 24);
        app.resume_hint = true;
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('j')));
        assert!(!app.resume_hint);
        let r = app.reader.as_ref().unwrap();
        assert_eq!(r.cursor, 0, "按键被弹窗拦截，不传给阅读器");
    }

    /// 造临时目录：sub/ 子目录 + a.md 文件，返回目录路径。
    fn temp_browser_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-app-key-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "hello").unwrap();
        dir
    }

    #[test]
    fn o_on_file_opens_reader() {
        let dir = temp_browser_dir("open");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir);
        app.browser.selected = 1; // a.md（目录优先，sub 在 0）
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('o')));
        assert!(matches!(app.mode, Mode::Reader));
        assert_eq!(app.reader.as_ref().unwrap().path, dir.join("a.md"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn o_on_dir_enters_it() {
        let dir = temp_browser_dir("enter");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir);
        app.browser.selected = 0; // sub
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('o')));
        assert!(matches!(app.mode, Mode::Browser), "进目录不打开阅读器");
        assert_eq!(app.browser.loc, Loc::Dir(dir.join("sub")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enter_l_right_h_unbound_in_browser() {
        let dir = temp_browser_dir("unbound");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir);
        handle_key(&mut app, KeyEvent::from(KeyCode::Enter));
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('l')));
        handle_key(&mut app, KeyEvent::from(KeyCode::Right));
        handle_key(&mut app, KeyEvent::from(KeyCode::Char('h')));
        assert!(matches!(app.mode, Mode::Browser));
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn left_and_backspace_go_up() {
        let dir = temp_browser_dir("up");
        let mut app = test_app(10, 24);
        app.mode = Mode::Browser;
        app.browser = Browser::new(&dir.join("sub"));
        handle_key(&mut app, KeyEvent::from(KeyCode::Left));
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        app.browser = Browser::new(&dir.join("sub"));
        handle_key(&mut app, KeyEvent::from(KeyCode::Backspace));
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_reader_reveals_file_in_browser() {
        let (dir, file) = temp_doc("reveal-open", 5);
        let mut app = test_app(10, 24);
        app.open_reader(file.clone(), 80, 0);
        assert_eq!(app.browser.loc, Loc::Dir(dir.clone()));
        assert_eq!(app.browser.entries[app.browser.selected].path(), file.as_path());
        std::fs::remove_dir_all(&dir).ok();
    }
}
