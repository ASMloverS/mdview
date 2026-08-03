# Writing Custom Themes

[中文](custom-themes.zh-CN.md)

mdview themes are plain CSS files written in a deliberately small CSS subset — colors, bold/italic/underline, and border colors. This guide covers everything the subset supports, how themes are loaded, and how each selector maps to rendered Markdown elements.

## Contents

- [Quick Start](#quick-start)
- [Loading and Enabling Themes](#loading-and-enabling-themes)
- [Syntax Highlighting Themes (syntax-styles)](#syntax-highlighting-themes-syntax-styles)
- [Selectors](#selectors)
- [Properties](#properties)
- [Colors](#colors)
- [Element Reference](#element-reference)
- [Complete Example](#complete-example)
- [Pitfalls](#pitfalls)
- [Further Reference](#further-reference)

## Quick Start

1. Create `md-styles/my-theme.css` next to the `mdview` executable.
2. Write a few rules:

   ```css
   body { color: #d8dee9; background-color: #2e3440; }
   h1 { color: #bf616a; font-weight: bold; }
   code { color: #ebcb8b; }
   ```

3. Use it: `mdview -t my-theme README.md`, or press `t` inside mdview and pick `my-theme` from the list.

The file's base name is the theme name. That's the whole workflow — the rest of this guide is the reference.

## Loading and Enabling Themes

**Lookup order** when a theme named `<name>` is requested:

1. `md-styles/<name>.css` next to the executable
2. A builtin theme with the same name
3. The default theme `gruvbox-dark` (a nonexistent name falls back here silently)

A file in `md-styles/` with the same name as a builtin **overrides** that builtin entirely.

**How a theme gets selected** (highest priority first):

1. CLI flag: `mdview -t <name>` / `--theme <name>` — one-shot, never persisted
2. `theme = "<name>"` in `config.toml` (next to the executable)
3. Builtin default `gruvbox-dark`

**Discovery**: `mdview --list-themes` prints all available themes (builtins plus every `.css` in `md-styles/`, deduplicated and sorted). The in-app theme picker (`t` key) lists custom themes alongside builtins and writes your choice back to `config.toml` when it closes.

## Syntax Highlighting Themes (syntax-styles)

Code-block syntax highlighting colors are independent of the page theme. They are defined by `syntax-styles/<name>.css` next to the executable; 20 builtin syntax themes ship under the same names as the page themes. A file in `syntax-styles/` with the same name as a builtin **overrides** that builtin entirely. Select one with the `syntax_theme` key in `config.toml` or the `--syntax-theme` flag; when unset, it automatically follows the page theme's namesake syntax theme. `--list-syntax-themes` lists every available syntax theme.

The CSS syntax is the same subset as page themes. Selectors are the 16 token classes, and a language name can be used as an ancestor selector for per-language specialization:

```css
keyword { color: #fb4934; font-weight: bold; }
comment { color: #928374; font-style: italic; }
rust macro { color: #fe8019; }      /* applies to rust only */
```

Classes: keyword / string / comment / function / type / number / operator / variable / constant / macro / attribute / decorator / module / namespace / punctuation / label.

Supported properties: `color`, `font-weight`, `font-style`, `text-decoration: underline` (`background` and `line-through` have no effect on code tokens).

Per-class fallback order: language-specific rule > global class rule > page theme `syntax-*` rule > alias derivation > code-block default foreground. Aliases: constant→number, macro/decorator→function, attribute/module/namespace→type, punctuation→operator, label→keyword, variable→default foreground.

Language names match both the code fence's info string verbatim (lowercased) and the canonical language name — so ```` ```rs ```` also hits `rust` rules. `bin/syntax-styles/example.css` is a fully annotated template.

## Selectors

Supported:

- **Element selectors**: `h1`, `blockquote`, `syntax-keyword`, … (letters, digits, hyphens; case-insensitive)
- **Descendant selectors**: `table td`. Note that `>` is treated as a plain space — `table > td` and `table td` are identical; there is no child-only matching.
- **Comma groups**: `table, th, td { ... }`

Not supported (silently stripped or skipped):

- **Class / ID / pseudo-class suffixes are stripped**: `a:hover` becomes `a`, `p.note` becomes `p`. The rule still applies — possibly more broadly than you intended.
- **Universal selector `*`**: the whole rule is skipped.

Matching rules:

- **Specificity = number of elements** in the selector. `blockquote p` beats `p`.
- On equal specificity, **later rules override earlier ones property by property** (not rule by rule).
- **No inheritance**: `p { color: red }` does not affect `strong` inside a paragraph. Each element is styled only by rules whose last selector component is that element. Anything unmatched falls back to the global default: foreground `#d4d4d4`, no background, no bold/italic/underline.

## Properties

Exactly seven declarations are recognized. Everything else (including all shorthand properties such as `border:`, `font:`, `margin`) is ignored. Property names and values are compared case-insensitively.

| Declaration | Effect | Accepted values |
| --- | --- | --- |
| `color` | Foreground color | Any color value (see [Colors](#colors)) |
| `background` / `background-color` | Background color (the two names are fully equivalent; solid colors only) | Any color value |
| `border-color` | Color of decorative lines/borders (h1/h2 rules, quote bar, table grid, `hr`) | Any color value |
| `font-weight` | Bold | `bold`, `bolder`, `600`–`900` → on; anything else → off |
| `font-style` | Italic | `italic`, `oblique` → on; anything else → off |
| `text-decoration` / `text-decoration-line` | Underline and strikethrough | Value containing `underline` → underline; containing `line-through` or `strikethrough` → strikethrough. One declaration sets both flags, so `text-decoration: none` explicitly turns both off. |

## Colors

Three syntaxes; everything is converted to truecolor internally:

- **Hex**: `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha is ignored)
- **Function**: `rgb(r, g, b)` — exactly three comma-separated integers 0–255. No `rgba()`, `hsl()`, or percentages.
- **Named colors** (18): `black`, `white`, `red`, `green`, `blue`, `yellow`, `orange`, `purple`, `pink`, `cyan`/`aqua`, `magenta`/`fuchsia`, `gray`/`grey`, `silver`, `teal`, `lime`, `navy`, `maroon`, `olive`

**Terminal degradation is automatic** — always author themes in truecolor:

- `COLORTERM` contains `truecolor`/`24bit`, or running in Windows Terminal → truecolor
- `TERM` contains `256color` → nearest xterm-256 color
- Otherwise → nearest of the 16 basic ANSI colors

## Element Reference

Markdown elements are styled by the tag at the end of their ancestor chain. This table lists every tag a theme can target and any special behavior.

| Tag | Renders | Notes |
| --- | --- | --- |
| `body` | Global default text and background | Its background also paints the whole TUI screen |
| `h1`–`h6` | Headings | `h1`/`h2` draw a full-width rule below the heading (`═` for h1, `─` for h2) colored by `border-color` (falls back to `color`); `h3`–`h6` draw no rule |
| `p` | Paragraph text | |
| `strong` / `em` / `del` | Bold / italic / strikethrough spans | |
| `code` | Inline code | **Only inline code** — code blocks never match `code`, so `pre code` has no target |
| `pre` | Code blocks | `color` is the default code text color (and for languages without highlighting); `background` fills the block. The line-number gutter and language tag use `footnote`'s foreground on `pre`'s background |
| `a` | Link text | The end-of-document link list title also uses `a`; the `[n]` markers in it use `footnote` |
| `img` | Image placeholder `🖼 alt` | |
| `math` | Inline and block math | Block math is centered; when it can't be rendered as Unicode it falls back to `pre` styling |
| `blockquote` | Quote blocks | `color` = text, `background` = full-line band, `border-color` = the left `▎` bar (falls back to `color`) |
| `li` | List markers only | Controls the color of `•` / `1.` / `☑`; list item text is still styled by `p` etc. |
| `table` / `th` / `td` | Tables | `table`'s `border-color` colors all grid lines; `th` = header cells, `td` = body cells |
| `hr` | Horizontal rule | `border-color`, falls back to `color` |
| `footnote` | Footnote refs/definitions | Also doubles as the global "dim" color: code gutter line numbers, language tags, link `[n]` markers, and other secondary text |
| `cursor` | Cursor line in the TUI reader | Only `background-color` is used; omit it for no cursor-line highlight |
| `syntax-keyword` | Keywords in code blocks | Color only |
| `syntax-string` | Strings | Color only |
| `syntax-comment` | Comments | Color only — comments are **always italic**, forced by the highlighter regardless of `font-style` |
| `syntax-function` | Function names | Color only |
| `syntax-type` | Types | Color only |
| `syntax-number` | Numbers | Color only |
| `syntax-operator` | Operators | Color only |

## Complete Example

A full theme covering every tag, section by section. Save it as `md-styles/my-theme.css` next to the executable, then run `mdview -t my-theme`.

```css
/* my-theme — a complete custom theme for mdview */

/* Global defaults: text color and the background of the whole screen */
body { color: #d5c4a1; background-color: #1d2021; }

/* Headings: h1/h2 also set border-color for the rule drawn beneath them */
h1 { color: #fb4934; font-weight: bold; border-color: #fb4934; }
h2 { color: #fe8019; font-weight: bold; border-color: #fe8019; }
h3 { color: #fabd2f; font-weight: bold; }
h4 { color: #b8bb26; font-weight: bold; }
h5 { color: #8ec07c; font-weight: bold; }
h6 { color: #928374; font-weight: bold; }

/* Inline text: paragraphs and emphasis spans */
p { color: #d5c4a1; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #928374; text-decoration: line-through; }
a { color: #83a598; text-decoration: underline; }
img { color: #928374; }
math { color: #d3869b; font-style: italic; }

/* Code: inline code, then the code block itself */
code { color: #fabd2f; background-color: #32302f; }
pre { color: #d5c4a1; background-color: #32302f; }

/* Syntax highlighting inside code blocks (color only) */
syntax-keyword { color: #fb4934; }
syntax-string { color: #b8bb26; }
syntax-comment { color: #928374; }
syntax-function { color: #b8bb26; }
syntax-type { color: #fabd2f; }
syntax-number { color: #d3869b; }
syntax-operator { color: #8ec07c; }

/* Blocks: quote, lists, tables, horizontal rule */
blockquote { color: #bdae93; background-color: #32302f; border-color: #504945; }
li { color: #fe8019; }
table, th, td { color: #d5c4a1; border-color: #504945; }
th { font-weight: bold; }
hr { border-color: #504945; }

/* Secondary text and UI: footnotes/dim color, cursor line highlight */
footnote { color: #928374; }
cursor { background-color: #3c3836; }
```

Editing tips:

- Iterate quickly with the picker: open any file, press `t`, select your theme, edit the CSS, quit and reopen to see changes.
- Compare against the builtins in `assets/styles/` — each is a complete, minimal theme of about 30 rules.

## Pitfalls

- **A missing `}` drops every rule after it.** The parser stops at the unterminated rule and discards the rest of the file. This is the only error with non-local impact.
- **Everything else fails silently.** Unknown properties, malformed declarations, unsupported selectors — all ignored without any warning. If a rule "does nothing," check its spelling first.
- **Custom themes don't merge with builtins.** A custom theme is a complete, standalone ruleset. Any element you don't style falls back to the global default (`#d4d4d4` foreground, no background) — not to the builtin default theme.
- **An invalid color invalidates only that declaration** — it leaves the property unset rather than clearing a valid value set by an earlier matching rule.
- **`p.note` becomes `p`.** Class/ID/pseudo-class suffixes are stripped, so a rule may match more broadly than written.
- **`pre code` matches nothing.** Code blocks are styled through `pre` alone; `code` is inline code only.
- **`>` means "descendant", not "child".** `table > td` and `table td` are equivalent.
- **Comments are always italic in code blocks**, forced by the syntax highlighter. `syntax-comment { font-style: normal }` has no effect.
- **An unknown theme name silently falls back** to `gruvbox-dark`. Run `mdview --list-themes` if your theme doesn't seem to load.

## Further Reference

- `assets/styles/` — the 20 builtin themes, the best starting templates
- `src/style/css.rs` — the subset parser (definitive list of supported syntax)
- `src/style/scheme.rs` — theme lookup, matching, and fallback logic
