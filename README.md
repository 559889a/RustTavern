<div align="center">

# RustTavern

[![CI](https://github.com/559889a/RustTavern/actions/workflows/ci.yml/badge.svg)](https://github.com/559889a/RustTavern/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/559889a/RustTavern)](https://github.com/559889a/RustTavern/releases)
[![License](https://img.shields.io/github/license/559889a/RustTavern)](LICENSE)

**简体中文** · [English](README.en.md)

</div>

**RustTavern** 把 [SillyTavern](https://github.com/SillyTavern/SillyTavern) 变成一个本地 HTTP 服务器：
**前端完整保留上游体验**（已同步 1.18.0），**后端从 Node.js 重写为 Rust**。下载一个包解压，`./start.sh start`
启动，然后用任意设备的浏览器访问 WebUI —— 不需要 Node.js 运行时，不需要命令行技巧。角色卡、聊天记录、
预设、世界书与前端扩展全部兼容。

## 特性

- **纯二进制服务器，无运行时依赖** —— 后端是单个 Rust 二进制（axum + tokio），不需要安装 Node.js，
  也不需要 npm 生态。前端资源与默认内容随包分发，解压即用。
- **完整 SillyTavern 体验** —— 前端同步上游 1.18.0，数据目录布局（`default-user`、`characters`、
  `chats`、`group chats`、`User Avatars`、`QuickReplies` 等的大小写与空格）、聊天 JSONL、角色卡 PNG
  metadata、世界书、预设、主题全部保持上游语义。
- **前端扩展生态** —— 内置原生 Git（gix 实现 smart HTTP 与 embedded worktree），安装、更新、
  切换分支、删除都在界面内完成，不需要系统 git。上游 Node-only 后端插件不支持。
- **内置多设备同步** —— 局域网加密配对同步（`tauritavern://` 配对链接）或经远端 TT-Sync v2 自动上传；
  支持同步任务编排与状态推送。
- **Agent 框架** —— 工具调用、Skills、子代理、运行时间线、workspace 读写与 checkpoint/commit，
  Agent 可消费 chat search、world info、workspace 工具。
- **应用内数据迁移** —— 直接导入 SillyTavern 数据归档，角色卡、聊天与扩展一并搬家。
- **面向长对话的性能工程** —— 分阶段启动、聊天虚拟 DOM 加载（viewport + true tail 挂载）、
  流式渲染策略与增量 token 缓存，超长聊天记录依然流畅。
- **数据自主** —— 所有数据保存在本地；用 `--data-root` 或 `config.yaml` 的 `dataRoot` 指定位置。
- **默认安全** —— 默认只监听 `127.0.0.1`；绑定非回环地址时**必须**在 `config.yaml` 配置
  `security.authMode: basic` 或 `security.whitelist`，否则服务器拒绝启动（fail-fast，不做静默降级）。
- **服务管理脚本** —— `start.sh` / `start.ps1` 提供 start/stop/restart/status/health/url/logs，
  带 PID 校验、端口探测与健康等待，见下文。

## 快速开始

从 [Releases](https://github.com/559889a/RustTavern/releases) 下载对应平台的包并解压，目录内会有
`rusttavern` 二进制、`src/`（前端）、`default/`（默认内容）、`config.yaml` 与启动脚本。

发布产物命名：

| 平台 | 产物 |
| --- | --- |
| Windows x64 | `RustTavern-<版本>-windows-x64.zip` |
| Linux x64 | `RustTavern-<版本>-linux-x64.zip` |
| macOS arm64 | `RustTavern-<版本>-macos-arm64.zip` |
| Android / Termux arm64 | `RustTavern-<版本>-android-arm64.tar.gz` |

每个产物都附带同名 `.sha256` 校验文件。

### Windows

```powershell
# 解压后进入目录
.\start.ps1 start      # 启动并等待 /__tt/health 通过
.\start.ps1 status     # 进程、内存、端口、访问控制、健康状态、URL
.\start.ps1 url        # 打印本机与局域网 URL
.\start.ps1 logs 80    # 查看最后 80 行 server.log
.\start.ps1 stop       # 按 PID 停止并确认端口释放
```

也提供 `start.cmd`，命令与 `start.ps1` 完全一致，供不愿用 PowerShell 的场景使用。首次绑定非回环地址时
Windows 会弹出防火墙提示，请勾选允许（专用/私有网络）。

### Linux / macOS

```bash
unzip RustTavern-<版本>-linux-x64.zip
cd RustTavern-<版本>-linux-x64
./rusttavern                 # 前台运行，默认 http://127.0.0.1:8000
```

Linux 上也可以用仓库内的 `packaging/termux/start.sh` —— 它不依赖 Termux 专有命令，
在普通 Linux 上同样可用：

```bash
./start.sh start | stop | restart | status | health | url | logs [N]
```

### Termux (Android)

Termux 版是原生 `aarch64-linux-android` 二进制（bionic libc，不是 glibc 构建）。

```bash
pkg install libc++
tar -xzf RustTavern-<版本>-android-arm64.tar.gz
cd rusttavern-termux-arm64
./start.sh start

# Android 会激进回收后台进程，脚本已在运行时自动持有 wake lock
termux-wake-lock          # 需要额外保活时可手动再加一道
```

手机上不需要任何 VPN 应用：把手机和电脑放在同一局域网，用 `TT_HOST=0.0.0.0 ./start.sh start` 启动，
然后在手机浏览器打开 `http://<电脑局域网IP>:8000`。

## 服务管理脚本

三份脚本功能完全一致，只是面向的平台不同：

| 脚本 | 平台 | 说明 |
| --- | --- | --- |
| `packaging/termux/start.sh` | Termux / Linux / macOS | POSIX shell；Termux 下自动加 wake lock |
| `packaging/windows/start.ps1` | Windows | 原生 PowerShell，推荐 |
| `packaging/windows/start.cmd` | Windows | cmd 版本，命令与 `start.ps1` 相同 |

命令：

| 命令 | 作用 |
| --- | --- |
| `start` | 启动并等待 `/__tt/health` 通过；启动即崩溃会打印日志尾部而不是假装成功 |
| `stop` | 按 PID 停止，轮询进程与端口直到真正释放 |
| `restart` | stop + start |
| `status` | 进程、内存、端口、访问控制、健康状态、数据根、日志路径、URL |
| `health` | 只打印健康 HTTP 码；仅 200 时退出码为 0 |
| `url` | 打印本机与局域网 URL |
| `logs [N]` | 查看最后 N 行日志（默认 40） |

环境变量覆盖（单次生效，不必改脚本）：

```bash
TT_HOST=0.0.0.0 ./start.sh start     # 监听所有网卡（需先配好访问控制）
TT_PORT=9000 ./start.sh start        # 换端口
TT_DATA_ROOT=/srv/tt ./start.sh start
```

```powershell
$env:TT_HOST = '0.0.0.0'; .\start.ps1 start
```

脚本刻意处理的平台差异：**升级前必须先 stop**（运行中的可执行文件不能被覆盖）；**PID 文件要与进程映像名
交叉校验**（陈旧 PID 在重启后可能指向无关进程）；**端口状态用连接探测而不是解析 netstat**
（列名与措辞随系统版本和语言变化）；`/__tt/health` 返回 **403 也算启动成功** —— 那说明服务在正常应答，
只是本机地址不在 `security.whitelist` 里。

## 命令行参数

```bash
rusttavern [--host 127.0.0.1] [--port 8000] [--data-root <dir>] [--config <file>] [--resources <dir>] [--no-open-browser]
```

| 参数 | 说明 |
| --- | --- |
| `--host` | 监听地址，默认 `127.0.0.1` |
| `--port` | 监听端口，默认 `8000` |
| `--data-root` | 数据目录，优先级高于 `config.yaml` 的 `dataRoot` |
| `--config` | 配置文件路径，默认 `<可执行文件目录>/config.yaml` |
| `--resources` | 资源根目录（包含 `default/` 与 `src/scripts/templates/`） |
| `--no-open-browser` | 启动时不打开浏览器（服务化部署/移动端用） |

## 配置

配置在可执行文件目录的 `config.yaml`，首次启动自动生成：

| 字段 | 含义 |
| --- | --- |
| `listen.host` / `listen.port` | 监听地址与端口（默认 `127.0.0.1:8000`） |
| `dataRoot` | 数据目录（默认 `<可执行文件目录>/data`；被 `--data-root` 覆盖） |
| `autoOpenBrowser` | 启动时是否自动打开浏览器 |
| `security.authMode` | `basic` 启用用户名/密码认证（每个请求都认证），或 `none` |
| `security.username` / `security.password` | `authMode: basic` 的凭据 |
| `security.whitelist` | 允许连接的地址列表，支持单 IP、CIDR 与 `192.168.1.*` 通配写法 |

> [!IMPORTANT]
> 绑定非回环地址时至少要配置一种访问控制（`authMode: basic` 或 `whitelist`），否则服务器**拒绝启动**。
> 这是有意为之：一个监听局域网、无认证、能读写本机文件的服务器不是一个可以静默接受的默认值。

认证走 Basic / 会话 cookie，传输是明文 HTTP。要暴露到公网，请在前面加 HTTPS 反向代理。

## 数据与兼容性

数据兼容的目标是**浏览器、扩展和用户数据可观察的 SillyTavern 语义**，而不是 Node/Express 的内部实现。

RustTavern 的私有状态（agent workspace、agent profiles、Skills、prompt cache、LLM connections）
放在数据根目录下的 `_tauritavern/`。

> [!NOTE]
> **关于仓库里残留的 `tauritavern` 字样**：RustTavern 是二开项目的二开 —— 上游是 SillyTavern，
> 中间是 TauriTavern（最初的 Tauri 桌面壳版本）。因为磁盘格式必须向后兼容，以下名字**刻意保留**，
> 它们是兼容契约而不是遗留命名，改掉会让既有用户的数据与配对链接失联：
>
> | 保留的名字 | 位置 |
> | --- | --- |
> | `_tauritavern/` | 数据根下的私有状态目录 |
> | `tauritavern-settings.json` | `default-user/` 下的设置文件 |
> | `data.extensions.tauritavern` | 角色卡 / 预设的扩展键 |
> | `extra.tauritavern` | 聊天消息 metadata |
> | `tauritavern://` | 局域网同步配对链接 scheme |
> | `SILLYTAVERN_*` | 上游约定的环境变量 |

## 构建与开发

**前置要求**：Rust stable（支持 edition 2024）· Node.js 20.19.x 或 22.12+ · pnpm

```bash
git clone https://github.com/559889a/RustTavern.git
cd RustTavern
pnpm install
pnpm run dev            # Rspack watch + cargo run 并行，自动打开浏览器
```

常用命令：

```bash
pnpm run build          # 构建发布版：前端 bundle → release 二进制 → release/*.zip
pnpm run check          # 前端 guardrails + 类型 + 日志边界 + Rust crate 边界 + 契约测试 + clippy
pnpm run web:build      # 只构建前端资源包（rspack）
pnpm run server:build   # 只构建 Rust 服务端（debug）
pnpm run test:contracts # 前端契约测试
```

> [!TIP]
> 本机是精简 Windows 镜像时，kernel32 可能缺少 `WaitOnAddress` 系列 API，导致 Rust 二进制启动即
> `0xc0000139`。仓库根目录的 `stub-synch.c` 等桩文件与 `scripts/patch-imports.mjs` 就是为这种环境准备的，
> 已被 gitignore，不属于产品。

## CI/CD

| 工作流 | 触发 | 作用 |
| --- | --- | --- |
| [`ci.yml`](.github/workflows/ci.yml) | push 到 `main`、PR | 前端 guardrails / 类型检查 / 日志与 crate 边界 / 契约测试；Rust clippy + `cargo test`；三平台 debug 产物 |
| [`debug-build.yml`](.github/workflows/debug-build.yml) | 手动 `workflow_dispatch` | 三平台 debug 产物，用于发版前的真机冒烟 |
| [`release.yml`](.github/workflows/release.yml) | 推送 `v*` tag | 校验 tag 与 `Cargo.toml`/`package.json` 版本一致 → 四平台打包 → 附 sha256 → 自动创建 GitHub Release |

发版流程：把 `Cargo.toml` 与 `package.json` 的版本改成同一个值，提交，然后打 tag：

```bash
git tag v2.3.0 && git push origin v2.3.0
```

tag 与两个清单文件的版本不一致时，`release.yml` 会在打包前直接失败 —— 不会出现二进制自报的版本与仓库声明不符。

## 项目结构

```
RustTavern/
├── Cargo.toml                    # Rust workspace 根（成员、依赖、dev/release profile）
├── package.json                  # 前端工具链与 pnpm script
├── rspack.config.js              # 前端 bundle 构建（vendor / agent-system / 设置面板）
├── default/                      # 首次运行的内容脚手架（config.yaml 模板、预设、主题）
├── resources/                    # 随二进制分发的资源（Claude tokenizer）
├── .github/workflows/            # ci.yml / release.yml / debug-build.yml
├── crates/                       # Rust workspace 成员（见下表）
├── docs/                         # 架构、前端、API 与实现状态文档
├── packaging/
│   ├── termux/start.sh           # 服务管理脚本（Termux / Linux / macOS）
│   ├── windows/start.ps1         # 服务管理脚本（Windows PowerShell）
│   └── windows/start.cmd         # 服务管理脚本（Windows cmd）
├── scripts/
│   ├── build-server.mjs          # 前端 bundle → cargo release → 打包 zip
│   ├── pack-termux.mjs           # 组装 Termux 运行时包（tar.gz）
│   ├── dev-server.mjs            # 开发模式（rspack watch + cargo run）
│   ├── check-*.mjs               # 四道工程守护（前端/frontend、preload、日志、crate 边界）
│   └── pack-dist.mjs             # 分发前的隐私/路径泄漏扫描
├── tests/                        # 前端契约测试（node --test）
└── src/                          # 前端
    ├── index.html                # WebUI 入口
    ├── script.js                 # 上游 SillyTavern 主脚本 + RustTavern 注入
    ├── host-bridge.js            # HTTP invoke 桥（POST /__tt/invoke/{command}）
    ├── host/main/                # Host Kernel：拦截 fetch/jQuery.ajax 并转到本地 Rust 服务
    ├── scripts/app/              # 前端功能模块（chat、regex、设置面板、启动、性能）
    ├── scripts/rusttavern/       # RustTavern 专属模块（agent、layout-kit、ios-policy）
    ├── scripts/extensions/       # 随包扩展（agent-system、data-migration、code-render…）
    └── locales/ css/ img/ sounds/ webfonts/
```

后端是遵循 Clean Architecture 的 Cargo workspace，源码依赖只能从外层指向内层
（`scripts/check-rust-crate-boundaries.mjs` 强制检查）：

| crate | 职责 |
| --- | --- |
| `rusttavern` | axum HTTP server host、命令分发层与组合根 |
| `tt-application` | use case、service、policy 编排 |
| `tt-domain` | 领域模型、值对象、领域错误、纯规则 |
| `tt-contracts` | 跨 crate DTO、事件、payload、host resource 契约 |
| `tt-ports` | repository / gateway / runtime trait |
| `tt-adapter-http` | 共享 HTTP client pool / profile |
| `tt-adapter-provider-http` | LLM、SD、Translate、TTS、provider metadata |
| `tt-adapter-tokenization` | tokenizer |
| `tt-adapter-storage-core` | `DataDirectory`、基础文件系统与基础存储 |
| `tt-adapter-storage-userdata` | 角色卡、世界书、agent workspace/profile、Skill |
| `tt-adapter-media` | 头像、背景、用户媒体、host resource |
| `tt-adapter-extension` | 第三方扩展的发现、安装、更新、分支切换 |
| `tt-adapter-sync` | LAN Sync、TT-Sync v2 runtime 与 sync jobs |
| `tt-adapter-archive` | 数据归档导入导出 |

请求链路：`前端/扩展 → 同源 fetch → src/host/main/interceptors.js → routes → host-bridge.js →
POST /__tt/invoke/{command} → presentation command → tt-application service → tt-ports trait → tt-adapter-*`。

## 文档

文档目录按**用途**分层（架构 / 扩展 API / 契约 / 现状 / 历史），完整索引见 **[docs/README.md](docs/README.md)**。常用入口：

- [docs/architecture/ProjectStructure.md](docs/architecture/ProjectStructure.md) —— 逐目录结构与职责导航（crate、请求链路、数据根、测试）
- [docs/architecture/TechStack.md](docs/architecture/TechStack.md) —— 技术栈与工程守护入口
- [docs/architecture/BackendStructure.md](docs/architecture/BackendStructure.md) —— 后端 Clean Architecture 与 crate 边界
- [docs/architecture/FrontendGuide.md](docs/architecture/FrontendGuide.md) —— 前端架构与扩展指南
- [docs/architecture/FrontendHostContract.md](docs/architecture/FrontendHostContract.md) —— 宿主层对外契约
- [docs/api/README.md](docs/api/README.md) —— `window.__RUSTTAVERN__.api.*` 扩展 API 参考
- [docs/contracts/](docs/contracts/) —— 跨模块长期契约（provider state、角色身份、host resource 缓存…）
- [docs/state/](docs/state/) —— 各模块「现状」快照
- [docs/history/](docs/history/) —— 已完成的迁移与实施记录（历史归档）

## 许可与致谢

基于 [SillyTavern](https://github.com/SillyTavern/SillyTavern) 构建。以 [AGPL-3.0](LICENSE) 许可发布
（与 SillyTavern 同系列许可协议）。
