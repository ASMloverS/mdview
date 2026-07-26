# mdview 终端 Markdown 渲染器 — 设计文档

日期：2026-07-26
状态：已确认（方案 A：现有管线增量扩展）

## 背景与目标

mdview 是一个终端 Markdown 渲染器（Rust），交互形态参考 glow，渲染效果参考
VS Code 的 Markdown 预览。仓库中已有一套约 3200 行的未提交实现，本设计在其
基础上迭代，不重写。

核心需求：

- TUI 交互浏览为主（文件浏览器 + 阅读器：滚动、搜索、主题热切换）
- 自定义 colorscheme：CSS 文件，放在 `md-styles/*.css`
- 多个内置 scheme，默认支持语法高亮（syntect）
- 渲染装饰向 VS Code 预览看齐

已确认的关键决策：

- 基于现有代码迭代（方案 A），不重构渲染管线，不换现成库
- CSS 主题能力范围：颜色 + 字体样式（不新增布局类自定义属性）
- 装饰性增强为内置默认开启，不提供配置开关
- 主题目录查找：cwd 优先，其次 exe 旁目录

## 现状（baseline）

技术栈：Rust + ratatui/crossterm（TUI）、pulldown-cmark（解析）、syntect（高亮）。

已实现：CLI（`mdview <file>` / `-t theme` / `-w max_width` / `--list-themes` /
stdin 管道模式）、TUI 浏览器/阅读器/主题选择器、CSS 子集解析、8 个内置主题、
IR 中间层（标题/列表/表格/代码块/数学/脚注）、文末链接列表 + OSC8。

本次已修复的 baseline 问题：

- `.cargo-vc.bat` 中 vcvars64 路径被转义损坏（`\v` 变垂直制表符），导致
  MSVC 环境未加载、Git 的 `link.exe` 抢占链接器 → 已修复
- 5 个编译错误（highlight.rs `&str` 比较、app.rs 与 layout.rs 的借用冲突）
- `link_list_at_bottom` 测试失败（链接列表序号错误地独占一行）

Windows 构建说明：需通过 `.cargo-vc.bat` 调用 cargo（如
`cmd //c ".cargo-vc.bat build"` / `... test`）以获得正确的 MSVC 链接器环境。

## 架构

渲染管线维持四层不变：

```
.md 文件 → pulldown-cmark → markdown::ir::Document
         → render::layout（排版 + 装饰）→ Rendered { lines: Vec<SLine> }
         → render::ansi（管道模式）/ ui（ratatui TUI）
```

### 模块拆分（纯移动，不改行为）

`render/layout.rs`（733 行）拆为 `render/layout/` 模块：

- `mod.rs` — `Renderer` 结构体、行原语（`emit`/`flush_line`/`blank`）、
  `render_document` 入口
- `text.rs` — 段落/标题/列表的文本流与换行（`flatten`/`emit_wrapped`/
  tokenize）
- `block.rs` — 代码块、引用块、表格、水平线、数学块
- `decorate.rs` — 新增：所有装饰原语（标题下划线、代码块 chrome、居中偏移、
  背景填充条），供 text/block 调用

现有测试跟随移动到对应模块。

## 装饰规格（VS Code 风格，内置默认开启）

颜色一律取自当前 scheme 的已有 CSS 属性，不新增 CSS 属性。

### 标题

- `h1`：文本下一整行 `═`（宽度 = 内容宽度），颜色取 `h1 { border-color }`，
  缺省回落到 `h1` 的 `color`
- `h2`：下方一行 `─`，颜色规则同上
- `h3`–`h6`：仅颜色 + 加粗（维持现状），不加线

### 代码块

- 首行右侧渲染语言标签（如 `rust`），dim 色（取 `footnote` 颜色），无语言
  时不渲染
- 每行行首右对齐行号（dim 色）+ `│` 分隔；行号列宽按总行数计算
- 整行（含行号、填充到内容宽度的空白）涂 `pre` 的 `background`
- 代码块上下各留一个空行（现状保留）

### 引用块

- 左侧 `▎` 竖线，颜色取 `blockquote { border-color }`
- 引用内文本背景涂 `blockquote` 的 `background`（若定义）
- 多行引用连续成块

### 表格

- 维持现有 Unicode 边框
- 表头与表体之间用 `╞═╪═╡` 双线分隔行
- 边框颜色取 `table { border-color }`

### 排版

- 内容限宽后水平居中：`offset = (终端宽 - 内容宽) / 2`，作为所有行的统一左
  偏移，管道模式同样生效
- 段落间距、列表缩进维持现状

### 链接

维持现状：正文内 `[n]` 引用 + 文末链接列表 + OSC8 可点击。

## 主题系统

### 查找顺序（同名前者优先）

1. `./md-styles/<name>.css`（cwd）
2. `<exe 所在目录>/md-styles/<name>.css`（`std::env::current_exe` 推导）
3. 内置 scheme
4. 兜底 `tokyo-night`

`Scheme::available()`（`--list-themes` 与主题选择器）合并三个来源去重。
`config.toml` 的 `theme` 字段与 `-t` 参数逻辑不变。

### 内置主题更新

8 个内置 CSS（tokyo-night、dracula、gruvbox-dark/light、nord、
solarized-dark/light、github-light）各补上：

- `h1`/`h2` 的 `border-color`（缺省等于各自文字色）
- `blockquote` 的 `background`（暗色主题用略亮于 body 的背景，浅色主题用
  略深于 body 的背景）

## 测试策略

沿用现有 `cargo test` 单元测试风格（`render() → plain` 文本断言）：

- 标题：h1 后一行全为 `═`；h2 后一行全为 `─`；h3 无线
- 代码块：语言标签出现在首行；行号右对齐；行号数量与代码行数一致
- 引用块：连续输出行均以 `▎` 起始
- 表格：表头下出现 `╞` 起始的双线分隔行
- 居中：输出行首出现预期偏移量的前导空格
- 主题查找：用临时目录模拟 cwd/exe 目录的优先级与同名覆盖

## 范围之外（YAGNI）

- 图片渲染（sixel/kitty 协议）
- CSS 布局类属性（padding/margin/class 选择器等）
- 装饰项的 config.toml 开关
- 文件变更监听自动重载
