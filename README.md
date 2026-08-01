# mdview

[中文](README.zh-CN.md)

Terminal Markdown renderer: [glow](https://github.com/charmbracelet/glow)-style TUI, VS Code-style rendering. Rust + ratatui + syntect.

## Features

- TUI file browser + reader (scroll, search, live theme switcher)
- VS Code-style decorations: h1/h2 rules, code gutter + lang tag, blockquote background, table header separator, centered content
- Syntax highlighting driven by the active theme
- 20 builtin themes + custom CSS themes; picker selection persists
- Pipe mode: markdown in, ANSI out

## Build

```bash
cargo build --release
```

On Windows (MSVC), if linking fails use the bundled wrapper: `.cargo-vc.bat build --release`

## Usage

```bash
mdview               # resume last file (file browser on first run)
mdview README.md     # open a file
cat a.md | mdview    # pipe mode
```

Options: `-t, --theme <name>` · `-w, --max-width <cols>` (default 100) · `--align <center|left>` · `--list-themes`

Keys: `j/k` scroll · `d/u` half page · `Ctrl+f/b` full page · `g/G` top/bottom · `/` `n/N` search · `t` themes · `a` align (reader) · `Enter` open · `Esc` back · `?` help · `q` quit

## Themes

Builtins: `tokyo-night`, `dracula`, `gruvbox-dark` (default), `gruvbox-light`, `nord`, `solarized-dark`, `solarized-light`, `github-light`, `github-dark`, `catppuccin-mocha`, `catppuccin-latte`, `kanagawa`, `rose-pine`, `rose-pine-dawn`, `everforest`, `everforest-light`, `one-dark`, `monokai`, `ayu-dark`, `ayu-light`.

Custom: write `md-styles/<name>.css` next to the executable (lookup: exe-adjacent `md-styles/` → builtins; same name overrides a builtin). Supported CSS subset: element/descendant selectors; `color`, `background(-color)`, `border-color`, `font-weight`, `font-style`, `text-decoration`; `syntax-keyword|string|comment|function|type|number|operator` classes for highlighting. See `assets/styles/` for examples.

Full guide: [Writing Custom Themes](docs/custom-themes.md).

## Config

`config.toml` (next to the executable, all optional):

```toml
theme = "gruvbox-dark"  # written automatically when the picker closes
max_width = 100
mouse = true
align = "center"        # or "left"; written automatically on 'a' toggle
history_size = 200      # remember cursor line per file (history.toml; 0 disables)
```

Priority: CLI flags > config.toml > builtin defaults (flags are one-shot, never persisted).
