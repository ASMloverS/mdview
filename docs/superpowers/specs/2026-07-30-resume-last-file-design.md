# 无参数启动恢复上次文件 — 设计

日期：2026-07-30
状态：已确认

## 背景

现状：无参数启动 mdview（stdin 为终端）直接进入文件浏览器（`Mode::Browser`）。
目标：无参数启动时默认打开上次退出时阅读的文件，恢复上次会话状态；
首次使用（无历史）时提示用户如何传入文件。

## 已确认的决策

| 问题 | 结论 |
| --- | --- |
| 空历史（首次使用） | 进文件浏览器 + 弹出式提示 |
| 最近文件已失效 | 清理失效条目后沿 MRU 重试，找到第一个有效文件即停 |
| `history_size = 0` | 按空历史处理，但**静默**进浏览器（不弹窗） |
| 光标位置 | 恢复到上次光标行（复用 `open_reader` 现有逻辑） |
| 弹窗形态 | 居中 overlay，仅启动时显示一次，任意键关闭 |
| 弹窗文案 | 英文，与现有 UI 语言一致 |
| 显式浏览器入口 | 不加新 CLI 参数，阅读器内 `Esc` 回浏览器即可 |
| 全部历史失效 | 清空 `history.toml` 并写盘，按空历史处理（弹窗 + 浏览器） |
| 文件存在但读取失败 | 同失效条目处理（跳过、剔除、试下一条） |

## 方案

方案 A：恢复逻辑收进 `History` + `App` 启动分支。

### 1. `History::latest_valid()`（`src/history.rs`）

```rust
/// 最近一个仍可打开的文件：从 MRU 头部遍历，剔除不存在/不可读
/// 的条目（立即写盘），返回第一个可用条目。全部失效则清空历史。
pub fn latest_valid(&mut self) -> Option<PathBuf>
```

- 有效性判定：`std::fs::read_to_string` 试读成功（同时覆盖"不存在"与
  "权限/编码读取失败"两种情况）。
- 遇到第一个有效条目即停，只剔除它前面的失效条目并写盘。
- 全部失效：清空历史并写盘，返回 `None`。
- 返回有效条目的路径，可直接用于 `open_reader`。

### 2. 启动分支（`src/app.rs`，`event_loop` 中 `App::new` 之后）

现有逻辑：`start_file` 为 `Some` 时直接 `open_reader`。新增：

- `start_file` 为 `None` 且 `history_size > 0`：调用 `history.latest_valid()`
  - `Some(path)` → `open_reader(path, ...)`；光标恢复由 `open_reader`
    现有逻辑（`src/app.rs:106-113`）自动完成，无需额外代码。
  - `None` → 保持浏览器模式，置 `resume_hint = true`。
- `history_size == 0` → 不查历史、不弹窗，静默进浏览器。

### 3. 弹窗（`src/app.rs` + `src/ui/`）

- `App` 新增字段 `resume_hint: bool`，默认 `false`，仅上述空历史分支置位。
- 渲染：复用 help/picker 的居中 overlay 绘制方式。文案三行：

  ```
  No recent file to resume.
  Open one directly: mdview <file>
  Press any key to browse.
  ```

- 按键：`resume_hint` 为 `true` 时任意键关闭（拦截模式同 `show_help`，
  见 `src/app.rs:399-406`），本次会话内不再出现。

### 4. 不受影响的行为

- 传文件参数启动、pipe 模式（stdin 非终端）、`--list-themes`：完全不变。
- 阅读器内 `Esc` 回浏览器、`q` 退出：不变；下次无参数启动仍恢复该文件
  （符合"回到上次退出状态"语义）。

### 5. 测试（in-file `#[cfg(test)]` 模式）

- `history.rs`：
  - `latest_valid` 跳过头部失效条目、剔除并写盘、返回第一个有效文件
  - 全部失效时清空历史并返回 `None`
  - 空历史返回 `None`
- `app.rs`：
  - 无 `start_file` 且有有效历史 → 启动进阅读器，光标恢复到记录行
  - 历史为空/全失效 → 进浏览器且 `resume_hint == true`
  - `history_size = 0` → 静默浏览器（`resume_hint == false`）
  - 任意键关闭弹窗后 `resume_hint == false`

### 6. 文档同步

- `README.md` / `README.zh-CN.md` 中"无参数打开文件浏览器"的描述需更新为
  新行为。
