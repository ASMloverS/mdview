# 重新打开文件时恢复上次光标位置 — 设计文档

日期：2026-07-30
状态：已确认

## 目标

TUI 阅读器中关闭文件后（Esc 回 Browser 或 q 退出程序），再次打开同一文件时，
光标自动恢复到上次关闭时所在的渲染行。

## 已确认的决策

- 存储位置：exe 同目录的独立文件 `history.toml`（不混入 `config.toml`）
- 落盘时机：关闭文件时（Esc / q / Ctrl+C），非每次光标移动
- 文件标识：`canonicalize` 后的绝对路径
- 存储内容：仅光标所在渲染行号，不存 scroll
- 容量控制：LRU，默认保留 200 条，`config.toml` 的 `history_size` 可配，
  0 = 禁用恢复功能
- 崩溃/直接关终端丢失本次位置：可接受

## 设计

### 1. 数据格式（`history.toml`）

MRU 在前，数组顺序即 LRU 顺序，无需时间戳：

```toml
[[history]]
path = "C:/abs/path/to/file.md"
line = 42
```

### 2. 新模块 `src/history.rs`

- `History` 内部为 `Vec<Entry>`（`Entry { path: PathBuf, line: usize }`），
  最前为最近使用
- `History::load() -> History`：读 exe 同目录 `history.toml`（路径解析与
  `config.rs` 的 `config_path()` 同款：exe 目录，失败回退 cwd 相对路径）；
  文件缺失或损坏返回空
- `get(&self, path: &Path) -> Option<usize>`：按 canonicalize 后的绝对
  路径查找行号
- `record(&mut self, path: &Path, line: usize, cap: usize)`：canonicalize
  路径；移除同路径旧条目；插到最前；截断到 `cap`；立即写盘。
  best-effort：IO 错误静默忽略（与 `Config::save_theme` 同款语义），
  损坏文件不覆盖之外的场景不存在——本文件由程序独占维护，
  解析失败时按空历史重建
- 测试注入路径：内部函数接受文件路径参数，公有包装固定 exe 同目录
  （同 `config.rs` 的 `save_key_to` / `save_theme` 分层）

### 3. 配置（`src/config.rs`）

`Config` 加 `history_size: Option<usize>`，缺省默认 200。只读不写（用户手
动编辑 `config.toml`，程序不主动落盘该键）。

### 4. 接线（`src/main.rs` / `src/app.rs`）

- `main.rs`：`cfg.history_size.unwrap_or(200)` 传入 `app::run()`
- `App` 加字段 `history: History`、`history_size: usize`；
  `App::new` 时 `History::load()`
- `open_reader`：渲染后若 `history_size > 0` 且 `history.get(&path)` 命中：
  - `cursor = saved.min(last)`（last = 渲染行数 - 1）
  - `scroll = cursor.saturating_sub(view_height / 2)`（光标大致居中；
    此时 view_height 是占位值 24，可接受，draw 后不再自动调整）
  - 未命中或无记录：`cursor = 0`、`scroll = 0`，与现状一致
- 保存时机（`history_size > 0` 时）：
  - `reader_key` 的 `Esc` 分支（回 Browser）
  - `reader_key` 的 `q`、`Ctrl+C` 分支（退出程序）
  - 均调用 `history.record(&reader.path, reader.cursor, self.history_size)`

### 5. 边界行为

- 文件被外部改短：恢复时 clamp 到末行
- 宽度/主题变化导致排版漂移：只保证不越界，不追求精确（存的是渲染行号）
- Esc 回 Browser 时已写盘，之后从 Browser 或命令行重开同一文件均可恢复
- 从 Browser 打开另一文件：前一文件位置已在 Esc 时落盘，无遗漏

### 6. 测试

- `history.rs`：
  - load：文件缺失返回空、损坏 TOML 返回空
  - record：新条目插最前、同路径去重提到最前、超过 cap 截断最旧条目
  - 写盘后重新 load 往返一致
- `app.rs`：
  - `open_reader` 命中记录时恢复 cursor（预置 history）
  - 无记录时 cursor = 0
  - 记录行号超出文档长度时 clamp 到末行

## 范围外（YAGNI）

- 恢复 scroll 偏移（视觉位置精确还原）
- 源文件行号映射（排版变化后的精确恢复）
- 淘汰已删除文件的条目
- 崩溃时位置的实时保存
- pipe 模式（无光标概念，不涉及）
