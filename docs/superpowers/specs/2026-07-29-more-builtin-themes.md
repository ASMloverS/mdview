# 新增 12 个内置主题设计规格

日期：2026-07-29
状态：待确认
对应计划：`docs/superpowers/plans/2026-07-29-more-builtin-themes.md`

## 背景

mdview 现有 8 个内置主题（5 暗 3 亮）：tokyo-night、dracula、gruvbox-dark、gruvbox-light、nord、solarized-dark、solarized-light、github-light。整体偏"经典程序员主题"，缺少近年流行的新主题（catppuccin、kanagawa、rose-pine 等），亮色选择也偏少。

机制现状：

- 内置主题以 `include_str!` 嵌入二进制，注册在 `src/style/scheme.rs` 的 `BUILTINS` 数组
- 全部 8 个主题严格遵循同一 31 行 CSS 模板：同一组元素（body / h1-h6 / p / strong / em / del / code / pre / a / blockquote / li / table / th / hr / img / math / footnote / cursor）+ 7 个 `syntax-*` 语法高亮类，仅色值不同
- 用户可在 exe 旁 `md-styles/<名称>.css` 放同名文件覆盖内置主题
- 既有测试约束：每个内置主题必须解析出非空规则、必须定义 `cursor` 背景色

## 目标

新增 12 个内置主题，内置总数从 8 扩到 20：

| 类型 | 主题 |
|---|---|
| 暗色（8） | catppuccin-mocha、kanagawa、rose-pine、everforest、one-dark、monokai、ayu-dark、github-dark |
| 亮色（4） | catppuccin-latte、rose-pine-dawn、everforest-light、ayu-light |

选择理由：

- catppuccin、kanagawa、rose-pine、everforest：近年社区最流行的新主题，各带官方亮色变体（kanagawa 官方亮色 dragon 版色板不完整，故只做暗色；缺口由 ayu-light 补足）
- one-dark、monokai：Atom / Sublime 时代经典，补齐"编辑器经典"维度
- ayu-dark / ayu-light：现代简约风格，且是少数官方亮暗双全的主题
- github-dark：与现有 github-light 配对

## 非目标

- 不改 CSS 子集、不新增样式属性、不改渲染/布局/TUI 代码
- 不改默认主题（保持 `gruvbox-dark`）
- 不调整现有 8 个主题的任何色值
- 不做主题预览图、不做按终端背景自动选主题等额外功能

## 设计决策

### 1. 严格复用 31 行模板

新主题不引入任何新元素，元素集合、行序、规则结构与现有主题逐行对齐，仅填色值。收益：

- 与 `style_for` 的选择器匹配行为完全一致，无意外样式缺失
- diff 易审查（可与 gruvbox-dark.css 逐行对照）
- 后续维护（新增元素时）可批量同步

### 2. 色值全部取自官方调色板

已联网核实的官方来源：

| 主题 | 来源 |
|---|---|
| catppuccin-mocha / latte | catppuccin/palette `palette.json` v1.8.0 |
| kanagawa | rebelot/kanagawa.nvim `lua/kanagawa/colors.lua` + `themes.lua`（wave 变体） |
| rose-pine / dawn | rose-pine/palette `palette.json` + rose-pine/neovim 官方语法映射 |
| everforest（暗/亮） | sainnhe/everforest `palette.md`（medium 对比度） |
| one-dark | atom/one-dark-syntax `styles/colors.less` + navarasu/onedark.nvim palette |
| monokai | microsoft/vscode 官方 Monokai 移植（`extensions/theme-monokai`） |
| ayu-dark / ayu-light | ayu-theme/ayu-colors `themes/{dark,light}.yaml` + `palette.svg` |
| github-dark | primer/primitives 现行 dark 令牌 + prettylights syntax |

官方色板无对应角色的少数位置（如 one-dark 光标行、ayu-light 边框），采用官方定义按底色推算的值，并在计划对应文件注释中标注。monokai 的 `syntax-comment` 取 Sublime 原版 `#75715e`（VS Code 调亮版 `#88846f` 仅用于 blockquote 保证终端可读性）。

### 3. 元素→色板映射约定（沿用现有主题惯例）

- `h1`=红/粉、`h2`=橙（均带同色边框）、`h3`=紫、`h4`=绿/青、`h5`=黄、`h6`=muted 灰
- `del`/`img`/`footnote`：muted 灰；`code`：强调色 + surface 背景；`pre`：正文色 + surface 背景
- `a`：蓝/青 + underline；`blockquote`：比正文暗一档的次要文字色
- `li`：强调色；`table`/`hr`：边框色；`math`：紫/粉 + italic
- `cursor`：官方光标行/高亮行背景（暗主题下比主背景略亮，亮主题下略暗）
- `syntax-*`：按各主题官方语法高亮语义映射（如 kanagawa keyword=oniViolet、rose-pine string=gold、everforest keyword=red）

例外说明：github-dark 的 h1/h2 与 github-light 配对（正文色标题），但标题边框取 GitHub 实际渲染的分隔线色 `#3d444d` 而非纯白。

### 4. 注册与排列

`BUILTINS` 数组保持"暗色在前、亮色在后"的现有结构：新暗色插入 `solarized-dark` 之后，新亮色追加在 `gruvbox-light` 之后。主题选择器本身按字母序展示，注册顺序仅为代码可读性。

### 5. 测试策略（TDD）

新增两个测试（`src/style/scheme.rs` 的 `mod tests`）：

- `new_builtin_schemes_registered`：断言 12 个新名称全部注册（先写测试确认失败 → 逐批次实现 → 最终转绿）
- `builtin_schemes_cover_template_elements`：断言**每个**内置主题（含现有 8 个）覆盖全部 24 个模板元素与 7 个 syntax 类——防止今后新增主题漏项

既有测试 `builtin_schemes_parse`、`builtin_schemes_define_cursor_background` 自动覆盖新主题。

### 6. 文档同步

- `AGENTS.md`："8 builtin themes" → "20 builtin themes"
- `README.md`：特性行数量 + Themes 节内置清单
- `README.zh-CN.md`：对应两处中文描述

## 验证标准

1. `cmd //c ".cargo-vc.bat test"` 全量通过、零警告
2. `mdview --list-themes` 输出 20 个主题，含全部 12 个新名称
3. 新主题文件与现有模板逐行结构对齐（24 元素 + 7 syntax 类 + cursor）
