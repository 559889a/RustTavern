# 数据目录选择（现状）

本文记录 **已经落地** 的数据目录选择能力：它解决什么问题、当前链路如何工作、支持/不支持什么，以及后续开发最容易误改的契约。

> 本文只写“现在怎么运行”。

## 1. 解决的问题

用户现在可以把应用数据根目录（`data_root`）迁移到自选路径，而不再被固定在可执行目录旁的默认位置。

该能力的目标不是“运行期热切换”，而是：

- 在启动早期完成数据根决议（CLI > config.yaml > 运行时引导配置 > 默认）；
- 若需要，先迁移数据，再让后续所有子系统读取同一个 `data_root`。

## 2. 当前端到端链路

### 2.1 数据根决议（服务器模式）

服务器形态（B/S）下数据根决议在 `crates/rusttavern/src/server/config.rs`：

1. CLI `--data-root <dir>`（最高优先级）
2. `config.yaml` 的 `dataRoot` 字段
3. 旧桌面形态的运行时引导配置 `rusttavern-runtime.json` 的 `data_root`（`load_runtime_config_data_root`，兼容保留）
4. 默认 `<exe_dir>/data`

`config.yaml` 默认位于 `<exe_dir>/config.yaml`，首次启动自动生成带注释模板（含 `dataRoot`、`listen.host/port`、`security` 等）。

### 2.2 运行时配置文件（桌面形态遗留，兼容保留）

- 文件：`{exe_dir}/rusttavern-runtime.json`
- 解析入口：`crates/rusttavern/src/infrastructure/paths.rs`（`load_runtime_config`）
- 字段：`version` / `data_root` / `migration` / `migration_error`

该文件是 **启动引导配置**，不属于常规 settings；目的是避免“必须先知道 data_root，才能去读 settings”的自举循环。服务器模式仍读取其中的 `data_root`，但不再写入新值（见 2.3）。

### 2.3 设置面板（服务器模式下受限）

- 位置：`src/scripts/app/setting/setting-panel/settings-popup.js`
- `System -> Data Directory`（折叠区块）通过 `src/host-bridge.js` 的 `openDialog(...)` 打开系统目录选择器；服务器模式下 `plugin:dialog|open` 返回 `null`，选择器不可用，该 UI 入口实际不可用（`set_data_root`/`get_runtime_paths` 命令仍注册）。
- 服务器模式更改数据目录请使用 `--data-root` 或 `config.yaml` 的 `dataRoot`。

结论：**数据目录切换只能在重启后生效**。

## 3. 当前已支持的语义

### 3.1 目录校验

- `set_data_root` 要求目标路径：
  - 必须是绝对路径
  - 必须已存在且是目录
  - 必须是 **effectively empty**
  - Windows 上会通过 `dunce` 规整路径，避免把 `\\?\\C:\\...` 这类 UNC/extended-length 前缀写入 runtime 配置

“effectively empty” 当前允许仅包含系统元数据文件：

- `.DS_Store`
- `.localized`
- `desktop.ini`
- `Thumbs.db`
- `Icon\r`

这套语义在：

- 设置命令校验
- 启动期迁移目标检查

两侧保持一致。

### 3.2 迁移策略

- 优先整目录 `rename`（同盘）
- `rename` 失败后回退到递归复制
- 递归复制当前支持：
  - 普通文件
  - 普通目录
  - 软链接（会在目标侧重建链接，而不是降级成普通文件）

### 3.3 失败恢复

- 若迁移失败但旧 `data_root` 仍存在：
  - 启动继续使用旧目录
  - `rusttavern-runtime.json` 保留 `migration`
  - 写入 `migration_error`
- 若旧 `data_root` 已不存在，但目标目录已经像一个初始化完成的数据根（当前以存在 `default-user/` 为判定）：
  - 启动自动视为“上次迁移已完成但标记未清理”
  - 清除 `migration`
  - 切换到目标目录
- 若旧 `data_root` 已不存在，且目标目录也不像一个已完成的数据根：
  - 视为不可恢复状态
  - 启动直接报错（fail-fast）

## 4. 当前明确不支持的内容

- 设置面板内更改数据目录（服务器模式下无系统目录选择器；请用 `--data-root` / `config.yaml`）
- 运行期热切换 `data_root`
- 对任意目标目录做自动合并迁移
- 自动忽略未知文件/目录后继续迁移

这些边界是刻意保守的：目的是降低数据分叉和静默损坏风险。

## 5. 持续开发约束

- **路径策略必须继续收敛在 `crates/rusttavern/src/infrastructure/paths.rs`**
  - 不要把 `data_root` 决议逻辑散落到 command、service、frontend 多处
- **`rusttavern-runtime.json` 是唯一的启动引导配置**
  - 不要把 `data_root` 写回常规 settings
- **“effectively empty” 语义必须前后端一致**
  - 改一处时必须同步改启动期迁移检查
- **迁移失败恢复与不可恢复状态要分开**
  - “旧目录还在”可以恢复
  - “旧目录没了且目标不可用”必须 fail-fast
- **配置文件替换逻辑复用基础设施公共实现**
  - 避免再复制一套 temp+rename/copy fallback 逻辑

## 6. 关键代码位置

- `crates/rusttavern/src/infrastructure/paths.rs`
- `crates/rusttavern/src/presentation/commands/runtime_paths_commands.rs`
- `src/scripts/app/setting/setting-panel/settings-popup.js`
- `src/host-bridge.js`
