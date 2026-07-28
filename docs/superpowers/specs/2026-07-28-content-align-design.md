# 内容对齐（居中/左对齐）— 设计文档

日期：2026-07-28
状态：已确认

## 目标

1. 当前渲染内容默认居中展示，需要支持调整为左对齐展示
2. 支持在阅读器内用按键即时切换，并持久化到配置文件
3. 默认行为保持居中不变，不加配置的用户无感知

## 已确认的决策

- 默认仍为**居中**；左对齐通过 `config.toml` 的 `align = "left"` 开启
- 作用范围：阅读器视图（Reader）为主；`--align` 与 `align` 配置对**管道模式同样生效**（行为一致不割裂）；浏览器预览面板本就左对齐（offset=0），不动
- 按键：阅读器内 `a` 键在 居中 ↔ 左对齐 间切换，**立即写入 config.toml 持久化**（与主题选择器同一模式）；浏览器模式不绑定
- 配置暴露：`config.toml` 加 `align = "left" | "center"`，CLI 加 `--align <center|left>`；优先级 **CLI > config.toml > 默认 center**（与 theme/max_width 一致）

## 现状分析

居中不是布局引擎的职责：布局只在内容宽度 `width` 内排版，居中靠
`render_document(..., offset)` 在产出的每行行首插入 `offset` 个空格
（`src/render/layout/mod.rs:78`）。`offset` 公式
`(可用宽度 - 2 - width) / 2` 在三处重复：

- `src/main.rs:65` — 管道模式（`term` 已减 2）
- `src/app.rs:240` — 启动直开文件
- `src/ui/reader.rs:17` — 阅读器每帧按终端尺寸重算，变化即重排版

因此左对齐 = `offset` 算成 0，布局引擎零改动。

## 设计

### 1. `ContentAlign` 类型（`src/config.rs`）

```rust
pub enum ContentAlign { Center, Left }
```

- `from_str("left"|"center") -> Option<ContentAlign>` 容错解析；非法值回退默认
- `as_str()`、`toggle()`
- derive `clap::ValueEnum` 供 CLI 使用
- 命名避开 `src/markdown/ir.rs` 的表格列 `Align`

`Config` 加 `pub align: Option<String>`——用 String 而非强类型反序列化，
无效值只让该键回退默认，不拖累整个配置文件解析。

### 2. 统一 offset 计算（`src/app.rs`）

```rust
pub fn content_offset(inner_width: u16, width: u16, align: ContentAlign) -> u16
// Center: inner_width.saturating_sub(width) / 2
// Left:   0
```

收敛三处重复公式：管道模式、启动直开、阅读器每帧重算（读 `app.align`）。
`App` 加 `align` 字段，`App::new` / `app::run` / `event_loop` 依次透传；
`main.rs` 解析 CLI > config > 默认。

### 3. 阅读器 `a` 键切换

- `App::toggle_align()`：翻转 `align` + 设置 status 提示（`"align: left"`）；
  纯方法不做 IO，便于单测
- `reader_key` 绑定 `a`：调 `toggle_align()` 后立即
  `Config::save_align(...)` 持久化（与 `close_picker` 同一模式）
- **无需手动触发重排版**：`ui/reader.rs` draw 中既有
  `reader.offset != want_offset` 比较，`app.align` 变化 → 下一帧
  `want_offset` 变化 → 自动重排版

### 4. 持久化（`src/config.rs`）

`save_theme_to` 泛化为 `save_key_to(path, key, value)`，`save_theme` /
`save_align` 作薄封装。保留现有语义：保留其他键、文件损坏不写、值未变不写。

### 5. 帮助与文档

- 帮助面板加 `("a", "toggle align (reader)")`
- `build.bat` 的 `:write_config` 模板与 `bin/config.toml` 加 align 注释行
- `README.md` / `README.zh-CN.md`：选项、按键、配置块、优先级说明
- `AGENTS.md` Conventions 的 Config 条目补 align 持久化说明

## 验证

- 每任务 TDD：config 解析/保存、content_offset 公式、toggle_align 翻转与
  status 提示均有单测
- `cmd //c ".cargo-vc.bat test"` 全量通过、零警告
- 手动：阅读器内按 `a` 内容贴左且 `config.toml` 写入 `align = "left"`；
  重启后保持左对齐；`--align left` 管道输出贴左

## 范围外（YAGNI）

- 右对齐、三态循环
- 浏览器预览面板的对齐控制（本就左对齐）
- 浏览器模式下的 `a` 键绑定
- `max_width` 语义调整（左对齐只是 offset=0，内容宽度限制不变）
