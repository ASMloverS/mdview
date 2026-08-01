# 目录树浏览器设计

日期：2026-08-01
状态：已确认

## 背景与目标

现有浏览器（`src/app.rs` 的 `scan_files`）递归扫描**启动时 cwd** 下的所有
markdown 文件，平铺展示，无法浏览其他目录，Windows 下无法跨驱动器。

目标：把文件列表改为**目录树浏览器**——单层按需加载，可逐级进入任意目录；
Windows 越过盘符根时出现虚拟驱动器列表，实现跨驱动器；Linux 可浏览整个
文件系统。顺带修正 Enter/l 等按键提示中的错误与遗漏。

## 需求决策（已与用户确认）

- 交互模型：目录树浏览器，替换现有递归平铺列表（`scan_files` 删除）。
- 列表内容：只显示子目录 + `.md`/`.markdown` 文件；隐藏条目（`.` 开头）
  默认隐藏。
- 排序：目录优先，各自按名称排序（大小写不敏感）。
- 选中目录时右栏预览：显示统计信息（子目录数、md 文件数，单层统计不递归）。
- Windows 驱动器列表：在盘符根再向上时自动出现，无需专用按键。
- 启动位置：打开了文件（CLI 参数或 history 恢复）→ 浏览器初始定位到该
  文件所在目录并选中该文件；未打开文件 → 从启动 cwd 开始。
- Esc 从阅读器返回：定位到刚读文件所在目录并选中该文件（reveal）。
- 不做路径输入跳转（YAGNI）。
- 无权限/读取失败目录：停留原位，状态栏提示错误。
- `r` 键语义：从「递归重扫」改为「刷新当前目录」。
- 路径显示：左栏边框标题显示当前位置完整路径。
- 打开/进入键：**仅 `o`**（`Enter`/`l`/`→` 在浏览器中不再绑定）；
  返回上级：**仅 `←` 和 `Backspace`**（`h` 不绑定）。

## 架构

新建 `src/browse.rs` 纯逻辑模块，与 App/UI 解耦，可独立单测：

```rust
/// 浏览位置：具体目录，或 Windows 虚拟驱动器列表层
pub enum Loc { Dir(PathBuf), Drives }

/// 列表条目
pub enum Entry {
    Dir(PathBuf),   // 子目录（含驱动器层的 C:\ D:\ ...）
    File(PathBuf),  // .md / .markdown 文件
}

pub struct Browser {
    pub loc: Loc,
    pub entries: Vec<Entry>,  // 目录优先，名称排序（大小写不敏感）
    pub selected: usize,
}
```

核心函数：

- `Browser::new(cwd: &Path)` — 从 cwd 加载。
- `Browser::reveal(&mut self, file: &Path)` — 切到文件所在目录并选中该文件；
  失败（无权限/目录已删）回退到当前位置，由调用方提示。启动时打开了文件
  也复用此逻辑初始化浏览器。
- `load(loc) -> io::Result<Vec<Entry>>` — 单层读取、过滤、排序；
  `Loc::Drives` 下枚举驱动器（A–Z 探测 `exists()`，零新依赖）。
- `enter(&mut self) -> Enter` — 选中目录 → 进入并重载；选中文件 → 返回
  文件路径交给 App 打开阅读器；失败返回错误信息。
- `up(&mut self)` — 返回父目录，选中定位到刚离开的子目录；Windows 盘符根
  （`parent() == None`）再向上 → `Loc::Drives`；`Drives` 再向上无操作；
  Linux 到 `/` 为止。
- `refresh(&mut self)` — 重读当前层，选中项按名称尽量保留。

`App` 侧：`files: Vec<PathBuf>` 与 `selected` 字段替换为 `browser: Browser`；
`scan_files` 删除。启动流程：`event_loop` 中 `open_reader`（CLI 文件或
`resume_latest`）之后调用 `browser.reveal(path)`；未打开文件则
`Browser::new(cwd)`。阅读器 Esc 返回时 `browser.reveal(&reader.path)`。

## 按键

浏览器（`browser_key`）：

| 按键 | 行为 |
|---|---|
| `j/k`、`↓/↑` | 移动选中 |
| `o` | 打开文件 / 进入目录 / 进入驱动器（唯一打开键） |
| `←`、`Backspace` | 返回上级（越根出驱动器列表） |
| `r` | 刷新当前目录 |
| `t` | 主题选择器（不变） |
| `q`、`Esc` | 退出（不变） |

阅读器按键不变。

## UI（`src/ui/browser.rs`）

- 左栏标题：当前位置完整路径（过长左侧截断保留尾部）；`Loc::Drives` 显示
  ` drives `。
- 条目：目录 `▸ name/`，文件纯名称，驱动器 `C:\` 形式；选中高亮不变。
- 右栏预览：文件 → 现有渲染预览（不变）；目录 → 统计信息
  （`N subdirs, M markdown files`）；`Loc::Drives` → 提示文本。
- 空目录：`no markdown files or subdirectories here`。

## 提示修正（排查结论）

排查发现（修正前状态）：

1. `src/app.rs:456` 浏览器中 `Enter`/`l`/`→` 均可打开文件，但帮助浮层只写
   `Enter, l`、状态栏只写 `Enter open` —— `→` 未提及。
2. 帮助浮层 `("Esc", "back to browser")` 有歧义：浏览器中 `Esc` 实为退出。

随按键重绑一并修正：

- 帮助浮层（`src/ui/mod.rs`）：`("o", "open file / enter dir")`、
  `("←, Bksp", "parent directory")`、`("Esc", "back (reader) / quit (browser)")`。
- 状态栏（`src/ui/browser.rs`）：
  `"o open · ←/Bksp up · r refresh · t theme · ? help"`。
- `README.md` / `README.zh-CN.md` 按键表同步更新。

## 错误处理

进入/刷新目录失败 → 停留原位，`app.status` 显示
`cannot read <path>: <err>`；`reveal` 失败回退浏览器当前位置并提示。

## 测试（in-file `#[cfg(test)]`，临时目录构造）

- `load`：目录优先排序、大小写不敏感、隐藏条目过滤、只收 md 文件。
- `up`：父目录返回后选中定位；Windows 盘符根 → `Drives`、`Drives` 再向上
  无操作（`#[cfg(windows)]`）。
- `enter`：目录成功进入；文件返回路径；不可读目录报错且不移动。
- `reveal`：定位到文件所在目录并选中；失败回退。
- 按键层：`o`/`←`/`Backspace` 行为；`Enter`/`l`/`→`/`h` 不再触发任何动作。
