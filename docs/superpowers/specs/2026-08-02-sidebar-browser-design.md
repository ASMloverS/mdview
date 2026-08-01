# 目录侧栏（Sidebar）设计

日期：2026-08-02
状态：已确认
前作：2026-08-01-directory-browser-design.md（目录树浏览器，本次迭代其交互模型）

## 背景与目标

上一版把目录树浏览器作为独立主视图（`Mode::Browser`），阅读器 Esc 返回浏览器。
本次迭代改为**阅读器为主 + 可开关的目录侧栏**：`o` 唤出侧栏，`Enter` 确认
（进目录 / 打开文件），`Backspace` 上级，打开文件后侧栏关闭直接渲染文档。
侧栏与阅读器可共存（覆盖式双栏），焦点可切换。

## 需求决策（已与用户确认）

- 布局：覆盖式双栏——侧栏在左（默认 30%），右侧始终显示阅读器内容；
  侧栏不做文件预览，右侧不随侧栏选中变化。
- 侧栏宽度可通过 `config.toml` 的 `sidebar_width` 配置（百分比，默认 30，
  clamp 10..=60）。
- 启动：有文件（CLI/history 恢复）→ 阅读器，侧栏关；无文件 → `reader = None`
  + 自动开侧栏（焦点在侧栏），右侧显示提示文本。`resume_hint` 欢迎弹窗删除。
- 关闭侧栏：`Esc`（仅侧栏焦点）/ `q`（侧栏焦点）/ `Enter` 打开文件；
  `o` 只开不关（不是 toggle）。
- `q` 随焦点：侧栏焦点 → 关侧栏；阅读器焦点 → 退出程序（侧栏开时也直接退出）。
- 焦点：`Tab` 在侧栏 ↔ 阅读器间切换；侧栏打开时默认焦点在侧栏。
  键盘输入按焦点分派（非独占）。
- 上级目录：仅 `Backspace`（`←` 不绑定）。
- 侧栏中保留辅助键：`r` 刷新、`t` 主题、`?` 帮助。
- 鼠标滚轮跟随焦点：侧栏滚列表，阅读器滚文档。
- 目录统计预览、文件预览缓存、空目录提示文案等旧浏览器 UI 一并删除。
- 无文件且侧栏被关时，右侧提示 `Press o to open the sidebar`。

## 架构（方案 A：去 Mode + sidebar 状态）

`src/browse.rs` 零改动复用（仅删除不再使用的 `dir_stats` 及其测试）。

`src/app.rs`：

```rust
pub enum Focus { Sidebar, Reader }

pub struct Sidebar {
    pub browser: Browser,
    pub focus: Focus,
}

pub struct App {
    pub reader: Option<Reader>,     // 可为空（无文件）
    pub sidebar: Option<Sidebar>,   // None = 侧栏关闭
    // 删除：mode、preview、resume_hint
    ...
}
```

- 删除 `Mode` 枚举与 `browser_key`；新增 `sidebar_key`（侧栏焦点）；
  `reader_key` 保留全量阅读器键（阅读器焦点时照常响应）。
- `handle_key` 顶层分派：主题选择器 / 帮助 / `t` / `?` 全局优先；侧栏开时
  `Tab` 切焦点，其余按 `sidebar.focus` 分派；侧栏关全部走 `reader_key`。
- `open_reader` 保持 reveal 不变；从侧栏 `Enter` 打开文件后关闭侧栏。
- 启动分流（`event_loop`）：CLI 文件 / `resume_latest` → 阅读器；否则
  `reader = None` + 开侧栏。

## 按键表

| 场景 | 键 | 行为 |
|---|---|---|
| 全局 | `t` / `?` | 主题 / 帮助（不变） |
| 全局（侧栏开） | `Tab` | 切换焦点 |
| 侧栏焦点 | `j/k`、`↓/↑` | 移动选中 |
| 侧栏焦点 | `Enter` | 目录 → 进入；md 文件 → 打开 + 关侧栏 |
| 侧栏焦点 | `Backspace` | 上级目录（越根出驱动器列表） |
| 侧栏焦点 | `r` | 刷新当前目录 |
| 侧栏焦点 | `Esc` / `q` | 关闭侧栏 |
| 阅读器焦点 | `o` | 打开侧栏（已开则无操作） |
| 阅读器焦点 | `q` | 退出程序 |
| 阅读器焦点 | 其余阅读器键 | `j/k/d/u/g/G///n/N/a/Ctrl+f/b` 不变；`Esc` 不绑定 |
| 鼠标滚轮 | — | 跟随焦点 |

## UI

- 侧栏开：水平二分，左 30%（`sidebar_width`）侧栏，右 70% 阅读器；
  侧栏关：阅读器全宽。
- 侧栏渲染复用现有列表（`▸ name/`、路径标题截断、驱动器层）；
  删除右栏预览/`preview` 缓存/目录统计。
- 焦点反馈：焦点所在面板边框用 `accent_style`，另一面板用 `chrome_style`。
- 空阅读器右侧提示文本（有侧栏时 `Select a markdown file from the sidebar`，
  无侧栏时 `Press o to open the sidebar`）。
- 状态栏：侧栏焦点 → `"Enter open · Bksp up · r refresh · Tab focus · ? help"`
  / `"Esc close"`；阅读器焦点 → 现有阅读器状态栏（侧栏开时追加 `Tab focus`）。
- 帮助浮层按键表按新模型重写。

## 配置

`config.toml` 新增 `sidebar_width`（百分比，默认 30，clamp 10..=60）；
读写沿用现有「读-改-写保留其他键」模式；`bin/config.toml` 示例同步。

## 清理清单

删除：`Mode`、`browser_key`、`App.preview`、`resume_hint`/`draw_resume_hint`、
`dir_stats`（及其测试）、旧浏览器状态栏文案、帮助浮层旧条目。

## 错误处理

沿用现状：进入/上级/刷新失败 → 停留原位 + `app.status` 提示
（`cannot read <path>: <err>`）。

## 测试

- 改造：`test_app` 去 `Mode`；原浏览器 5 测试改写为侧栏语义（`Enter`
  打开/进入、`Backspace` 上级、reveal）。
- 新增：`Tab` 切焦点；`q` 随焦点（侧栏关栏 / 阅读器退出）；`Esc` 仅侧栏
  焦点有效；阅读器焦点 `o` 开栏；`Enter` 打开文件后关栏且 reader 就位；
  无文件启动自动开栏（`reader = None` + `sidebar.is_some()`）。
- `browse.rs`：仅删 `dir_stats` 测试，其余不动。

## 文档

`README.md` / `README.zh-CN.md` 按键说明重写；`bin/config.toml` 加
`sidebar_width` 示例；`AGENTS.md` 架构描述同步。
