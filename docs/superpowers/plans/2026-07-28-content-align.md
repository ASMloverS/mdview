# 内容对齐（居中/左对齐）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 支持 `align = "left" | "center"` 配置、`--align` CLI 参数和阅读器内 `a` 键切换，让内容可以左对齐展示；默认仍为居中。

**Architecture:** 居中靠 `render_document(..., offset)` 在行首插空格实现，布局引擎零改动。新增 `ContentAlign` 枚举与统一的 `content_offset` helper，收敛三处重复的居中公式（管道、启动直开、阅读器每帧）；`a` 键翻转 `app.align` 后由阅读器 draw 中既有的 offset 比较自动触发重排版。

**Tech Stack:** Rust、clap（ValueEnum）、toml、ratatui/crossterm；Windows/MSVC 下构建测试一律用 `cmd //c ".cargo-vc.bat test"`。

**关键事实（已勘察）：**

- 居中公式三处重复：`src/main.rs:65`（管道，`term` 已减 2）、`src/app.rs:240`（启动直开）、`src/ui/reader.rs:17`（每帧重算）。
- `render_document` 签名不变；`src/render/layout/mod.rs:78` 的填充逻辑不变。
- `App::new` 调用点仅三处：`src/app.rs:236`、`src/app.rs:468`（测试）、`src/ui/reader.rs:104`（测试）。
- `app::run` 仅 `src/main.rs:71` 调用。
- 浏览器预览 `src/ui/browser.rs:48` 本就 `offset = 0`，不动。
- 提交信息格式（强制）：`<gitmoji> <type>(<scope>): <message>`。
- 注意 `src/markdown/ir.rs` 已有表格列 `Align`，新类型必须叫 `ContentAlign`。

---

### Task 1: `ContentAlign` 类型与 `align` 配置键

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: 写失败测试**

在 `src/config.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn content_align_parse_toggle_and_as_str() {
        assert_eq!(ContentAlign::from_str("center"), Some(ContentAlign::Center));
        assert_eq!(ContentAlign::from_str("left"), Some(ContentAlign::Left));
        assert_eq!(ContentAlign::from_str("bogus"), None);
        assert_eq!(ContentAlign::Center.toggle(), ContentAlign::Left);
        assert_eq!(ContentAlign::Left.toggle(), ContentAlign::Center);
        assert_eq!(ContentAlign::Center.as_str(), "center");
        assert_eq!(ContentAlign::Left.as_str(), "left");
    }

    #[test]
    fn saves_align_preserving_other_keys() {
        let p = temp_path("align");
        std::fs::write(&p, "theme = \"nord\"\n").unwrap();
        assert!(save_key_to(&p, "align", "left").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("align").and_then(|t| t.as_str()), Some("left"));
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("nord"));
        cleanup(&p);
    }
```

同时把既有测试中的 `save_theme_to(&p, ...)` 全部改为 `save_key_to(&p, "theme", ...)`（共 4 处：`writes_new_file`、`updates_theme_and_preserves_other_keys`、`skips_write_when_theme_unchanged`、`never_clobbers_malformed_file`）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test config"`
Expected: 编译失败，`ContentAlign`、`save_key_to` 未定义。

- [ ] **Step 3: 实现**

`src/config.rs` 顶部（`Config` 结构体之前）新增：

```rust
/// 内容水平对齐方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ContentAlign {
    Center,
    Left,
}

impl ContentAlign {
    /// 容错解析配置字符串；非法值返回 None（回退默认）。
    pub fn from_str(s: &str) -> Option<ContentAlign> {
        match s {
            "center" => Some(ContentAlign::Center),
            "left" => Some(ContentAlign::Left),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ContentAlign::Center => "center",
            ContentAlign::Left => "left",
        }
    }

    pub fn toggle(&self) -> ContentAlign {
        match self {
            ContentAlign::Center => ContentAlign::Left,
            ContentAlign::Left => ContentAlign::Center,
        }
    }
}
```

`Config` 结构体加字段（放在 `mouse` 之后）：

```rust
    /// Content alignment: "center" (default) or "left".
    pub align: Option<String>,
```

`save_theme` 改为薄封装，并新增 `save_align`：

```rust
    pub fn save_theme(name: &str) {
        let _ = save_key_to(&config_path(), "theme", name);
    }

    /// Persist the content alignment into `config.toml`, preserving all
    /// other keys. Same best-effort semantics as `save_theme`.
    pub fn save_align(value: &str) {
        let _ = save_key_to(&config_path(), "align", value);
    }
```

把 `save_theme_to` 改名为 `save_key_to`，签名加 `key` 参数，函数体中两处 `"theme"` 改为 `key`：

```rust
/// Update `key` in the TOML file at `path`, creating the file if missing.
/// Returns Ok(true) when the file was written; Ok(false) when the value
/// was already current or the existing file could not be parsed (in which
/// case it is left untouched).
fn save_key_to(path: &Path, key: &str, value: &str) -> std::io::Result<bool> {
    let mut value_: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => match text.parse() {
            Ok(v) => v,
            Err(_) => return Ok(false),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(e),
    };
    let Some(table) = value_.as_table_mut() else {
        return Ok(false);
    };
    if table.get(key).and_then(|v| v.as_str()) == Some(value) {
        return Ok(false);
    }
    table.insert(key.to_string(), toml::Value::String(value.to_string()));
    let text = toml::to_string(&value_)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, text)?;
    Ok(true)
}
```

（参数名 `value` 与局部变量冲突，局部变量改名 `value_`；其余逻辑与原 `save_theme_to` 完全一致。）

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test config"`
Expected: 全部 PASS，零警告。

- [ ] **Step 5: 提交**

```bash
git add src/config.rs
git commit -m "✨ feat(config): add ContentAlign and align config key"
```

---

### Task 2: `content_offset` helper 与全链路接线

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/reader.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 写失败测试**

在 `src/app.rs` 的 `mod tests` 中追加（并先把 `test_app` 里 `App::new(scheme, ColorLevel::True, 100)` 改为 `App::new(scheme, ColorLevel::True, 100, ContentAlign::Center)`）：

```rust
    #[test]
    fn content_offset_centers_and_left_aligns() {
        assert_eq!(content_offset(100, 80, ContentAlign::Center), 10);
        assert_eq!(content_offset(100, 80, ContentAlign::Left), 0);
        assert_eq!(content_offset(50, 80, ContentAlign::Center), 0, "窄终端不溢出");
    }
```

`src/ui/reader.rs` 的 `test_app` 中 `App::new(scheme, ColorLevel::True, 100)` 同样加 `ContentAlign::Center` 参数。

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test app"`
Expected: 编译失败，`content_offset`、`ContentAlign` 未定义。

- [ ] **Step 3: 实现**

`src/app.rs`：

1. import 改为 `use crate::config::{Config, ContentAlign};`
2. `App` 结构体加字段 `pub align: ContentAlign,`（放在 `max_width` 之后）。
3. `App::new` 签名加 `align: ContentAlign` 参数并初始化字段。
4. `content_width` 之后新增 helper：

```rust
/// 内容水平偏移：Center 居中，Left 贴左（0）。
/// inner 为去掉边框后的可用宽度。
pub fn content_offset(inner_width: u16, width: u16, align: ContentAlign) -> u16 {
    match align {
        ContentAlign::Center => inner_width.saturating_sub(width) / 2,
        ContentAlign::Left => 0,
    }
}
```

5. `event_loop` 启动直开处（原 `let offset = term_w.saturating_sub(2).saturating_sub(width) / 2;`）改为：

```rust
        let offset = content_offset(term_w.saturating_sub(2), width, app.align);
```

6. `pub fn run(...)` 签名末尾加 `align: ContentAlign`，透传给 `event_loop`；`event_loop` 签名同样加 `align` 并传给 `App::new`。
7. `app.rs` 测试的 `test_app`：`App::new(scheme, ColorLevel::True, 100, ContentAlign::Center)`。

`src/ui/reader.rs`：

1. import 改为 `use crate::app::{content_offset, content_width, App};`，并加 `use crate::config::ContentAlign;`（测试用）。
2. 原 `let want_offset = view.width.saturating_sub(2).saturating_sub(want_width) / 2;` 改为：

```rust
    let want_offset = content_offset(view.width.saturating_sub(2), want_width, app.align);
```

3. 测试 `test_app` 的 `App::new(...)` 加 `ContentAlign::Center`。

`src/main.rs`：

1. import 改为 `use config::{Config, ContentAlign};`
2. `Cli` 加字段（放在 `max_width` 之后）：

```rust
    /// Content alignment: center (default) or left.
    #[arg(long, value_enum)]
    align: Option<ContentAlign>,
```

3. `max_width` 解析之后新增：

```rust
    let align = cli
        .align
        .or_else(|| cfg.align.as_deref().and_then(ContentAlign::from_str))
        .unwrap_or(ContentAlign::Center);
```

4. 管道模式中原 `let offset = (term - width) / 2;` 改为：

```rust
        let offset = app::content_offset(term as u16, width as u16, align) as usize;
```

5. 末尾调用改为 `app::run(cli.file, scheme, level, max_width, cfg.mouse.unwrap_or(true), align)`。

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全量 PASS，零警告。

- [ ] **Step 5: 提交**

```bash
git add src/app.rs src/ui/reader.rs src/main.rs
git commit -m "✨ feat(app): thread ContentAlign through reader, pipe mode and CLI"
```

---

### Task 3: 阅读器 `a` 键切换 + 帮助面板

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: 写失败测试**

在 `src/app.rs` 的 `mod tests` 中追加：

```rust
    #[test]
    fn toggle_align_flips_and_sets_status() {
        let mut app = test_app(10, 24);
        assert_eq!(app.align, ContentAlign::Center);
        app.toggle_align();
        assert_eq!(app.align, ContentAlign::Left);
        assert_eq!(app.status.as_deref(), Some("align: left"));
        app.toggle_align();
        assert_eq!(app.align, ContentAlign::Center);
        assert_eq!(app.status.as_deref(), Some("align: center"));
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test app"`
Expected: 编译失败，`toggle_align` 未定义。

- [ ] **Step 3: 实现**

`src/app.rs`：

1. `impl App` 中（`apply_scheme` 之后）新增：

```rust
    /// 切换居中/左对齐并提示；重排版由 draw 的 offset 比较自动触发。
    pub fn toggle_align(&mut self) {
        self.align = self.align.toggle();
        self.status = Some(format!("align: {}", self.align.as_str()));
    }
```

2. `reader_key` 的 match 中（`KeyCode::Char('N')` 行之后）新增：

```rust
        KeyCode::Char('a') => {
            app.toggle_align();
            Config::save_align(app.align.as_str());
        }
```

（持久化放按键处理处而非 `toggle_align` 内，保持纯方法可无 IO 单测——与 `close_picker` 的模式一致。）

`src/ui/mod.rs`：`draw_help` 的 `keys` 数组中 `("t", "theme picker"),` 之后加一行：

```rust
        ("a", "toggle align (reader)"),
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全量 PASS，零警告。

- [ ] **Step 5: 提交**

```bash
git add src/app.rs src/ui/mod.rs
git commit -m "✨ feat(reader): toggle content alignment with 'a'"
```

---

### Task 4: 文档与分发配置

**Files:**
- Modify: `build.bat:44-55`（`:write_config` 段）
- Modify: `bin/config.toml`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: 更新 `build.bat` 与 `bin/config.toml`**

`build.bat` 的 `:write_config` 段在 `# max_width` 两行之后、`# mouse` 之前插入：

```bat
>> "%CFG%" echo # Content alignment: center (default) or left; toggle in reader with 'a'
>> "%CFG%" echo # align = "center"
>> "%CFG%" echo.
```

`bin/config.toml`（已存在，build.bat 不会覆盖）追加：

```toml
# Content alignment: center (default) or left; toggle in reader with 'a'
# align = "center"
```

- [ ] **Step 2: 更新 `README.md`**

- 第 31 行 Options 改为：
  ``Options: `-t, --theme <name>` · `-w, --max-width <cols>` (default 100) · `--align <center|left>` · `--list-themes` ``
- 第 33 行 Keys 在 `` `t` themes `` 后加 `` · `a` align (reader) ``。
- Config 代码块（第 45-49 行）改为：

  ```toml
  theme = "gruvbox-dark"  # written automatically when the picker closes
  max_width = 100
  mouse = true
  align = "center"        # or "left"; written automatically on 'a' toggle
  ```

- 第 51 行优先级说明改为：`Priority: CLI flags > config.toml > builtin defaults (flags are one-shot, never persisted).`

- [ ] **Step 3: 更新 `README.zh-CN.md`**

- 第 39 行选项改为：`选项：-t, --theme <名称>`、`-w, --max-width <列数>`（默认 100）、`--align <center|left>`、`--list-themes`。`
- 按键表（第 50 行 `t` 行之后）加一行：`| \`a\` | 切换内容居中/左对齐（阅读器，自动保存） |`
- 配置代码块（第 69-73 行）改为：

  ```toml
  theme = "gruvbox-dark"  # 主题选择器关闭时自动写入
  max_width = 100
  mouse = true
  align = "center"        # 或 "left"；阅读器内按 a 切换时自动写入
  ```

- 第 75 行优先级说明改为：`优先级：CLI 参数 > config.toml > 内置默认（CLI 参数为一次性覆盖，不落盘）。`

- [ ] **Step 4: 更新 `AGENTS.md`**

Conventions 中的 Config 条目改为：

```
- Default theme: `gruvbox-dark`. Config: `config.toml` next to the
  executable (theme persisted on picker close, `align` persisted on
  reader `a` toggle; preserve other keys when writing).
```

- [ ] **Step 5: 全量测试 + 提交**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 全量 PASS，零警告。

```bash
git add build.bat bin/config.toml README.md README.zh-CN.md AGENTS.md
git commit -m "📝 docs: document align config, --align flag and 'a' key"
```

---

## 自审记录

- **规格覆盖**：默认居中 ✓（默认回退 `ContentAlign::Center`，Task 2）；config.toml `align` 键 ✓（Task 1）；`--align` CLI ✓（Task 2）；管道模式生效 ✓（Task 2 Step 3.4）；仅阅读器 `a` 键 ✓（Task 3，`reader_key`）；切换即持久化 ✓（Task 3 Step 3.2）；帮助面板 ✓（Task 3）；浏览器预览不动 ✓（无任务触碰 `ui/browser.rs`）；布局引擎零改动 ✓。
- **占位符扫描**：无 TBD/TODO；所有代码步骤含完整代码。
- **类型一致性**：`ContentAlign::{Center, Left}`、`from_str/as_str/toggle`、`save_key_to(path, key, value)`、`content_offset(inner_width: u16, width: u16, align)`、`App::new(scheme, level, max_width, align)`、`run(..., mouse, align)`、`toggle_align()` 在各 Task 间一致；Task 1 的测试在 Task 1 Step 3 定义的类型上编译，Task 2/3 引用的类型均已在 Task 1 定义。
