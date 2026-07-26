# mdview 默认主题 gruvbox-dark + 选择持久化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 默认主题改为 gruvbox-dark；选择器关闭时把当前主题持久化到 `./config.toml`（保留其他字段），下次启动生效。

**Architecture:** `config.rs` 增加基于 `toml::Value` 的读改写保存函数；`app.rs` 选择器关闭分支统一走 `close_picker()`；`scheme.rs` 改 `DEFAULT_THEME` 常量。

**Tech Stack:** Rust 2021, toml 0.8（已有依赖）, serde。

**Spec:** `docs/superpowers/specs/2026-07-26-mdview-theme-persistence-design.md`

**构建/测试命令（Windows，必须走 .cargo-vc.bat）：**

```bash
cmd //c ".cargo-vc.bat test"          # 全部测试
cmd //c ".cargo-vc.bat test <name>"   # 单个测试
```

**Baseline:** master 分支，31 个测试全绿，0 警告。工作分支 `feature/default-theme-persistence`。

**实现者背景（无需再读其他文件）：**
- `src/config.rs` 现状：`Config { theme: Option<String>, max_width: Option<usize>, mouse: Option<bool> }`（derive Debug/Clone/Default/Deserialize），`Config::load()` 读 `./config.toml`，失败给默认值。
- `src/app.rs` 选择器处理（handle_key 内，约 296-315 行）：`if let Some(sel) = app.picker { match key.code { KeyCode::Esc | KeyCode::Char('t') => app.picker = None, j/Down 与 k/Up 两个导航分支, KeyCode::Enter => app.picker = None, _ => {} } return; }`。app.rs 当前**没有** `use crate::config::Config;`（main.rs 有）。
- `src/style/scheme.rs`：`pub const DEFAULT_THEME: &str = "tokyo-night";`（约 38 行）。
- 注意：layout 测试通过 `Scheme::load(DEFAULT_THEME)` 构造主题并断言 `pre`/`blockquote` 背景存在——gruvbox-dark 两者都有，这些测试应继续通过。

---

### Task 1: 主题持久化 + 默认 gruvbox-dark

**Files:**
- Modify: `src/config.rs`（保存函数 + 测试模块）
- Modify: `src/app.rs`（close_picker + import）
- Modify: `src/style/scheme.rs`（DEFAULT_THEME + 测试）

- [ ] **Step 1: 写失败测试**

`src/config.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mdview-cfg-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    fn cleanup(p: &std::path::Path) {
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[test]
    fn writes_new_file() {
        let p = temp_path("new");
        assert!(save_theme_to(&p, "nord").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("nord"));
        cleanup(&p);
    }

    #[test]
    fn updates_theme_and_preserves_other_keys() {
        let p = temp_path("keep");
        std::fs::write(&p, "theme = \"nord\"\nmax_width = 80\nmouse = false\ncustom = \"x\"\n").unwrap();
        assert!(save_theme_to(&p, "dracula").unwrap());
        let v: toml::Value = std::fs::read_to_string(&p).unwrap().parse().unwrap();
        assert_eq!(v.get("theme").and_then(|t| t.as_str()), Some("dracula"));
        assert_eq!(v.get("max_width").and_then(|t| t.as_integer()), Some(80));
        assert_eq!(v.get("mouse").and_then(|t| t.as_bool()), Some(false));
        assert_eq!(v.get("custom").and_then(|t| t.as_str()), Some("x"));
        cleanup(&p);
    }

    #[test]
    fn skips_write_when_theme_unchanged() {
        let p = temp_path("same");
        let original = "theme = \"nord\"\nmax_width = 80\n";
        std::fs::write(&p, original).unwrap();
        assert!(!save_theme_to(&p, "nord").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        cleanup(&p);
    }

    #[test]
    fn never_clobbers_malformed_file() {
        let p = temp_path("bad");
        let original = "theme = [not valid toml";
        std::fs::write(&p, original).unwrap();
        assert!(!save_theme_to(&p, "nord").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        cleanup(&p);
    }
}
```

`src/style/scheme.rs` 的 `mod tests` 中追加：

```rust
#[test]
fn default_theme_is_gruvbox_dark() {
    assert_eq!(DEFAULT_THEME, "gruvbox-dark");
    assert!(!Scheme::load(DEFAULT_THEME).rules.is_empty());
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: 编译错误（`save_theme_to` 不存在）+ `default_theme_is_gruvbox_dark` 断言失败（当前是 tokyo-night）

- [ ] **Step 3: 实现**

`src/config.rs` 改为（完整替换文件内容，Config 结构与 load 不变）：

```rust
//! Local configuration: `config.toml` in the tool's working directory.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// Theme name: builtin or `md-styles/<name>.css`.
    pub theme: Option<String>,
    /// Max content width in columns.
    pub max_width: Option<usize>,
    /// Enable mouse capture in the TUI.
    pub mouse: Option<bool>,
}

impl Config {
    /// Load `./config.toml`; missing or invalid files yield defaults.
    pub fn load() -> Config {
        std::fs::read_to_string("config.toml")
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Persist the selected theme into `./config.toml`, preserving all
    /// other keys. Best-effort: IO errors are ignored and a malformed
    /// existing file is never overwritten.
    pub fn save_theme(name: &str) {
        let _ = save_theme_to(Path::new("config.toml"), name);
    }
}

/// Update the `theme` key in the TOML file at `path`, creating the file
/// if missing. Returns Ok(true) when the file was written; Ok(false)
/// when the theme was already current or the existing file could not
/// be parsed (in which case it is left untouched).
fn save_theme_to(path: &Path, name: &str) -> std::io::Result<bool> {
    let mut value: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => match text.parse() {
            Ok(v) => v,
            Err(_) => return Ok(false),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(e),
    };
    let Some(table) = value.as_table_mut() else {
        return Ok(false);
    };
    if table.get("theme").and_then(|v| v.as_str()) == Some(name) {
        return Ok(false);
    }
    table.insert("theme".to_string(), toml::Value::String(name.to_string()));
    let text = toml::to_string(&value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, text)?;
    Ok(true)
}
```

`src/style/scheme.rs`：`pub const DEFAULT_THEME: &str = "tokyo-night";` → `"gruvbox-dark"`。

`src/app.rs`：
- 顶部 import 区加 `use crate::config::Config;`
- 新增函数（放在 `reader_key` 之后即可）：

```rust
/// Close the theme picker and persist the current theme selection.
fn close_picker(app: &mut App) {
    app.picker = None;
    Config::save_theme(&app.scheme.name);
}
```

- 选择器 match 的两个关闭分支改为统一调用：
  - `KeyCode::Esc | KeyCode::Char('t') => app.picker = None,` → `KeyCode::Esc | KeyCode::Char('t') => close_picker(app),`
  - `KeyCode::Enter => app.picker = None,` → `KeyCode::Enter => close_picker(app),`

- [ ] **Step 4: 跑测试确认通过**

Run: `cmd //c ".cargo-vc.bat test"`
Expected: `test result: ok. 36 passed`（31 + 4 config + 1 scheme）；无新警告。
若 layout 测试因默认主题切换失败，报告 DONE_WITH_CONCERNS 并说明（预期不会：gruvbox-dark 的 pre/blockquote 均有背景色）。

- [ ] **Step 5: 冒烟验证持久化链路（可选但推荐）**

```bash
cmd //c ".cargo-vc.bat build"
printf 'theme = "nord"\nmax_width = 90\n' > /tmp/mdview-cfg-check.toml
```

人工推理核对（不必真跑 TUI）：启动顺序 `cli.theme.or(cfg.theme).unwrap_or(DEFAULT_THEME)` 使 config.toml 的 theme 自然生效。

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/app.rs src/style/scheme.rs
git commit -m "feat: default to gruvbox-dark; persist picker theme to config.toml"
```

---

## Self-Review 记录

- Spec 覆盖：默认主题（Step 3 scheme.rs）、save_theme_to/Config::save_theme（Step 3 config.rs）、close_picker 与三个关闭分支（Step 3 app.rs）、全部测试（Step 1）均有对应；范围外项无任务。
- 无占位符；所有代码完整给出（config.rs 为完整文件替换）。
- 类型一致性：`save_theme_to(&Path, &str) -> std::io::Result<bool>` 在测试与实现中一致；`toml::Value::Table(toml::map::Map::new())` 是 toml 0.8 的正确构造；`close_picker(&mut App)` 与调用点一致。
