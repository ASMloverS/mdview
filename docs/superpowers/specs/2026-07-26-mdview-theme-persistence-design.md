# mdview 默认主题 gruvbox-dark + 选择持久化 — 设计文档

日期：2026-07-26
状态：已确认

## 目标

1. 默认主题由 tokyo-night 改为 gruvbox-dark
2. 运行时通过选择器选定的主题持久化，下次启动生效

## 已确认的决策

- 持久化位置：`./config.toml`（cwd，与现有读取位置一致）
- 落盘时机：主题选择器关闭时（Enter/Esc/t），j/k 预览阶段不写盘

## 设计

### 1. 默认主题（`src/style/scheme.rs`）

`DEFAULT_THEME` 改为 `"gruvbox-dark"`，兜底链不变。

### 2. 持久化写入（`src/config.rs`）

- `save_theme_to(path, name) -> io::Result<bool>`（内部）：读 TOML 为
  `toml::Value`（文件不存在则空表起步）；theme 相同则跳过不写
  （Ok(false)）；文件存在但解析失败则**不覆盖**（Ok(false)）；否则更新
  theme 键写回（Ok(true)）
- `Config::save_theme(name)`（公有）：固定 `./config.toml` 路径的包装，
  错误静默忽略（best-effort）
- 用 `toml::Value` 读写，保留 `max_width`/`mouse`/未知键

### 3. 落盘时机（`src/app.rs`）

新增 `close_picker(app)`：`app.picker = None` + `Config::save_theme(&app.scheme.name)`。
选择器的三个关闭分支（Esc、Char('t')、Enter）统一改调它。

### 4. 启动顺序（不变）

`-t` 参数 > `./config.toml` 的 theme > 默认 gruvbox-dark。
`-t` 为一次性覆盖，不落盘。

### 5. 测试

- `config.rs` 新测试模块（注入临时路径）：写新文件、更新已有 theme、
  保留其他字段与未知键、相同值跳过、损坏 TOML 不覆盖
- `scheme.rs`：断言 `DEFAULT_THEME == "gruvbox-dark"` 且可加载解析

## 范围外（YAGNI）

- exe 旁/用户配置目录、多层级配置合并
- `-t` 覆盖的持久化
- 选择器中主题未变化时的跳过优化（save_theme_to 内部已按值判断）
