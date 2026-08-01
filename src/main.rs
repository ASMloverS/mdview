//! mdview — terminal markdown renderer with a TUI file browser.

mod app;
mod browse;
mod config;
mod highlight;
mod history;
mod markdown;
mod math;
mod render;
mod style;
mod ui;

use anyhow::Result;
use clap::Parser;
use config::{Config, ContentAlign};
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use style::{ColorLevel, Scheme, DEFAULT_THEME};

#[derive(Parser)]
#[command(name = "mdview", version, about = "Terminal markdown renderer")]
struct Cli {
    /// Markdown file to open directly. With no file, resumes the last opened
    /// file (file browser on first run).
    file: Option<PathBuf>,

    /// Theme name: a builtin scheme or `md-styles/<name>.css`.
    #[arg(short, long)]
    theme: Option<String>,

    /// Maximum content width in columns.
    #[arg(short = 'w', long)]
    max_width: Option<usize>,

    /// Content alignment: center (default) or left.
    #[arg(long, value_enum)]
    align: Option<ContentAlign>,

    /// List all available themes and exit.
    #[arg(long)]
    list_themes: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_themes {
        for name in Scheme::available() {
            println!("{name}");
        }
        return Ok(());
    }

    let cfg = Config::load();
    let sidebar_width = cfg.sidebar_width();
    let theme_name = cli
        .theme
        .or(cfg.theme)
        .unwrap_or_else(|| DEFAULT_THEME.to_string());
    let scheme = Scheme::load(&theme_name);
    let level = ColorLevel::detect();
    let max_width = cli.max_width.or(cfg.max_width).unwrap_or(100);
    let align = cli
        .align
        .or_else(|| cfg.align.as_deref().and_then(ContentAlign::from_str))
        .unwrap_or(ContentAlign::Center);

    // Pipe mode: markdown on stdin, ANSI on stdout.
    let stdin_is_pipe = !std::io::stdin().is_terminal();
    if cli.file.is_none() && stdin_is_pipe {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        let doc = markdown::parse_document(&text);
        let term = terminal_width();
        let width = term.min(max_width);
        let offset = app::content_offset(term as u16, width as u16, align) as usize;
        let rendered = render::layout::render_document(&doc, &scheme, width, offset);
        print!("{}", render::ansi::render_ansi(&rendered.lines, level));
        return Ok(());
    }

    let history_size = cfg.history_size.unwrap_or(history::DEFAULT_HISTORY_SIZE);
    app::run(
        cli.file,
        scheme,
        level,
        max_width,
        cfg.mouse.unwrap_or(true),
        align,
        history_size,
        sidebar_width,
    )
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .saturating_sub(2)
        .max(20)
}
