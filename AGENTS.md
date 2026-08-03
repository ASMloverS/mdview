# mdview Agent Guide

Terminal markdown renderer in Rust. Glow-style TUI, VS Code-style rendering.

## Build & Test

- Windows/MSVC: **always** use the wrapper batch file — plain `cargo` picks up
  Git's `link.exe` and fails to link:
  - `cmd //c ".cargo-vc.bat test"` (from Git Bash)
  - `cmd //c ".cargo-vc.bat build"`
- Distribution build: `build.bat` (release) / `build.bat -d` (debug) —
  assembles `bin/` (mdview.exe + config.toml + md-styles/).
- Run the full test suite before claiming any change is done. Zero-warning
  builds are the norm; keep it that way.

## Architecture

```
.md → pulldown-cmark → markdown::ir::Document
    → render::layout (styled lines: Vec<SLine>) 
    → render::ansi (pipe mode) / ui (ratatui TUI)
```

- `src/markdown/` — parsing + IR (`parse.rs`, `ir.rs`)
- `src/render/layout/` — layout engine: `mod.rs` (Renderer, line primitives,
  dispatch, tests), `text.rs` (paragraph/heading/list/inlines), `block.rs`
  (code/quote/table/rule/math/footnote), `decorate.rs` (shared decoration
  primitives)
- `src/render/ansi.rs` — one-shot ANSI output
- `src/style/` — CSS subset parser (`css.rs`), scheme registry (`scheme.rs`),
  colors (`color.rs`)
- `src/style/syntax.rs` — syntax theme registry (`syntax-styles/*.css`, 16 token
  classes)
- `src/history.rs` — per-file cursor position history (LRU, exe-adjacent `history.toml`)
- `src/browse.rs` — directory browser logic (single-level load, sorting/filtering,
  navigation, Windows drive list)
- `src/app.rs`, `src/ui/` — TUI state machine; sidebar (`sidebar.rs`) + reader views
- `assets/styles/` — 20 builtin themes (embedded via `include_str!`)
- `docs/superpowers/{specs,plans}/` — design specs and implementation plans
- `docs/custom-themes{,.zh-CN}.md` — bilingual user guide for custom CSS
  themes; keep in sync with `src/style/` when the CSS subset changes

## Conventions

- Themes: builtins live in `assets/styles/*.css`; user themes in
  `md-styles/*.css` next to the executable (lookup: exe-adjacent →
  builtin). CSS subset: element/descendant selectors,
  color/background/border/font properties only.
- Syntax themes: builtins in `assets/syntax-styles/*.css` (paired 1:1 with
  page themes); user themes in `syntax-styles/` next to the exe (lookup:
  exe-adjacent → builtin). Selected via `syntax_theme` in `config.toml` or
  `--syntax-theme`; unset → follows the page theme's name; per-class
  fallback to the page theme's `syntax-*` rules, then alias derivation.
- Default theme: `gruvbox-dark`. Config: `config.toml` next to the
  executable (theme persisted on picker close, `align` persisted on
  reader `a` toggle; preserve other keys when writing). Reading
  positions: `history.toml` next to the executable (LRU capped by
  `history_size`, 0 disables).
- Tests: in-file `#[cfg(test)] mod tests`. Render tests assert on plain-text
  output (`render_document(...).plain`) and on span styles.
- Code comments: Chinese, concise — match the existing style.
- Docs under `docs/superpowers/` (specs and plans): written in Chinese.
- Commit only when asked or when executing an approved plan; feature work
  happens on short-lived branches merged back to master.
- Commit message format (MANDATORY): `<gitmoji> <type>(<scope>): <message>`
  — e.g. `✨ feat(reader): add ctrl+f/b paging`. Emojis per gitmoji.dev.

## This File

- Every rule in AGENTS.md MUST be English, concise, and mandatory.

## Behavioral Guidelines

### Think Before Coding
- State assumptions explicitly. Uncertain → ask.
- Multiple interpretations exist → present all, don't pick silently.
- Simpler approach exists → say so. Push back when warranted.

### Simplicity First
- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- If you write 200 lines and it could be 50, rewrite it.

### Surgical Changes
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- Remove imports/variables/functions YOUR changes made unused.
- Every changed line must trace directly to the user's request.

### Goal-Driven Execution
- Transform tasks into verifiable goals:
  - "Add validation" → "Write tests for invalid inputs, make them pass"
  - "Fix the bug" → "Write a test that reproduces it, make it pass"
  - "Refactor X" → "Ensure tests pass before and after"
- Multi-step tasks → state a brief plan with verify checkpoints.

## File Hygiene
- UTF-8, LF endings.
- No trailing whitespace.
