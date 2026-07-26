# mdview 阅读器整屏翻页（Ctrl+f / Ctrl+b）— 设计文档

日期：2026-07-26
状态：已确认

## 目标

为阅读器新增 vim 风格整屏翻页：`Ctrl+f` 向前一屏、`Ctrl+b` 向后一屏。
现有 `d`/`u`（半屏）与 `PgDn`/`PgUp` 行为不变。

## 已确认的决策

- 一屏 = `view_height - 2` 行（保留 2 行上下文重叠，vim 风格）；小窗口
  （view_height ≤ 2）退化为 1 行，防止 0 或下溢
- 仅阅读器模式生效，文件浏览器不支持
- 为翻页逻辑补单元测试（app.rs 目前无测试）

## 设计

### 1. 按键绑定（`src/app.rs` `reader_key`）

现有 `page`（半屏 = view_height/2）旁新增 `full_page`：

```rust
KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    scroll_reader(app, full_page)
}
KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
    scroll_reader(app, -full_page)
}
```

无按键冲突：f/b 未绑定其他功能；搜索输入模式优先吞键不受影响。
滚动夹取沿用 `scroll_reader` 现有逻辑（0..=max）。

### 2. 幅度函数（`src/app.rs`）

```rust
fn page_delta(view_height: usize) -> usize {
    view_height.saturating_sub(2).max(1)
}
```

独立成函数以便单元测试。

### 3. 帮助面板（`src/ui/mod.rs` `draw_help`）

在 `d/u, PgDn/PgUp` 行之后新增一行：`("Ctrl+f/b", "page forward / back")`。

### 4. 单元测试（`src/app.rs` 新增 `#[cfg(test)] mod tests`）

- `page_delta(24) == 22`；`page_delta(2) == 1`；`page_delta(1) == 1`
- `scroll_reader` 边界：scroll=0 时向后翻页仍为 0；接近底部时夹到 max
- 按键映射：构造 `App` + `Reader`（view_height=24、100 行内容），模拟
  Ctrl+f 的 `KeyEvent`，断言 scroll 前进 22 行

## 范围外（YAGNI）

- 浏览器翻页
- 帮助面板以外的文档更新
- `d`/`u` 半屏行为变更
