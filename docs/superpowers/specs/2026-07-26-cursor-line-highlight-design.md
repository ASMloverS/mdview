# mdview 阅读器光标行高亮 — 设计文档

日期：2026-07-26
状态：已确认

## 目标

阅读模式下，用 `j/k`（及其他移动键）移动时，高亮显示当前行（整行），
类似 vim 的 `cursorline`。

## 已确认的需求

- 独立光标行：移动键移动光标，滚动跟随以保证光标可见；不做「始终居中」。
- 高亮样式来自主题 CSS 元素（`cursor`），8 个内置主题均定义，用户主题可自定义。
- 所有移动操作都移动光标：`j/k`、方向键、`d/u`、`PgDn/PgUp`、`Ctrl+f/b`、
  `g/G`、搜索跳转 `n/N`。鼠标滚轮保持纯滚动，不移动光标。
- 高亮铺满整个可视区宽度（边框内侧），含内容居中后的两侧留白。
- 打开文件立即高亮第 0 行。

## 方案

采用：UI 态光标 + 绘制时打补丁（方案 A）。

否决：
- 缓冲区后处理（渲染后遍历 ratatui buffer 单元格）：单测困难，坐标换算
  依赖 `frame.buffer_mut()`。
- 布局管线感知光标：每次按键都整篇重排版，且把 UI 关注点泄进
  `render::layout`。

## 设计

### 1. 状态与移动（`src/app.rs`）

- `Reader` 新增 `cursor: usize`。`open_reader` 置 0；`reload_reader`
  clamp 到渲染行数范围内。
- 新增 `move_cursor(app, delta)`：
  - `cursor = (cursor + delta).clamp(0, lines.len() - 1)`
  - 滚动跟随：`cursor < scroll → scroll = cursor`；
    `cursor >= scroll + view_height → scroll = cursor - view_height + 1`。
- `reader_key` 按键映射改动：
  - `j/k/↓/↑` → `move_cursor(±1)`
  - `d/u/PgDn/PgUp` → `move_cursor(±半页)`
  - `Ctrl+f/b` → `move_cursor(±page_delta(view_height))`
  - `g/G` → 光标到首/末行
  - `n/N` → `jump_match` 把匹配行赋给 `cursor` 再滚动跟随
    （替代现在直接赋 `scroll` 的写法）
- 鼠标滚轮维持现有滚动逻辑，光标不动。
- `scroll_reader` 的键盘路径并入 `move_cursor`；纯滚动版本保留给鼠标。
- 现有 app 测试更新为新 cursor/scroll 语义的断言。

### 2. 高亮绘制（`src/ui/mod.rs`、`src/ui/reader.rs`）

- `src/ui/mod.rs` 新增 `cursor_style(app) -> Option<Style>`：读取
  `app.scheme.element("cursor")` 的 `bg`；主题未定义时返回 `None`，
  旧用户主题则不显示高亮。
- `reader::draw` 中 `convert()` 之后，若光标行在可视窗口内：
  - 把该 `Line` 自身的 style 设为光标背景色，由 ratatui 填充整行宽度
    （含两侧留白）；
  - 对行内每个 span `patch_style` 叠加光标背景色，保留原有前景色。
- 高亮逻辑抽成纯函数（如 `highlight_line(line, style)`）便于单测。

### 3. 主题（`assets/styles/*.css`）

- 8 个内置主题各加一条规则：`cursor { background-color: <颜色>; }`，
  一般取该主题现有 `code`/`pre` 底色，或比 `body` 略亮/略暗的颜色。
- CSS 子集与 `AGENTS.md` 约定无需改动。

### 4. 测试

- `app.rs`：j/k 移动光标；视口上下边缘滚动跟随；`g/G`；翻页 clamp；
  搜索跳转置光标；reload clamp 光标。
- `scheme.rs`：`cursor` 元素可从 CSS 解析。
- `reader` 高亮辅助函数：span 叠加背景、整行填充。

### 5. 范围外

- 搜索匹配项高亮（跳转已会把光标移过去）。
- 帮助浮层文案（`j/k` 描述仍然准确）。
- 文件浏览器模式的选中高亮（已有 `selected` 机制）。
