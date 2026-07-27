# build.bat 构建脚本 + exe 旁配置定位 — 设计文档

日期：2026-07-28
状态：已确认

## 目标

1. 提供 Windows 构建脚本 `build.bat`，一键产出可分发的 `bin/` 目录
2. `bin/mdview.exe` 无论在哪个目录下运行，读写的 `config.toml` 与
   `md-styles/` 都固定在 exe 所在目录（与 cwd 解耦）

## 已确认的决策

- 只做 `build.bat`，`build.sh` 留待以后
- 用户主题 `md-styles/` 查找也改为「只看 exe 旁」，与 config 行为一致
- `bin/config.toml` 初始内容为完整注释模板
- 开发态（`cargo run` / target 下的 exe）接受去 target 旁找 config 的行为，
  不做 cwd 兜底
- `current_exe()` 失败时回退 cwd（兜底不 panic，实际几乎不会发生）

## 设计

### 1. `build.bat`（仓库根目录，新增）

用法：

```
build.bat              release 构建（默认）
build.bat -d|--debug   debug 构建
build.bat -h|--help    打印帮助后退出，不构建
其他参数               报错 + 打印帮助，非零码退出
```

流程：

1. 解析参数（仅取第一个参数，`-d`/`--debug`/`-h`/`--help`/空）
2. 调用 `.cargo-vc.bat build`（debug）或 `.cargo-vc.bat build --release`
   （release）；构建失败则以非零码退出，不产出半成品 bin
3. 在仓库根创建 `bin/`，拷贝对应 `target\{release|debug}\mdview.exe`
   （每次覆盖）
4. `bin\config.toml` **不存在时**写入注释模板（已存在则不覆盖，保护用户
   在 bin 里的修改）：

```toml
# mdview configuration
# Theme: builtin name (gruvbox-dark, nord, dracula, ...) or a css file in md-styles/
theme = "gruvbox-dark"

# Max content width in columns (comment out for terminal width)
# max_width = 100

# Enable mouse capture in the TUI
# mouse = true
```

5. `bin\md-styles\` 不存在则创建空目录（已存在则保留内容）

### 2. 配置定位（`src/config.rs`）

- 新增内部辅助 `config_path() -> PathBuf`：`current_exe()` 所在目录
  + `config.toml`；`current_exe()` 失败回退 `PathBuf::from("config.toml")`
  （cwd）
- `Config::load` / `Config::save_theme` 改用 `config_path()`，逻辑不变
- `save_theme_to` 及其测试不受影响（路径由调用方注入）

### 3. 主题目录定位（`src/style/scheme.rs`）

- theme 目录列表由 `[cwd/md-styles, exe旁/md-styles]` 改为
  `[exe旁/md-styles]`；`current_exe()` 失败同样回退 cwd 相对路径
- 内置主题回退链不变

### 4. `.gitignore`

新增 `/bin/`。已有 `/config.toml`、`/md-styles/` 条目保留不动。

### 5. 文档

- `AGENTS.md`：Conventions 中「Config: `./config.toml`」与主题查找
  「cwd → exe-adjacent → builtin」的描述更新为 exe 旁定位；Build & Test
  一节补充 `build.bat` 用法

## 验证

- `cmd //c ".cargo-vc.bat test"` 全量测试通过、零警告
- 手动：`build.bat` 后切换到其他目录运行 `bin\mdview.exe`，通过主题选择器
  换主题，确认写入的是 `bin\config.toml` 而非 cwd
- `build.bat -d`、`-h`、非法参数的行为各验证一次

## 范围外（YAGNI）

- `build.sh` / Unix 支持
- 多层级配置合并、cwd 兜底查找
- `md-styles/` 内置示例 css、bin 打包压缩
