# 新增 12 个内置主题实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 mdview 新增 12 个内置配色主题（8 暗色 + 4 亮色），内置主题从 8 个扩充到 20 个。

**Architecture:** 新主题全部复用现有 31 行 CSS 模板（与现有 8 个主题结构完全一致，仅色值不同），以 `include_str!` 嵌入二进制，在 `src/style/scheme.rs` 的 `BUILTINS` 数组中注册。不改 CSS 子集、不改渲染代码、不改默认主题（保持 `gruvbox-dark`）。色值全部取自各主题官方调色板仓库（catppuccin/palette、rebelot/kanagawa.nvim、rose-pine/palette、sainnhe/everforest、atom/one-dark-syntax、microsoft/vscode monokai 移植、ayu-theme/ayu-colors、primer/primitives），已联网核实。

**Tech Stack:** Rust, pulldown-cmark（不涉及）, cargo test via `cmd //c ".cargo-vc.bat test"`（Windows/MSVC 必须用此包装，直接 cargo 会链接失败）。

**分支：** `feat/more-builtin-themes`（从 main 切出，完成后合回 main）。

**提交格式（强制）：** `<gitmoji> <type>(<scope>): <message>`，见 AGENTS.md。

---

## 模板映射约定（所有新主题统一遵守）

参照现有主题的映射惯例：

- `body`/`p`：主文字色 + 主背景色
- `h1`=红/粉槽位、`h2`=橙槽位（均带同色 `border-color`）、`h3`=紫、`h4`=绿/青、`h5`=黄、`h6`=muted 灰
- `del`/`img`/`footnote`：muted 灰；`del` 带 `line-through`
- `code`：强调色 + surface 背景；`pre`：主文字 + surface 背景（`code`/`pre` 同背景）
- `a`：蓝/青链接色 + underline
- `blockquote`：比正文稍暗的次要文字色 + 边框色 + surface 背景
- `li`：强调色（多为蓝或橙）
- `table, th, td`：主文字 + 边框色；`th` 额外 bold；`hr`：边框色
- `math`：紫/粉 + italic
- `cursor`：官方光标行/高亮行背景（亮主题下比主背景略暗，暗主题下略亮）
- `syntax-*`：按各主题官方语法高亮语义映射

每个文件第 1 行注释：`/* mdview builtin scheme: <name> */`，共 31 行，UTF-8、LF。

---

## Task 1: 切分支 + 写失败测试（TDD Red）

**Files:**
- Test: `src/style/scheme.rs`（`#[cfg(test)] mod tests` 内追加两个测试）

- [ ] **Step 1: 切功能分支**

```bash
git checkout -b feat/more-builtin-themes
```

- [ ] **Step 2: 写失败测试**

在 `src/style/scheme.rs` 的 `mod tests` 中（`default_theme_is_gruvbox_dark` 测试之后）追加：

```rust
    #[test]
    fn new_builtin_schemes_registered() {
        let names = Scheme::builtin_names();
        for expected in [
            "catppuccin-mocha",
            "kanagawa",
            "rose-pine",
            "everforest",
            "one-dark",
            "monokai",
            "ayu-dark",
            "github-dark",
            "catppuccin-latte",
            "rose-pine-dawn",
            "everforest-light",
            "ayu-light",
        ] {
            assert!(names.contains(&expected), "missing builtin {expected}");
        }
    }

    #[test]
    fn builtin_schemes_cover_template_elements() {
        const TAGS: &[&str] = &[
            "body", "h1", "h2", "h3", "h4", "h5", "h6", "p", "strong", "em", "del",
            "code", "pre", "a", "blockquote", "li", "table", "th", "td", "hr", "img",
            "math", "footnote", "cursor",
        ];
        const SYNTAX: &[&str] = &[
            "keyword", "string", "comment", "function", "type", "number", "operator",
        ];
        for name in Scheme::builtin_names() {
            let s = Scheme::load(name);
            for tag in TAGS {
                let c = s.element(tag);
                assert!(
                    c.fg.is_some()
                        || c.bg.is_some()
                        || c.border.is_some()
                        || c.bold
                        || c.italic
                        || c.underline
                        || c.strike,
                    "builtin {name} missing style for {tag}"
                );
            }
            for class in SYNTAX {
                assert!(
                    s.syntax_color(class).is_some(),
                    "builtin {name} missing syntax-{class}"
                );
            }
        }
    }
```

- [ ] **Step 3: 运行测试确认按预期失败**

Run: `cmd //c ".cargo-vc.bat test style::scheme"`
Expected: `new_builtin_schemes_registered` FAIL（missing builtin catppuccin-mocha）；`builtin_schemes_cover_template_elements` 暂时 PASS（现有 8 个主题已覆盖模板，属预期）。

**注意：** 本任务不提交，测试随 Task 2 一起提交。

---

## Task 2: 暗色批次 1（catppuccin-mocha / kanagawa / rose-pine / everforest）

**Files:**
- Create: `assets/styles/catppuccin-mocha.css`
- Create: `assets/styles/kanagawa.css`
- Create: `assets/styles/rose-pine.css`
- Create: `assets/styles/everforest.css`
- Modify: `src/style/scheme.rs:33`（`BUILTINS` 中 `solarized-dark` 条目之后插入 4 条注册）

- [ ] **Step 1: 创建 `assets/styles/catppuccin-mocha.css`**

色板来源：catppuccin/palette v1.8.0 palette.json。

```css
/* mdview builtin scheme: catppuccin-mocha */
body { color: #cdd6f4; background-color: #1e1e2e; }
h1 { color: #f38ba8; font-weight: bold; border-color: #f38ba8; }
h2 { color: #fab387; font-weight: bold; border-color: #fab387; }
h3 { color: #cba6f7; font-weight: bold; }
h4 { color: #a6e3a1; font-weight: bold; }
h5 { color: #f9e2af; font-weight: bold; }
h6 { color: #9399b2; font-weight: bold; }
p { color: #cdd6f4; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #9399b2; text-decoration: line-through; }
code { color: #fab387; background-color: #313244; }
pre { color: #cdd6f4; background-color: #313244; }
a { color: #89b4fa; text-decoration: underline; }
blockquote { color: #a6adc8; border-color: #585b70; background: #313244; }
li { color: #89b4fa; }
table, th, td { color: #cdd6f4; border-color: #585b70; }
th { font-weight: bold; }
hr { border-color: #585b70; }
img { color: #9399b2; }
math { color: #f5c2e7; font-style: italic; }
footnote { color: #9399b2; }
cursor { background-color: #313244; }
syntax-keyword { color: #cba6f7; }
syntax-string { color: #a6e3a1; }
syntax-comment { color: #9399b2; font-style: italic; }
syntax-function { color: #89b4fa; }
syntax-type { color: #f9e2af; }
syntax-number { color: #fab387; }
syntax-operator { color: #89dceb; }
```

- [ ] **Step 2: 创建 `assets/styles/kanagawa.css`**

色板来源：rebelot/kanagawa.nvim `lua/kanagawa/colors.lua` + `themes.lua`（wave 变体）。

```css
/* mdview builtin scheme: kanagawa */
body { color: #dcd7ba; background-color: #1f1f28; }
h1 { color: #e46876; font-weight: bold; border-color: #e46876; }
h2 { color: #ffa066; font-weight: bold; border-color: #ffa066; }
h3 { color: #957fb8; font-weight: bold; }
h4 { color: #98bb6c; font-weight: bold; }
h5 { color: #e6c384; font-weight: bold; }
h6 { color: #727169; font-weight: bold; }
p { color: #dcd7ba; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #727169; text-decoration: line-through; }
code { color: #e6c384; background-color: #2a2a37; }
pre { color: #dcd7ba; background-color: #2a2a37; }
a { color: #7e9cd8; text-decoration: underline; }
blockquote { color: #c8c093; border-color: #54546d; background: #2a2a37; }
li { color: #7e9cd8; }
table, th, td { color: #dcd7ba; border-color: #54546d; }
th { font-weight: bold; }
hr { border-color: #54546d; }
img { color: #727169; }
math { color: #d27e99; font-style: italic; }
footnote { color: #727169; }
cursor { background-color: #363646; }
syntax-keyword { color: #957fb8; }
syntax-string { color: #98bb6c; }
syntax-comment { color: #727169; font-style: italic; }
syntax-function { color: #7e9cd8; }
syntax-type { color: #7aa89f; }
syntax-number { color: #d27e99; }
syntax-operator { color: #c0a36e; }
```

- [ ] **Step 3: 创建 `assets/styles/rose-pine.css`**

色板来源：rose-pine/palette palette.json（main 变体）+ rose-pine/neovim 官方语法映射（keyword=iris, string=gold, function=rose, type=foam, number=love, operator=subtle）。

```css
/* mdview builtin scheme: rose-pine */
body { color: #e0def4; background-color: #191724; }
h1 { color: #eb6f92; font-weight: bold; border-color: #eb6f92; }
h2 { color: #f6c177; font-weight: bold; border-color: #f6c177; }
h3 { color: #c4a7e7; font-weight: bold; }
h4 { color: #9ccfd8; font-weight: bold; }
h5 { color: #ebbcba; font-weight: bold; }
h6 { color: #6e6a86; font-weight: bold; }
p { color: #e0def4; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #6e6a86; text-decoration: line-through; }
code { color: #ebbcba; background-color: #1f1d2e; }
pre { color: #e0def4; background-color: #1f1d2e; }
a { color: #9ccfd8; text-decoration: underline; }
blockquote { color: #908caa; border-color: #403d52; background: #1f1d2e; }
li { color: #eb6f92; }
table, th, td { color: #e0def4; border-color: #403d52; }
th { font-weight: bold; }
hr { border-color: #403d52; }
img { color: #6e6a86; }
math { color: #c4a7e7; font-style: italic; }
footnote { color: #6e6a86; }
cursor { background-color: #21202e; }
syntax-keyword { color: #c4a7e7; }
syntax-string { color: #f6c177; }
syntax-comment { color: #6e6a86; font-style: italic; }
syntax-function { color: #ebbcba; }
syntax-type { color: #9ccfd8; }
syntax-number { color: #eb6f92; }
syntax-operator { color: #908caa; }
```

- [ ] **Step 4: 创建 `assets/styles/everforest.css`**

色板来源：sainnhe/everforest `palette.md`（dark, medium 对比度；keyword=red, operator=orange, string/function=green, type=yellow, number=purple, comment=grey1）。

```css
/* mdview builtin scheme: everforest */
body { color: #d3c6aa; background-color: #2d353b; }
h1 { color: #e67e80; font-weight: bold; border-color: #e67e80; }
h2 { color: #e69875; font-weight: bold; border-color: #e69875; }
h3 { color: #d699b6; font-weight: bold; }
h4 { color: #a7c080; font-weight: bold; }
h5 { color: #dbbc7f; font-weight: bold; }
h6 { color: #7a8478; font-weight: bold; }
p { color: #d3c6aa; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #7a8478; text-decoration: line-through; }
code { color: #dbbc7f; background-color: #343f44; }
pre { color: #d3c6aa; background-color: #343f44; }
a { color: #7fbbb3; text-decoration: underline; }
blockquote { color: #9da9a0; border-color: #4f585e; background: #343f44; }
li { color: #e69875; }
table, th, td { color: #d3c6aa; border-color: #4f585e; }
th { font-weight: bold; }
hr { border-color: #4f585e; }
img { color: #7a8478; }
math { color: #d699b6; font-style: italic; }
footnote { color: #7a8478; }
cursor { background-color: #3d484d; }
syntax-keyword { color: #e67e80; }
syntax-string { color: #a7c080; }
syntax-comment { color: #859289; font-style: italic; }
syntax-function { color: #a7c080; }
syntax-type { color: #dbbc7f; }
syntax-number { color: #d699b6; }
syntax-operator { color: #e69875; }
```

- [ ] **Step 5: 注册 4 个主题**

修改 `src/style/scheme.rs` 的 `BUILTINS`，在 `solarized-dark` 条目后插入：

```rust
    ("catppuccin-mocha", include_str!("../../assets/styles/catppuccin-mocha.css")),
    ("kanagawa", include_str!("../../assets/styles/kanagawa.css")),
    ("rose-pine", include_str!("../../assets/styles/rose-pine.css")),
    ("everforest", include_str!("../../assets/styles/everforest.css")),
```

- [ ] **Step 6: 运行测试**

Run: `cmd //c ".cargo-vc.bat test style::scheme"`
Expected: 全部 PASS（`new_builtin_schemes_registered` 仍 FAIL，因为还差 8 个——属预期；`builtin_schemes_cover_template_elements` 现在覆盖到 12 个主题，PASS）。

- [ ] **Step 7: 提交**

```bash
git add assets/styles/catppuccin-mocha.css assets/styles/kanagawa.css assets/styles/rose-pine.css assets/styles/everforest.css src/style/scheme.rs
git commit -m "✨ feat(themes): add 4 dark builtin schemes (catppuccin-mocha, kanagawa, rose-pine, everforest)"
```

---

## Task 3: 暗色批次 2（one-dark / monokai / ayu-dark / github-dark）

**Files:**
- Create: `assets/styles/one-dark.css`
- Create: `assets/styles/monokai.css`
- Create: `assets/styles/ayu-dark.css`
- Create: `assets/styles/github-dark.css`
- Modify: `src/style/scheme.rs`（`BUILTINS` 中 `everforest` 条目之后插入 4 条注册）

- [ ] **Step 1: 创建 `assets/styles/one-dark.css`**

色板来源：atom/one-dark-syntax `styles/colors.less`（hue-5 红取 Atom 原版 `#e06c75`）+ navarasu/onedark.nvim palette（bg1/bg2/light_grey）。光标行 `#2c313a` 为 Atom 官方 cursor-line 推导值（无命名色板条目）。

```css
/* mdview builtin scheme: one-dark */
body { color: #abb2bf; background-color: #282c34; }
h1 { color: #e06c75; font-weight: bold; border-color: #e06c75; }
h2 { color: #d19a66; font-weight: bold; border-color: #d19a66; }
h3 { color: #c678dd; font-weight: bold; }
h4 { color: #98c379; font-weight: bold; }
h5 { color: #e5c07b; font-weight: bold; }
h6 { color: #5c6370; font-weight: bold; }
p { color: #abb2bf; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #5c6370; text-decoration: line-through; }
code { color: #e5c07b; background-color: #31353f; }
pre { color: #abb2bf; background-color: #31353f; }
a { color: #61afef; text-decoration: underline; }
blockquote { color: #848b98; border-color: #393f4a; background: #31353f; }
li { color: #61afef; }
table, th, td { color: #abb2bf; border-color: #393f4a; }
th { font-weight: bold; }
hr { border-color: #393f4a; }
img { color: #5c6370; }
math { color: #c678dd; font-style: italic; }
footnote { color: #5c6370; }
cursor { background-color: #2c313a; }
syntax-keyword { color: #c678dd; }
syntax-string { color: #98c379; }
syntax-comment { color: #5c6370; font-style: italic; }
syntax-function { color: #61afef; }
syntax-type { color: #e5c07b; }
syntax-number { color: #d19a66; }
syntax-operator { color: #56b6c2; }
```

- [ ] **Step 2: 创建 `assets/styles/monokai.css`**

色板来源：microsoft/vscode 官方 Monokai 移植（`extensions/theme-monokai`）；`syntax-comment` 取 Sublime 原版 `#75715e`，`blockquote` 取 VS Code 调亮版 `#88846f` 以保证终端可读性。

```css
/* mdview builtin scheme: monokai */
body { color: #f8f8f2; background-color: #272822; }
h1 { color: #f92672; font-weight: bold; border-color: #f92672; }
h2 { color: #fd971f; font-weight: bold; border-color: #fd971f; }
h3 { color: #ae81ff; font-weight: bold; }
h4 { color: #a6e22e; font-weight: bold; }
h5 { color: #e6db74; font-weight: bold; }
h6 { color: #75715e; font-weight: bold; }
p { color: #f8f8f2; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #75715e; text-decoration: line-through; }
code { color: #e6db74; background-color: #3e3d32; }
pre { color: #f8f8f2; background-color: #3e3d32; }
a { color: #66d9ef; text-decoration: underline; }
blockquote { color: #88846f; border-color: #414339; background: #3e3d32; }
li { color: #fd971f; }
table, th, td { color: #f8f8f2; border-color: #414339; }
th { font-weight: bold; }
hr { border-color: #414339; }
img { color: #75715e; }
math { color: #ae81ff; font-style: italic; }
footnote { color: #75715e; }
cursor { background-color: #3e3d32; }
syntax-keyword { color: #f92672; }
syntax-string { color: #e6db74; }
syntax-comment { color: #75715e; font-style: italic; }
syntax-function { color: #a6e22e; }
syntax-type { color: #66d9ef; }
syntax-number { color: #ae81ff; }
syntax-operator { color: #fd971f; }
```

- [ ] **Step 3: 创建 `assets/styles/ayu-dark.css`**

色板来源：ayu-theme/ayu-colors `themes/dark.yaml` + `palette.svg`（步进色已解析为最终 hex）。边框取官方 `ui.line`，整体偏素是 ayu 的固有风格。

```css
/* mdview builtin scheme: ayu-dark */
body { color: #bfbdb6; background-color: #0d1017; }
h1 { color: #f07178; font-weight: bold; border-color: #f07178; }
h2 { color: #ff8f40; font-weight: bold; border-color: #ff8f40; }
h3 { color: #d2a6ff; font-weight: bold; }
h4 { color: #aad94c; font-weight: bold; }
h5 { color: #e6b450; font-weight: bold; }
h6 { color: #5a6378; font-weight: bold; }
p { color: #bfbdb6; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #5a6378; text-decoration: line-through; }
code { color: #e6b450; background-color: #10141c; }
pre { color: #bfbdb6; background-color: #10141c; }
a { color: #59c2ff; text-decoration: underline; }
blockquote { color: #5a6378; border-color: #1b1f29; background: #10141c; }
li { color: #ff8f40; }
table, th, td { color: #bfbdb6; border-color: #1b1f29; }
th { font-weight: bold; }
hr { border-color: #1b1f29; }
img { color: #5a6378; }
math { color: #d2a6ff; font-style: italic; }
footnote { color: #5a6378; }
cursor { background-color: #161a24; }
syntax-keyword { color: #ff8f40; }
syntax-string { color: #aad94c; }
syntax-comment { color: #5a6673; font-style: italic; }
syntax-function { color: #ffb454; }
syntax-type { color: #59c2ff; }
syntax-number { color: #d2a6ff; }
syntax-operator { color: #f29668; }
```

- [ ] **Step 4: 创建 `assets/styles/github-dark.css`**

色板来源：primer/primitives 现行官方 dark 令牌（`fgColor-default #f0f6fc`、`bgColor-muted #151b23`、`borderColor-default #3d444d`、prettylights syntax 系列）。h1/h2 与 `github-light` 配对（正文色标题），但边框取 GitHub 实际渲染的标题分隔线色 `#3d444d` 而非纯白。

```css
/* mdview builtin scheme: github-dark */
body { color: #f0f6fc; background-color: #0d1117; }
h1 { color: #f0f6fc; font-weight: bold; border-color: #3d444d; }
h2 { color: #f0f6fc; font-weight: bold; border-color: #3d444d; }
h3 { color: #4493f8; font-weight: bold; }
h4 { color: #3fb950; font-weight: bold; }
h5 { color: #ffa657; font-weight: bold; }
h6 { color: #9198a1; font-weight: bold; }
p { color: #f0f6fc; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #9198a1; text-decoration: line-through; }
code { color: #f85149; background-color: #151b23; }
pre { color: #f0f6fc; background-color: #151b23; }
a { color: #4493f8; text-decoration: underline; }
blockquote { color: #9198a1; border-color: #3d444d; background: #151b23; }
li { color: #4493f8; }
table, th, td { color: #f0f6fc; border-color: #3d444d; }
th { font-weight: bold; }
hr { border-color: #3d444d; }
img { color: #9198a1; }
math { color: #ab7df8; font-style: italic; }
footnote { color: #9198a1; }
cursor { background-color: #262c36; }
syntax-keyword { color: #ff7b72; }
syntax-string { color: #a5d6ff; }
syntax-comment { color: #9198a1; font-style: italic; }
syntax-function { color: #d2a8ff; }
syntax-type { color: #7ee787; }
syntax-number { color: #79c0ff; }
syntax-operator { color: #f0f6fc; }
```

- [ ] **Step 5: 注册 4 个主题**

修改 `src/style/scheme.rs` 的 `BUILTINS`，在 `everforest` 条目后插入：

```rust
    ("one-dark", include_str!("../../assets/styles/one-dark.css")),
    ("monokai", include_str!("../../assets/styles/monokai.css")),
    ("ayu-dark", include_str!("../../assets/styles/ayu-dark.css")),
    ("github-dark", include_str!("../../assets/styles/github-dark.css")),
```

- [ ] **Step 6: 运行测试**

Run: `cmd //c ".cargo-vc.bat test style::scheme"`
Expected: `builtin_schemes_cover_template_elements` PASS（16 个主题）；`new_builtin_schemes_registered` 仍 FAIL（差 4 个亮色，属预期）。

- [ ] **Step 7: 提交**

```bash
git add assets/styles/one-dark.css assets/styles/monokai.css assets/styles/ayu-dark.css assets/styles/github-dark.css src/style/scheme.rs
git commit -m "✨ feat(themes): add 4 dark builtin schemes (one-dark, monokai, ayu-dark, github-dark)"
```

---

## Task 4: 亮色批次（catppuccin-latte / rose-pine-dawn / everforest-light / ayu-light）

**Files:**
- Create: `assets/styles/catppuccin-latte.css`
- Create: `assets/styles/rose-pine-dawn.css`
- Create: `assets/styles/everforest-light.css`
- Create: `assets/styles/ayu-light.css`
- Modify: `src/style/scheme.rs`（`BUILTINS` 中 `gruvbox-light` 条目之后插入 4 条注册）

- [ ] **Step 1: 创建 `assets/styles/catppuccin-latte.css`**

色板来源：catppuccin/palette v1.8.0 palette.json（latte）。`cursor` 取官方 mantle（比 base 略暗）。

```css
/* mdview builtin scheme: catppuccin-latte */
body { color: #4c4f69; background-color: #eff1f5; }
h1 { color: #d20f39; font-weight: bold; border-color: #d20f39; }
h2 { color: #fe640b; font-weight: bold; border-color: #fe640b; }
h3 { color: #8839ef; font-weight: bold; }
h4 { color: #40a02b; font-weight: bold; }
h5 { color: #df8e1d; font-weight: bold; }
h6 { color: #7c7f93; font-weight: bold; }
p { color: #4c4f69; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #7c7f93; text-decoration: line-through; }
code { color: #fe640b; background-color: #ccd0da; }
pre { color: #4c4f69; background-color: #ccd0da; }
a { color: #1e66f5; text-decoration: underline; }
blockquote { color: #6c6f85; border-color: #acb0be; background: #ccd0da; }
li { color: #1e66f5; }
table, th, td { color: #4c4f69; border-color: #acb0be; }
th { font-weight: bold; }
hr { border-color: #acb0be; }
img { color: #7c7f93; }
math { color: #ea76cb; font-style: italic; }
footnote { color: #7c7f93; }
cursor { background-color: #e6e9ef; }
syntax-keyword { color: #8839ef; }
syntax-string { color: #40a02b; }
syntax-comment { color: #7c7f93; font-style: italic; }
syntax-function { color: #1e66f5; }
syntax-type { color: #df8e1d; }
syntax-number { color: #fe640b; }
syntax-operator { color: #04a5e5; }
```

- [ ] **Step 2: 创建 `assets/styles/rose-pine-dawn.css`**

色板来源：rose-pine/palette palette.json（dawn 变体，text 为新版 `#464261`）+ rose-pine/neovim 官方语法映射。`cursor` 取 highlight_low（比 base 略暗）。

```css
/* mdview builtin scheme: rose-pine-dawn */
body { color: #464261; background-color: #faf4ed; }
h1 { color: #b4637a; font-weight: bold; border-color: #b4637a; }
h2 { color: #ea9d34; font-weight: bold; border-color: #ea9d34; }
h3 { color: #907aa9; font-weight: bold; }
h4 { color: #56949f; font-weight: bold; }
h5 { color: #d7827e; font-weight: bold; }
h6 { color: #9893a5; font-weight: bold; }
p { color: #464261; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #9893a5; text-decoration: line-through; }
code { color: #d7827e; background-color: #fffaf3; }
pre { color: #464261; background-color: #fffaf3; }
a { color: #286983; text-decoration: underline; }
blockquote { color: #797593; border-color: #dfdad9; background: #fffaf3; }
li { color: #b4637a; }
table, th, td { color: #464261; border-color: #dfdad9; }
th { font-weight: bold; }
hr { border-color: #dfdad9; }
img { color: #9893a5; }
math { color: #907aa9; font-style: italic; }
footnote { color: #9893a5; }
cursor { background-color: #f4ede8; }
syntax-keyword { color: #907aa9; }
syntax-string { color: #ea9d34; }
syntax-comment { color: #9893a5; font-style: italic; }
syntax-function { color: #d7827e; }
syntax-type { color: #56949f; }
syntax-number { color: #b4637a; }
syntax-operator { color: #797593; }
```

- [ ] **Step 3: 创建 `assets/styles/everforest-light.css`**

色板来源：sainnhe/everforest `palette.md`（light, medium 对比度）。`cursor` 取 bg_dim/bg2（比 base 略暗）。

```css
/* mdview builtin scheme: everforest-light */
body { color: #5c6a72; background-color: #fdf6e3; }
h1 { color: #f85552; font-weight: bold; border-color: #f85552; }
h2 { color: #f57d26; font-weight: bold; border-color: #f57d26; }
h3 { color: #df69ba; font-weight: bold; }
h4 { color: #8da101; font-weight: bold; }
h5 { color: #dfa000; font-weight: bold; }
h6 { color: #a6b0a0; font-weight: bold; }
p { color: #5c6a72; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #a6b0a0; text-decoration: line-through; }
code { color: #dfa000; background-color: #f4f0d9; }
pre { color: #5c6a72; background-color: #f4f0d9; }
a { color: #3a94c5; text-decoration: underline; }
blockquote { color: #829181; border-color: #e0dcc7; background: #f4f0d9; }
li { color: #f57d26; }
table, th, td { color: #5c6a72; border-color: #e0dcc7; }
th { font-weight: bold; }
hr { border-color: #e0dcc7; }
img { color: #a6b0a0; }
math { color: #df69ba; font-style: italic; }
footnote { color: #a6b0a0; }
cursor { background-color: #efebd4; }
syntax-keyword { color: #f85552; }
syntax-string { color: #8da101; }
syntax-comment { color: #939f91; font-style: italic; }
syntax-function { color: #8da101; }
syntax-type { color: #dfa000; }
syntax-number { color: #df69ba; }
syntax-operator { color: #f57d26; }
```

- [ ] **Step 4: 创建 `assets/styles/ayu-light.css`**

色板来源：ayu-theme/ayu-colors `themes/light.yaml`。`cursor`（`#f0f1f3`）与边框（`#e7eaed`）为官方 alpha 定义按底色的推算值。

```css
/* mdview builtin scheme: ayu-light */
body { color: #5c6166; background-color: #f8f9fa; }
h1 { color: #f07171; font-weight: bold; border-color: #f07171; }
h2 { color: #fa8532; font-weight: bold; border-color: #fa8532; }
h3 { color: #a37acc; font-weight: bold; }
h4 { color: #86b300; font-weight: bold; }
h5 { color: #eba400; font-weight: bold; }
h6 { color: #828e9f; font-weight: bold; }
p { color: #5c6166; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #828e9f; text-decoration: line-through; }
code { color: #eba400; background-color: #ebeef0; }
pre { color: #5c6166; background-color: #ebeef0; }
a { color: #22a4e6; text-decoration: underline; }
blockquote { color: #828e9f; border-color: #e7eaed; background: #ebeef0; }
li { color: #fa8532; }
table, th, td { color: #5c6166; border-color: #e7eaed; }
th { font-weight: bold; }
hr { border-color: #e7eaed; }
img { color: #828e9f; }
math { color: #a37acc; font-style: italic; }
footnote { color: #828e9f; }
cursor { background-color: #f0f1f3; }
syntax-keyword { color: #fa8532; }
syntax-string { color: #86b300; }
syntax-comment { color: #adaeb1; font-style: italic; }
syntax-function { color: #eba400; }
syntax-type { color: #22a4e6; }
syntax-number { color: #a37acc; }
syntax-operator { color: #f2a191; }
```

- [ ] **Step 5: 注册 4 个主题**

修改 `src/style/scheme.rs` 的 `BUILTINS`，在 `gruvbox-light` 条目后插入：

```rust
    ("catppuccin-latte", include_str!("../../assets/styles/catppuccin-latte.css")),
    ("rose-pine-dawn", include_str!("../../assets/styles/rose-pine-dawn.css")),
    ("everforest-light", include_str!("../../assets/styles/everforest-light.css")),
    ("ayu-light", include_str!("../../assets/styles/ayu-light.css")),
```

- [ ] **Step 6: 运行全量测试（TDD Green）**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS、零警告。`new_builtin_schemes_registered` 与 `builtin_schemes_cover_template_elements` 均 PASS（20 个主题全覆盖）。

- [ ] **Step 7: 提交**

```bash
git add assets/styles/catppuccin-latte.css assets/styles/rose-pine-dawn.css assets/styles/everforest-light.css assets/styles/ayu-light.css src/style/scheme.rs
git commit -m "✨ feat(themes): add 4 light builtin schemes (catppuccin-latte, rose-pine-dawn, everforest-light, ayu-light)"
```

---

## Task 5: 文档同步（AGENTS.md + README × 2）

**Files:**
- Modify: `AGENTS.md`（"8 builtin themes" → "20 builtin themes"）
- Modify: `README.md:12` 和 `README.md:37`
- Modify: `README.zh-CN.md:12` 和 `README.zh-CN.md:60`

- [ ] **Step 1: 更新 `AGENTS.md`**

把 `- assets/styles/ — 8 builtin themes (embedded via include_str!)` 改为：

```
- `assets/styles/` — 20 builtin themes (embedded via `include_str!`)
```

（保持该行原有的反引号格式，只改数字 8 → 20。）

- [ ] **Step 2: 更新 `README.md`**

第 12 行 `- 8 builtin themes + custom CSS themes; picker selection persists` 改为：

```
- 20 builtin themes + custom CSS themes; picker selection persists
```

第 37 行内置主题清单改为（默认主题标注保持不变）：

```
Builtins: `tokyo-night`, `dracula`, `gruvbox-dark` (default), `gruvbox-light`, `nord`, `solarized-dark`, `solarized-light`, `github-light`, `github-dark`, `catppuccin-mocha`, `catppuccin-latte`, `kanagawa`, `rose-pine`, `rose-pine-dawn`, `everforest`, `everforest-light`, `one-dark`, `monokai`, `ayu-dark`, `ayu-light`.
```

- [ ] **Step 3: 更新 `README.zh-CN.md`**

第 12 行 `- **主题系统**：8 个内置主题 + 自定义 CSS 主题；选择器选定的主题自动持久化` 改为：

```
- **主题系统**：20 个内置主题 + 自定义 CSS 主题；选择器选定的主题自动持久化
```

第 60 行内置主题清单改为：

```
内置主题：`tokyo-night`、`dracula`、`gruvbox-dark`（默认）、`gruvbox-light`、`nord`、`solarized-dark`、`solarized-light`、`github-light`、`github-dark`、`catppuccin-mocha`、`catppuccin-latte`、`kanagawa`、`rose-pine`、`rose-pine-dawn`、`everforest`、`everforest-light`、`one-dark`、`monokai`、`ayu-dark`、`ayu-light`。
```

- [ ] **Step 4: 提交**

```bash
git add AGENTS.md README.md README.zh-CN.md
git commit -m "📝 docs(themes): update builtin theme count and list to 20"
```

---

## Task 6: 最终验证

**Files:** 无（只读验证）

- [ ] **Step 1: 全量测试 + 零警告构建**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全部 PASS，无 warning。

- [ ] **Step 2: 确认发行版打包无需改动**

`bin/md-styles/` 不存在于仓库（`build.bat` 组装的是用户自定义主题目录），内置主题通过 `include_str!` 嵌入，无需改 `build.bat`。确认 `git status` 干净、所有改动已在 Task 2–5 提交。

- [ ] **Step 3: 抽查一个新主题实际加载**

Run: `cmd //c ".cargo-vc.bat run -- --list-themes"`
Expected: 输出包含全部 20 个主题名（字母序），含 `catppuccin-mocha`、`ayu-light` 等新主题。

---

## Self-Review 记录

- **规格覆盖：** 12 个新主题（Task 2–4 各 4 个，CSS 全文已给出）✓；注册（各批次 Step 5）✓；TDD 测试（Task 1）✓；默认主题不变（未触碰 `DEFAULT_THEME`）✓；文档三处同步（Task 5）✓；全量测试零警告（Task 4 Step 6 + Task 6）✓。
- **占位符扫描：** 无 TBD/TODO；每个 CSS 文件 31 行完整给出；测试代码完整。
- **一致性：** 主题名在测试数组、`BUILTINS` 注册、文件名、README 清单中完全一致；12 个 CSS 文件均含 `cursor` 背景（满足既有测试 `builtin_schemes_define_cursor_background`）与全部 7 个 `syntax-*` 类。
