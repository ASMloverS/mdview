# 语法高亮主题（syntax-styles）设计

日期：2026-08-04
状态：已确认

## 背景与目标

现状：语法高亮由 `src/highlight.rs`（syntect）实现，配色取自页面主题
`md-styles/<theme>.css` 中的 7 条 `syntax-*` 规则，全局应用于所有语言：

- 语法配色与页面主题耦合，无法独立选择；
- 只有 7 个类别，粒度粗（变量/常量/宏/属性/模块/标点不可区分）；
- 无 per-language 特化能力；
- 缺陷：`highlight.rs` 给 comment 硬编码 ITALIC，但 `block.rs` 构建 span
  时只取 `fg`，粗体/斜体从未真正生效。

目标：语法主题独立成 `syntax-styles/*.css`（与 `md-styles/` 同目录、同机制），
支持用户自定义、per-language 特化、16 个类别、粗体/斜体/下划线真正生效。

## 需求决策（已与用户确认）

- 文件组织：**一个文件 = 一套语法主题**，与页面主题平行，可独立切换。
- 与页面主题关系：**解耦**。`config.toml` 新增 `syntax_theme` 独立配置；
  页面主题的 `syntax-*` 规则保留，作为回退层（向后兼容）。
- 类别粒度：扩展为固定 16 类，每类映射一组 sublime scope。
- 字体样式：本次一并修复，代码块支持 color + bold + italic + underline。
- 语言特化语法：语言名作祖先选择器（`rust macro { ... }`）。
- 内置主题：从 20 个页面主题提取，同名配对。
- 切换方式：仅 `config.toml` + CLI `--syntax-theme`，不动 TUI picker。
- 缺失回退：逐类别三级回退（语法主题 → 页面主题 syntax-* → 别名派生）。
- CLI：新增 `--syntax-theme <name>` 与 `--list-syntax-themes`；
  现有 `--list-themes` 输出不变。
- 分发：`build.bat` 复制一份带注释示例到 `bin/syntax-styles/example.css`。
- 语言名匹配：fence token 小写原文 + syntect 规范语言名小写都匹配
  （` ```rs ` 命中 `rust macro { ... }`）。

## CSS 语法

复用现有 `css.rs` 解析器，零改动。选择器链 leaf = 类别名，祖先 = 语言名：

```css
/* 全局类别规则 */
keyword  { color: #fb4934; font-weight: bold; }
comment  { color: #928374; font-style: italic; }

/* 语言特化：语言名作祖先选择器 */
rust macro       { color: #fe8019; }
python decorator { color: #8ec07c; }
```

支持属性：`color`、`font-weight`、`font-style`、`text-decoration: underline`。
`background`、`border-color`、`text-decoration: line-through` 对代码 token
不支持（syntect FontStyle 无 strike；token 背景与 `pre` 铺底冲突），文档注明忽略。

## 16 类别与 scope 映射

| 类别 | sublime scope selector |
|---|---|
| keyword | `keyword, storage` |
| string | `string` |
| comment | `comment` |
| function | `entity.name.function, support.function, meta.function-call variable.function` |
| type | `entity.name.type, entity.name.class, support.type, support.class, storage.type` |
| number | `constant.numeric` |
| operator | `keyword.operator` |
| variable | `variable` |
| constant | `constant.language, constant.other` |
| macro | `entity.name.macro, support.macro` |
| attribute | `entity.other.attribute-name, entity.name.attribute` |
| decorator | `storage.type.annotation, punctuation.definition.annotation` |
| module | `support.module, entity.name.module` |
| namespace | `entity.name.namespace` |
| punctuation | `punctuation` |
| label | `entity.name.label` |

注：`constant.language`（true/false/null 等）从 number 移到 constant；
ThemeItem 顺序按上表自上而下（宽泛在前、具体在后，保证 syntect 打分时
`entity.name.function` 优先于 `variable`）。

## 解析与回退链

逐类别解析（`SyntaxTheme::resolve(langs, class)`），返回
`{ fg: Option<Rgb>, bold, italic, underline }`，`fg = None` 表示用代码块默认前景色：

1. 语法主题语言特化规则 `[lang, class]`（langs = fence token 小写、
   syntect 规范语言名小写，按序尝试）；
2. 语法主题全局规则 `[class]`；
3. 页面主题 `syntax-<class>`（`Scheme::style_for(&["body", "syntax-<class>"])`，
   Computed 自带 bold/italic/underline，原 syntax-comment 的 italic 由此生效）；
4. 别名派生：constant→number、macro→function、attribute→type、
   decorator→function、module→type、namespace→type、punctuation→operator、
   label→keyword、variable→`None`（默认前景色）；别名目标自身再走上层回退，
   最多跳一次（别名目标不允许再是别名类，上表保证）；
5. 全部缺失：`fg = None`、无字体样式。

## 模块结构与改动清单

- **`src/style/syntax.rs`（新）**
  - `SyntaxTheme { name, rules: Vec<Rule> }`；
  - `SyntaxTheme::load(name) -> SyntaxTheme`：exe 同级 `syntax-styles/<name>.css`
    → 内置 `assets/syntax-styles/<name>.css` → 空主题（rules=[]，全部走回退）；
    目录解析复用 `style_dirs()` 模式（提取共用辅助函数，目录名参数化）；
  - `SyntaxTheme::available() -> Vec<String>`（内置 + 用户目录 stem，供
    `--list-syntax-themes`）；
  - `resolve(&self, langs: &[String], class: &str, page: &Scheme) -> SyntaxStyle`。
- **`src/highlight.rs`（重写）**
  - `SCOPE_MAP` 扩到 16 类；
  - `Highlighter::new(scheme: &Scheme, syntax_theme: &SyntaxTheme)`；
  - 每个语言懒构建 syntect `Theme`（16 个 ThemeItem，fg + FontStyle
    BOLD/ITALIC/UNDERLINE），`HashMap<String, Theme>` 缓存；无语言（纯文本）
    直接走 `default_fg`；
  - `highlight()` 返回类型改为 `Vec<Vec<HSpan>>`：
    `HSpan { fg: Rgb, bold: bool, italic: bool, underline: bool, text: String }`。
- **`src/render/layout/block.rs`**：代码块 span 应用
  `Computed { fg, bg: pre.bg, bold, italic, underline, .. }`（修复 comment 斜体）。
- **`src/config.rs`**：`syntax_theme: Option<String>` 字段 +
  `save_syntax_theme(name)`（复用 `save_key_to`）。
- **`src/main.rs`**：`--syntax-theme <name>`、`--list-syntax-themes`；
  解析顺序：CLI > config > 页面主题同名 > `DEFAULT_THEME` 同名。
- **`src/app.rs` / TUI**：保存 `syntax_theme_override: Option<String>`
  （CLI/config 提供）；运行时按 `t` 切换页面主题时，无 override 则语法主题
  跟随新页面主题名重新 resolve，有 override 则固定不变。Renderer 重建逻辑
  沿用现有主题切换路径。
- **`assets/syntax-styles/*.css` × 20（新）**：从对应页面主题提取 syntax-*
  配色（去 `syntax-` 前缀），补齐 16 类——新增 9 类按别名派生规则从该主题
  色板选具体色值，使每个文件自包含、可直接当用户模板。
- **`assets/syntax-styles/example.css`（新）**：带完整注释的示例（16 类清单 +
  语言特化写法），`build.bat` 复制到 `bin/syntax-styles/`。
- **`src/style/scheme.rs`**：`syntax_color()` 保留（回退层改用
  `style_for(&["body", "syntax-<class>"])` 取完整 Computed，方法签名按需调整）；
  现有 7 类完整性测试保持通过。

## 数据流

```
CLI --syntax-theme / config.syntax_theme / 页面主题名
  → SyntaxTheme::load（user 目录 > 内置 > 空）
  → Highlighter::new(page_scheme, syntax_theme)
  → 每语言懒构建 syntect Theme（16 类经 resolve 三级回退）
  → highlight(code, lang) → Vec<Vec<HSpan{fg, bold, italic, underline, text}>>
  → block.rs → SSpan{fg, bg: pre.bg, bold, italic, underline}
```

## 测试

- `syntax.rs`：加载优先级（user > builtin > 空）、`available()` 去重；
  回退链逐级命中（语言特化 > 全局 > 页面 syntax-* > 别名 > None）；
  fence 别名（`rs` → `rust`）匹配；`font-weight`/`font-style` 解析进
  SyntaxStyle。
- `highlight.rs`：HSpan 携带 bold/italic；per-language 覆盖生效；
  未知语言走 default_fg；`constant.language` 归 constant 而非 number。
- `block.rs` 渲染测试：comment span 带 italic modifier（断言 span style）。
- 内置完整性：20 个内置语法主题均可解析、16 类全覆盖（仿
  `builtin_schemes_cover_template_elements`）。
- 现有测试套件全量通过，零警告。

## 文档

- `docs/custom-themes.md` / `custom-themes.zh-CN.md`：新增 syntax-styles
  章节（文件位置、CSS 语法、16 类表、语言特化、回退链、example.css 说明）。
- `AGENTS.md`：架构段落补 `syntax-styles` 与 `src/style/syntax.rs`；约定段落
  补语法主题查找规则。
- `build.bat`：复制 `example.css` 到 `bin/syntax-styles/`。

## 非目标

- TUI picker 不支持语法主题切换（仅 config/CLI）。
- 不暴露原始 sublime scope 作为 CSS 选择器。
- 不支持 token 背景色与删除线。
- 页面主题的 `syntax-*` 规则不删除（保留为回退层）。
