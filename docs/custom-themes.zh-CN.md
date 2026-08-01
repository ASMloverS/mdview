# 编写自定义主题

[English](custom-themes.md)

mdview 的主题就是一个 CSS 文件，用的是刻意精简的 CSS 子集——只能控制颜色、粗体/斜体/下划线和边框色。本教程覆盖该子集的全部能力、主题加载方式，以及每个选择器与渲染出的 Markdown 元素的对应关系。

## 目录

- [快速上手](#快速上手)
- [主题的加载与启用](#主题的加载与启用)
- [选择器](#选择器)
- [属性](#属性)
- [颜色](#颜色)
- [元素参考](#元素参考)
- [完整示例](#完整示例)
- [常见陷阱](#常见陷阱)
- [参考资料](#参考资料)

## 快速上手

1. 在 `mdview` 可执行文件旁新建 `md-styles/my-theme.css`。
2. 写几条规则：

   ```css
   body { color: #d8dee9; background-color: #2e3440; }
   h1 { color: #bf616a; font-weight: bold; }
   code { color: #ebcb8b; }
   ```

3. 启用：`mdview -t my-theme README.md`，或在 mdview 里按 `t` 从列表中选择 `my-theme`。

文件主名即主题名。流程就这么多——本文其余部分是参考手册。

## 主题的加载与启用

请求名为 `<名称>` 的主题时，**查找顺序**：

1. 可执行文件旁的 `md-styles/<名称>.css`
2. 同名内置主题
3. 默认主题 `gruvbox-dark`（主题名不存在时静默回退到这里）

`md-styles/` 中与内置主题同名的文件会**整体覆盖**该内置主题。

**主题按以下优先级启用**（从高到低）：

1. 命令行参数：`mdview -t <名称>` / `--theme <名称>`——一次性生效，不落盘
2. `config.toml`（可执行文件旁）中的 `theme = "<名称>"`
3. 内置默认 `gruvbox-dark`

**发现主题**:`mdview --list-themes` 列出全部可用主题（内置 + `md-styles/` 里的所有 `.css`，去重排序）。程序内的主题选择器（按 `t`）会把自定义主题和内置主题一起列出，关闭时把选中的主题写回 `config.toml`。

## 选择器

支持：

- **元素选择器**:`h1`、`blockquote`、`syntax-keyword`……（字母、数字、连字符，大小写不敏感）
- **后代选择器**:`table td`。注意 `>` 被当作普通空格——`table > td` 和 `table td` 完全等价，没有"仅子代"的语义。
- **逗号分组**:`table, th, td { ... }`

不支持（静默剥离或跳过）:

- **类 / ID / 伪类后缀会被剥掉**:`a:hover` 变成 `a`,`p.note` 变成 `p`。规则依然生效——匹配范围可能比你预期的大。
- **通配符 `*`**：整条规则被跳过。

匹配规则：

- **特异性 = 选择器中的元素个数**。`blockquote p` 优先于 `p`。
- 特异性相同时，**后写的规则按属性逐个覆盖**先写的（不是整条规则替换）。
- **无继承**:`p { color: red }` 不会影响段落里的 `strong`。每个元素只被"选择器链尾等于该元素"的规则影响；没有命中的属性落到全局默认：前景 `#d4d4d4`，无背景，无粗斜体下划线。

## 属性

只识别七个声明，其余（包括 `border:`、`font:`、`margin` 等所有简写属性）一律忽略。属性名和值比较时不区分大小写。

| 声明 | 作用 | 合法取值 |
| --- | --- | --- |
| `color` | 前景色 | 任意颜色值（见[颜色](#颜色)） |
| `background` / `background-color` | 背景色（两个名字完全等价，只支持纯色） | 任意颜色值 |
| `border-color` | 装饰线/边框颜色（h1/h2 规则线、引用竖条、表格框线、`hr`) | 任意颜色值 |
| `font-weight` | 粗体 | `bold`、`bolder`、`600`–`900` → 开；其他任何值 → 关 |
| `font-style` | 斜体 | `italic`、`oblique` → 开；其他任何值 → 关 |
| `text-decoration` / `text-decoration-line` | 下划线与删除线 | 值含 `underline` → 下划线；含 `line-through` 或 `strikethrough` → 删除线。一条声明同时设置两个开关，因此 `text-decoration: none` 会把两者都显式关掉 |

## 颜色

三种写法，内部一律转成 truecolor:

- **十六进制**:`#rgb`、`#rrggbb`、`#rrggbbaa`(alpha 通道被忽略）
- **函数**:`rgb(r, g, b)`——必须恰好三个逗号分隔的 0–255 整数。不支持 `rgba()`、`hsl()`、百分比。
- **命名色**(18 个）:`black`、`white`、`red`、`green`、`blue`、`yellow`、`orange`、`purple`、`pink`、`cyan`/`aqua`、`magenta`/`fuchsia`、`gray`/`grey`、`silver`、`teal`、`lime`、`navy`、`maroon`、`olive`

**终端降级是自动的**——主题只管写 truecolor 值：

- `COLORTERM` 含 `truecolor`/`24bit`，或运行在 Windows Terminal → truecolor
- `TERM` 含 `256color` → 映射到最接近的 xterm-256 色
- 否则 → 映射到最接近的 16 色 ANSI 色

## 元素参考

Markdown 元素按其祖先链链尾的标签取样式。下表列出主题可覆盖的全部标签及其特殊行为。

| 标签 | 渲染对象 | 说明 |
| --- | --- | --- |
| `body` | 全局默认文字与背景 | 其背景同时铺满整个 TUI 界面 |
| `h1`–`h6` | 标题 | `h1`/`h2` 会在标题下方画整宽规则线（h1 为 `═`,h2 为 `─`)，颜色取 `border-color`（缺省回退 `color`);`h3`–`h6` 不画线 |
| `p` | 段落文字 | |
| `strong` / `em` / `del` | 粗体 / 斜体 / 删除线行内片段 | |
| `code` | 行内代码 | **仅限行内代码**——代码块不匹配 `code`，因此 `pre code` 没有作用目标 |
| `pre` | 代码块 | `color` 是代码的默认字色（也是无高亮语言的字色）;`background` 铺满代码块。行号槽和语言标签使用 `footnote` 的前景 + `pre` 的背景 |
| `a` | 链接文字 | 文末链接列表的标题也用 `a`；列表中的 `[n]` 序号标记用 `footnote` |
| `img` | 图片占位符 `🖼 alt` | |
| `math` | 行内与块级数学公式 | 块级公式居中；无法转成 Unicode 时回退为 `pre` 样式 |
| `blockquote` | 引用块 | `color` = 文字，`background` = 整行铺底，`border-color` = 左侧 `▎` 竖条（缺省回退 `color`) |
| `li` | 仅列表标记 | 只控制 `•` / `1.` / `☑` 标记的颜色；列表正文仍由 `p` 等决定 |
| `table` / `th` / `td` | 表格 | `table` 的 `border-color` 控制所有框线字符；`th` = 表头，`td` = 正文单元格 |
| `hr` | 分隔线 | `border-color`，缺省回退 `color` |
| `footnote` | 脚注引用与定义 | 同时兼任全局"弱化色"：代码行号、语言标签、链接 `[n]` 标记等次要文本 |
| `cursor` | TUI 阅读器光标行 | 只取 `background-color`；不写则无光标行高亮 |
| `syntax-keyword` | 代码块中的关键字 | 只取颜色 |
| `syntax-string` | 字符串 | 只取颜色 |
| `syntax-comment` | 注释 | 只取颜色——注释**始终为斜体**，由高亮器强制，与 `font-style` 无关 |
| `syntax-function` | 函数名 | 只取颜色 |
| `syntax-type` | 类型 | 只取颜色 |
| `syntax-number` | 数字 | 只取颜色 |
| `syntax-operator` | 运算符 | 只取颜色 |

## 完整示例

一份覆盖全部标签的完整主题，逐段注释。保存为可执行文件旁的 `md-styles/my-theme.css`，然后运行 `mdview -t my-theme`。

```css
/* my-theme — mdview 完整自定义主题示例 */

/* 全局默认：文字颜色与整个界面的背景 */
body { color: #d5c4a1; background-color: #1d2021; }

/* 标题：h1/h2 的 border-color 同时决定标题下方规则线的颜色 */
h1 { color: #fb4934; font-weight: bold; border-color: #fb4934; }
h2 { color: #fe8019; font-weight: bold; border-color: #fe8019; }
h3 { color: #fabd2f; font-weight: bold; }
h4 { color: #b8bb26; font-weight: bold; }
h5 { color: #8ec07c; font-weight: bold; }
h6 { color: #928374; font-weight: bold; }

/* 行内文本：段落与强调片段 */
p { color: #d5c4a1; }
strong { font-weight: bold; }
em { font-style: italic; }
del { color: #928374; text-decoration: line-through; }
a { color: #83a598; text-decoration: underline; }
img { color: #928374; }
math { color: #d3869b; font-style: italic; }

/* 代码：行内代码与代码块本体 */
code { color: #fabd2f; background-color: #32302f; }
pre { color: #d5c4a1; background-color: #32302f; }

/* 代码块内的语法高亮（只取颜色） */
syntax-keyword { color: #fb4934; }
syntax-string { color: #b8bb26; }
syntax-comment { color: #928374; }
syntax-function { color: #b8bb26; }
syntax-type { color: #fabd2f; }
syntax-number { color: #d3869b; }
syntax-operator { color: #8ec07c; }

/* 块级元素：引用、列表、表格、分隔线 */
blockquote { color: #bdae93; background-color: #32302f; border-color: #504945; }
li { color: #fe8019; }
table, th, td { color: #d5c4a1; border-color: #504945; }
th { font-weight: bold; }
hr { border-color: #504945; }

/* 次要文本与界面：脚注/弱化色、光标行高亮 */
footnote { color: #928374; }
cursor { background-color: #3c3836; }
```

调试技巧：

- 用选择器快速迭代：打开任意文件，按 `t` 选中你的主题，修改 CSS 后重启 mdview 即可看到效果。
- 对照 `assets/styles/` 下的内置主题——每个都是一份约 30 条规则的完整最小主题。

## 常见陷阱

- **缺少 `}` 会丢弃其后所有规则**。解析器在未闭合的规则处终止，文件剩余部分全部作废。这是唯一影响范围超出单条的解析错误。
- **其余错误全部静默**。未知属性、畸形声明、不支持的选择器——一律忽略，没有任何警告。某条规则"不生效"时，先检查拼写。
- **自定义主题不与内置主题合并**。自定义主题是完整独立的一套规则；没写的元素落到全局默认（前景 `#d4d4d4`、无背景），而不是回退到内置默认主题。
- **无效颜色只使该条声明失效**——它只是不设置该属性，不会清掉之前命中规则设置的有效值。
- **`p.note` 会退化成 `p`**。类/ID/伪类后缀被剥离，规则的实际匹配范围可能比写出来的更大。
- **`pre code` 匹配不到任何东西**。代码块只通过 `pre` 设置样式，`code` 仅限行内代码。
- **`>` 表示"后代"而非"子代"**。`table > td` 与 `table td` 等价。
- **代码块中的注释永远是斜体**，由语法高亮器强制。`syntax-comment { font-style: normal }` 无效。
- **主题名写错会静默回退**到 `gruvbox-dark`。主题似乎没加载时，先跑 `mdview --list-themes` 确认名字。

## 参考资料

- `assets/styles/`——20 个内置主题，最好的改写模板
- `src/style/css.rs`——子集解析器（支持语法的权威清单）
- `src/style/scheme.rs`——主题查找、匹配与回退逻辑
