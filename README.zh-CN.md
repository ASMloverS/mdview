# mdview

[English](README.md)

终端 Markdown 渲染器：交互形态参考 [glow](https://github.com/charmbracelet/glow)，渲染效果向 VS Code 的 Markdown 预览看齐。Rust 编写，TUI 基于 ratatui，语法高亮基于 syntect。

## 特性

- **TUI 浏览**：文件浏览器 + 阅读器（滚动、搜索、主题热切换、帮助面板）
- **VS Code 风格渲染**：h1/h2 下划规则线、代码块行号与语言标签、引用块背景、表格双线表头分隔、内容限宽居中
- **语法高亮**：syntect 驱动，颜色取自当前主题的 `syntax-*` 规则
- **主题系统**：8 个内置主题 + 自定义 CSS 主题；选择器选定的主题自动持久化
- **管道模式**：stdin 进、ANSI 出，可作 `cat` 增强
- Markdown 支持：标题/列表/任务列表/表格/代码块/引用/数学公式/脚注/链接（OSC8 可点击 + 文末链接列表）

## 构建

```bash
cargo build --release
```

Windows（MSVC）下如遇到链接器问题，用仓库自带的环境包装脚本：

```bash
.cargo-vc.bat build --release
.cargo-vc.bat test
```

## 使用

```bash
mdview                # 打开文件浏览器（扫描当前目录的 .md）
mdview README.md      # 直接打开文件
mdview -t dracula a.md
mdview --list-themes  # 列出所有可用主题
cat a.md | mdview     # 管道模式，渲染到 stdout
```

选项：`-t, --theme <名称>`、`-w, --max-width <列数>`（默认 100）、`--list-themes`。

### 按键

| 按键 | 作用 |
| --- | --- |
| `j/k` `↓/↑` | 移动 / 滚动 |
| `d/u` `PgDn/PgUp` | 半屏下 / 上 |
| `Ctrl+f` `Ctrl+b` | 整屏下 / 上（vim 风格，保留 2 行重叠） |
| `g/G` | 顶部 / 底部 |
| `/` `n/N` | 搜索 / 下一个、上一个匹配 |
| `t` | 主题选择器（j/k 预览，关闭时保存选择） |
| `Enter/l` | 打开文件（浏览器） |
| `Esc` | 返回浏览器 |
| `r` | 重新扫描文件（浏览器） |
| `?` | 帮助面板 |
| `q` | 退出 |

## 主题

内置主题：`tokyo-night`、`dracula`、`gruvbox-dark`、`gruvbox-light`、`nord`、`solarized-dark`、`solarized-light`、`github-light`（默认 `gruvbox-dark`）。

自定义主题：在可执行文件旁的 `md-styles/<名称>.css` 写一个 CSS 文件（查找顺序：可执行文件旁 `md-styles/` → 内置），然后用 `-t <名称>` 或在选择器里选中。同名文件会覆盖内置主题。

CSS 子集支持：元素与后代选择器（`pre code`）；属性 `color`、`background`/`background-color`、`border-color`、`font-weight`、`font-style`、`text-decoration`；颜色支持 `#rgb`/`#rrggbb`/`rgb(r,g,b)`/命名色。语法高亮用 `syntax-keyword`、`syntax-string`、`syntax-comment`、`syntax-function`、`syntax-type`、`syntax-number`、`syntax-operator` 七个类。参考 `assets/styles/` 下的内置主题。

## 配置

`config.toml`（可执行文件旁），全部可选：

```toml
theme = "gruvbox-dark"  # 主题选择器关闭时自动写入
max_width = 100
mouse = true
```

优先级：`-t` 参数 > `config.toml` > 内置默认（`-t` 为一次性覆盖，不落盘）。
