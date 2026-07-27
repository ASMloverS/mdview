# build.bat + exe 旁配置定位 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增 `build.bat` 一键产出 `bin/` 分发目录，并将 `config.toml` / `md-styles/` 的定位改为 exe 所在目录（与 cwd 解耦）。

**Architecture:** `Config::load`/`save_theme` 与 `style_dirs()` 统一改为 `current_exe()` 旁路径（失败回退 cwd 相对路径）；`build.bat` 解析 `-d/--debug`/`-h/--help`，调用 `.cargo-vc.bat` 构建后组装 `bin/`（exe 覆盖、config 不覆盖、md-styles 保留）。

**Tech Stack:** Rust、Windows batch、MSVC（经 `.cargo-vc.bat` 包装）。

**Spec:** `docs/superpowers/specs/2026-07-28-build-script-exe-local-config-design.md`

**注意：** Windows/MSVC 下所有 cargo 命令必须经包装器执行（Git Bash 中
`cmd //c ".cargo-vc.bat test"`），plain `cargo` 会误用 Git 的 `link.exe`。

---

### Task 1: `src/config.rs` — config.toml 定位到 exe 旁

**Files:**
- Modify: `src/config.rs`（模块文档第 1 行、`Config::load` 第 17-23 行、`Config::save_theme` 第 25-30 行；文件末尾 tests 模块内新增测试）

- [ ] **Step 1: 写失败测试**

在 `src/config.rs` 的 `#[cfg(test)] mod tests` 中追加：

```rust
    #[test]
    fn config_path_is_next_to_executable() {
        let p = config_path();
        assert_eq!(p.file_name().and_then(|n| n.to_str()), Some("config.toml"));
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(p.parent().unwrap(), exe_dir);
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test config_path"`
Expected: 编译失败，`cannot find function 'config_path' in this scope`

- [ ] **Step 3: 实现 `config_path()` 并改用它**

1. 模块文档第 1 行改为：

```rust
//! Local configuration: `config.toml` next to the executable.
```

2. `impl Config` 上方（`save_theme_to` 之前）新增：

```rust
/// Path of `config.toml` next to the executable; falls back to the
/// cwd-relative path when the exe location is unavailable.
fn config_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("config.toml")))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}
```

3. `Config::load` 改为（文档注释同步更新）：

```rust
    /// Load `config.toml` next to the executable; missing or invalid
    /// files yield defaults.
    pub fn load() -> Config {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }
```

4. `Config::save_theme` 改为（文档注释同步更新）：

```rust
    /// Persist the selected theme into `config.toml` next to the
    /// executable, preserving all other keys. Best-effort: IO errors are
    /// ignored and a malformed existing file is never overwritten.
    pub fn save_theme(name: &str) {
        let _ = save_theme_to(&config_path(), name);
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test config"`
Expected: `config_path_is_next_to_executable` 及原有 4 个 config 测试全部 PASS，零警告

- [ ] **Step 5: 提交**

```bash
git add src/config.rs
git commit -m "✨ feat(config): resolve config.toml next to executable"
```

---

### Task 2: `src/style/scheme.rs` — md-styles 只看 exe 旁

**Files:**
- Modify: `src/style/scheme.rs`（`style_dirs` 第 41-51 行、`Scheme::load` 文档第 98-101 行、`Scheme::available` 文档第 121-122 行；tests 模块内新增测试）

- [ ] **Step 1: 写失败测试**

在 `src/style/scheme.rs` 的 tests 模块中追加：

```rust
    #[test]
    fn style_dirs_only_next_to_executable() {
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert_eq!(style_dirs(), vec![exe_dir.join("md-styles")]);
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cmd //c ".cargo-vc.bat test style_dirs"`
Expected: FAIL — 当前 `style_dirs()` 返回 2 个目录且第一个是 cwd 相对的 `md-styles`

- [ ] **Step 3: 修改 `style_dirs()` 及相关文档注释**

1. `style_dirs`（含文档注释）改为：

```rust
/// User CSS theme directory: `md-styles` next to the executable
/// (cwd-relative fallback when the exe location is unavailable).
fn style_dirs() -> Vec<PathBuf> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    vec![dir.join("md-styles")]
}
```

（`unwrap_or_default()` 得空路径，`join("md-styles")` 后即为 cwd 相对回退。）

2. `Scheme::load` 文档注释第 98-101 行改为：

```rust
    /// Resolve a scheme by name: user `md-styles/<name>.css` next to the
    /// executable first, then builtins. A user file with the same name as
    /// a builtin overrides the builtin. Unknown names fall back to default.
```

3. `Scheme::available` 文档注释第 121-122 行改为：

```rust
    /// All loadable scheme names: builtins plus user CSS files in
    /// `md-styles/` next to the executable.
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cmd //c ".cargo-vc.bat test scheme"`
Expected: `style_dirs_only_next_to_executable` 及 scheme 全部既有测试 PASS
（`user_dirs_priority_and_fallback` 测的是注入目录的 `load_from_dirs`/`available_in`，不受影响），零警告

- [ ] **Step 5: 提交**

```bash
git add src/style/scheme.rs
git commit -m "✨ feat(theme): look up md-styles only next to executable"
```

---

### Task 3: `build.bat` — 构建脚本

**Files:**
- Create: `build.bat`（仓库根目录）

- [ ] **Step 1: 创建 `build.bat`**

完整内容（注意 `>> "%CFG%" echo ...` 重定向前置写法是有意的：避免
`100>>` 被 cmd 解析成 fd 重定向，也避免行尾空格）：

```bat
@echo off
setlocal

if /i "%~1"=="-h" goto help
if /i "%~1"=="--help" goto help
if "%~1"=="" set "PROFILE=release" & goto build
if /i "%~1"=="-d" set "PROFILE=debug" & goto build
if /i "%~1"=="--debug" set "PROFILE=debug" & goto build
echo Unknown option: %~1 1>&2
call :print_help
exit /b 1

:help
call :print_help
exit /b 0

:build
if "%PROFILE%"=="release" (
    call "%~dp0.cargo-vc.bat" build --release
) else (
    call "%~dp0.cargo-vc.bat" build
)
if errorlevel 1 exit /b 1

if not exist "%~dp0bin\" mkdir "%~dp0bin"
copy /y "%~dp0target\%PROFILE%\mdview.exe" "%~dp0bin\mdview.exe" >NUL || exit /b 1
if not exist "%~dp0bin\config.toml" call :write_config
if not exist "%~dp0bin\md-styles\" mkdir "%~dp0bin\md-styles"
echo Built %~dp0bin\mdview.exe [%PROFILE%]
exit /b 0

:print_help
echo Usage: build.bat [option]
echo.
echo   (no option)    Release build (default)
echo   -d, --debug    Debug build
echo   -h, --help     Show this help and exit
echo.
echo Output: bin\mdview.exe + bin\config.toml + bin\md-styles\
goto :eof

:write_config
set "CFG=%~dp0bin\config.toml"
> "%CFG%" echo # mdview configuration
>> "%CFG%" echo # Theme: builtin name (gruvbox-dark, nord, dracula, ...) or a css file in md-styles/
>> "%CFG%" echo theme = "gruvbox-dark"
>> "%CFG%" echo.
>> "%CFG%" echo # Max content width in columns (comment out for terminal width)
>> "%CFG%" echo # max_width = 100
>> "%CFG%" echo.
>> "%CFG%" echo # Enable mouse capture in the TUI
>> "%CFG%" echo # mouse = true
goto :eof
```

- [ ] **Step 2: 验证 help 与非法参数**

Run: `cmd //c "build.bat -h"`
Expected: 打印 Usage 帮助，退出码 0，不触发构建

Run: `cmd //c "build.bat --bogus"; echo "exit=$?"`
Expected: 打印 `Unknown option: --bogus` + 帮助，`exit=1`

- [ ] **Step 3: 验证 release 构建产物**

Run: `cmd //c "build.bat"`（构建较慢，timeout 设 300s）
Expected: 退出码 0；`bin/mdview.exe`、`bin/config.toml`、`bin/md-styles/` 均存在；
`bin/config.toml` 内容与上面模板逐行一致

- [ ] **Step 4: 验证不覆盖已有 config**

Run: `echo theme = "nord" > bin/config.toml && cmd //c "build.bat" && cat bin/config.toml`
Expected: 输出仍为 `theme = "nord"`（未被模板覆盖）

- [ ] **Step 5: 验证 debug 构建**

Run: `cmd //c "build.bat -d"`（timeout 300s）
Expected: 退出码 0；`bin/mdview.exe` 被替换为 `target/debug/mdview.exe`
（比对 `ls -l bin/mdview.exe target/debug/mdview.exe` 大小一致）

- [ ] **Step 6: 提交**

```bash
git add build.bat
git commit -m "✨ feat(build): add build.bat producing bin/ distribution"
```

---

### Task 4: `.gitignore` + `AGENTS.md` 文档同步

**Files:**
- Modify: `.gitignore`（第 23-25 行项目段）
- Modify: `AGENTS.md`（Build & Test 第 5-12 行；Conventions 第 36-40 行）

- [ ] **Step 1: `.gitignore` 项目段加 `/bin/`**

将：

```
# Project: per-machine local state (written at runtime)
/config.toml
/md-styles/
```

改为：

```
# Project: per-machine local state (written at runtime)
/config.toml
/md-styles/
/bin/
```

- [ ] **Step 2: `AGENTS.md` Build & Test 补充 build.bat**

在第 10 行 `` - `cmd //c ".cargo-vc.bat build"` `` 之后插入：

```
- Distribution build: `build.bat` (release) / `build.bat -d` (debug) —
  assembles `bin/` (mdview.exe + config.toml + md-styles/).
```

- [ ] **Step 3: `AGENTS.md` Conventions 更新定位规则**

将第 36-40 行：

```
- Themes: builtins live in `assets/styles/*.css`; user themes in
  `md-styles/*.css` (lookup: cwd → exe-adjacent → builtin). CSS subset:
  element/descendant selectors, color/background/border/font properties only.
- Default theme: `gruvbox-dark`. Config: `./config.toml` (theme persisted on
  picker close; preserve other keys when writing).
```

改为：

```
- Themes: builtins live in `assets/styles/*.css`; user themes in
  `md-styles/*.css` next to the executable (lookup: exe-adjacent →
  builtin). CSS subset: element/descendant selectors,
  color/background/border/font properties only.
- Default theme: `gruvbox-dark`. Config: `config.toml` next to the
  executable (theme persisted on picker close; preserve other keys when
  writing).
```

- [ ] **Step 4: 提交**

```bash
git add .gitignore AGENTS.md
git commit -m "📝 docs(agents): exe-local config/theme lookup + build.bat usage"
```

---

### Task 5: 全量测试 + 端到端验证

**Files:** 无改动，仅验证。

- [ ] **Step 1: 全量测试 + 零警告**

Run: `cmd //c ".cargo-vc.bat test"`（timeout 300s）
Expected: 全部测试 PASS，无 warning

- [ ] **Step 2: 验证 exe 旁定位（跨目录运行）**

Run（Git Bash，仓库根）:

```bash
cmd //c "build.bat"
mkdir -p /tmp/mdview-e2e && cd /tmp/mdview-e2e
"C:/Workspace/Repositories/mdview/bin/mdview.exe" "C:/Workspace/Repositories/mdview/README.md" | head -5
ls config.toml 2>&1
```

Expected: README 正常渲染输出；`ls: cannot access 'config.toml'`（cwd 下不生成
config）；且运行前后 `bin/config.toml` 无变化（pipe 模式不写配置）

- [ ] **Step 3: 交回用户做 TUI 手动验证**

提示用户：在其他目录运行 `bin\mdview.exe <file>` 进入 TUI，按 `t` 换主题并
关闭选择器，确认写入的是 `bin\config.toml` 而非 cwd。
