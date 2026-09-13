# RustTavern 项目结构

本文按目录逐个说明「这里有什么、它负责什么、谁调用它」，用于在不熟悉仓库时快速定位代码。
架构约束的权威定义仍在 [BackendStructure.md](BackendStructure.md) 与
[FrontendHostContract.md](FrontendHostContract.md)；本文只做导航与职责划分。

> 规模参考（2026-09 统计）：Rust 约 11.9 万行 / 14 个 crate；前端功能性模块约 460 个文件
> （不含上游单文件主体）；契约测试 148 个。

## 1. 顶层布局

| 路径 | 作用 |
| --- | --- |
| `Cargo.toml` / `Cargo.lock` | Rust workspace 根。成员、workspace 依赖、dev/release profile 都在这里 |
| `crates/` | 全部 Rust 代码（见 §2） |
| `src/` | 前端（见 §4） |
| `default/` | 首次运行的内容脚手架：`config.yaml` 模板、预设、主题、默认角色 |
| `resources/tokenizers/` | 随二进制分发的资源（Claude tokenizer，`include_bytes!` 进二进制） |
| `docs/` | 文档（见 §7） |
| `tests/` | 前端契约测试（见 §6） |
| `scripts/` | 构建、开发、工程守护脚本（见 §8） |
| `packaging/` | 分发脚本：`termux/start.sh`、`windows/start.ps1`、`windows/start.cmd` |
| `nix/` + `flake.nix` | Nix 包定义（从源码构建） |
| `.github/workflows/` | `ci.yml`、`release.yml`、`debug-build.yml` |

## 2. Rust 后端：14 个 crate

Clean Architecture：源码依赖只能从外层指向内层，`scripts/check-rust-crate-boundaries.mjs`
会直接让违规构建失败。

| crate | 文件 | 行数 | 职责 |
| --- | --- | --- | --- |
| `rusttavern` | 122 | 23,174 | host：axum 服务器、命令分发、组合根 |
| `tt-application` | 253 | 72,995 | use case / service / policy 编排（代码量最大） |
| `tt-adapter-storage-core` | 43 | 20,232 | `DataDirectory`、基础文件系统与基础存储 |
| `tt-adapter-storage-userdata` | 43 | 17,836 | 角色卡、世界书、agent workspace/profile、Skill |
| `tt-adapter-provider-http` | 30 | 13,161 | LLM / SD / Translate / TTS provider 仓储 |
| `tt-domain` | 41 | 7,960 | 领域模型、值对象、领域错误、纯规则 |
| `tt-adapter-extension` | 19 | 5,583 | 第三方扩展的发现、安装、更新、分支切换 |
| `tt-adapter-sync` | 22 | 5,067 | LAN Sync、TT-Sync v2 runtime 与 sync jobs |
| `tt-adapter-archive` | 11 | 3,725 | 数据归档导入导出 |
| `tt-adapter-media` | 10 | 3,715 | 头像、背景、用户媒体、host resource |
| `tt-ports` | 50 | 3,027 | repository / gateway / runtime trait |
| `tt-contracts` | 11 | 1,442 | 跨 crate DTO、事件、payload 契约 |
| `tt-adapter-tokenization` | 2 | 851 | tokenizer 具体实现 |
| `tt-adapter-http` | 4 | 741 | 共享 HTTP client pool / profile |

### host crate `crates/rusttavern`

| 目录 | 职责 |
| --- | --- |
| `src/app/` | 组合根：启动 profile、`AppContext`、事件总线（`events.rs`）、后台错误队列、契约测试 |
| `src/presentation/` | 对前端的命令层。`commands/registry.rs` 是 HTTP 分发入口的命令注册表 |
| `src/server/` | axum 路由（`router.rs`）、分发（`dispatch.rs`）、安全（`security.rs`）、静态资源与 SSE |
| `src/infrastructure/` | 日志（`logging/`）、持久化与资源（`persistence/`、`assets.rs`）、GitHub 更新检查 |
| `build.rs` | 生成默认内容清单、注入 git branch/revision |

`build.rs` 的相对路径是 `../../default/content`（相对 crate 根），改动 workspace 布局时要同步。

## 3. 请求链路

```text
前端 / 扩展
  → 同源 fetch / jQuery.ajax
  → src/host/main/interceptors.js        拦截并分类
  → src/host/main/routes/*               路由到具体实现
  → src/host-bridge.js                   HTTP invoke（带 x-tt-invoke CSRF 头）
  → POST /__tt/invoke/{command}
  → crates/rusttavern/src/presentation/commands/registry.rs
  → tt-application service
  → tt-ports trait
  → tt-adapter-* 具体实现
```

约定：`/api/*` 只承载上游 SillyTavern 兼容行为；RustTavern 新能力走
`window.__RUSTTAVERN__.api.*`。健康检查端点是 `/__tt/health`。

## 4. 前端

| 路径 | 作用 |
| --- | --- |
| `src/index.html` | WebUI 入口（上游主体） |
| `src/script.js` | 上游 SillyTavern 主脚本 + RustTavern 注入 |
| `src/style.css` | 上游样式主体 |
| `src/host/` | **Host Kernel**（169 文件）：`context / kernel / services / adapters / routes / api / bootstrap`，拦截同源请求并接管资源加载 |
| `src/host-bridge.js` | HTTP invoke 桥，前端调用 Rust 命令的唯一通道 |
| `src/host-main.js` | Host Kernel 入口 |
| `src/scripts/app/` | RustTavern 前端功能模块（60 文件）：`chat`、`message`、`perf`、`regex`、`setting`、`startup`、`agent-profiles`、`agent-skills` |
| `src/scripts/rusttavern/` | RustTavern 专属模块（21 文件）：`agent`、`layout-kit`、`ios-policy*`、`tool-turn-projection` |
| `src/scripts/extensions/` | 随包扩展（215 文件），含一方实现 `agent-system`、`rusttavern-version`、`data-migration`、`code-render` |
| `src/locales/`、`src/css/`、`src/img/`、`src/sounds/`、`src/webfonts/` | 上游语言包与静态资源 |
| `src/lib*.js`、`src/init.js`、`src/tt-ext-sw.js` | 依赖聚合、启动引导、扩展 service worker |
| `src/types.d.ts`、`src/global.d.ts` | Host ABI 类型定义 |

## 5. 数据根

默认在可执行文件目录的 `data/`（可用 `--data-root` 覆盖）。上游语义目录
（`default-user`、`characters`、`chats`、`group chats` …）与 RustTavern 私有状态分开：

```text
data/
├── default-user/            # 上游布局：characters / chats / groups / settings
│   └── tauritavern-settings.json   # RustTavern 私有设置（名字为兼容保留）
├── characters/ chats/ ...   # 其余上游目录
└── _tauritavern/            # RustTavern 私有状态
    ├── agent-workspaces/    agent run 的工作区与索引
    ├── agent-profiles/      Agent Profile 定义
    ├── skills/              本地 Skill 包
    ├── prompt-cache/        prompt 缓存
    ├── extension-store/     扩展 KV / Blob 存储
    ├── extension-sources/   扩展来源元数据
    └── llm-connections/     LLM 连接定义
```

## 6. 测试

`tests/` 下 148 个 `*.test.mjs`，用 `node --test` 跑（`pnpm run test:contracts`）。
它们大多是**契约测试**：读源码文本断言约定（防回归），或直接 import 模块执行逻辑。
按主题分组：

| 分组 | 例子 |
| --- | --- |
| Agent | `agent-api-contract`、`agent-frontend-contract`、`agent-profile-*` |
| 聊天与渲染 | `chat-surface-*`、`chat-payload-*`、`message-render-transaction` |
| 世界书 | `world-info-*` |
| 扩展 | `extension-*`、`skill-api-contract` |
| 设置与面板 | `settings-*`、`rusttavern-*-vue-contract` |
| 移动端与布局 | `mobile-*`、`layout-api-contract` |
| provider | `openai-*`、`claude-*`、`gemini-*`、`minimax-*` |
| host 资源与桥 | `host-resource-*`、`host-bridge-contract` |

Rust 侧测试在各 crate 的 `#[cfg(test)]` 模块内（`cargo test --workspace`；本机不跑，见
`AGENTS.md` 的编译节奏，CI 里跑）。

## 7. 文档

| 文档 | 权威内容 |
| --- | --- |
| [BackendStructure.md](BackendStructure.md) | 后端 Clean Architecture、crate 边界与依赖方向 |
| [FrontendGuide.md](FrontendGuide.md) | 前端结构与宿主注入启动链 |
| [FrontendHostContract.md](FrontendHostContract.md) | Host Kernel 对外契约（重构必读） |
| [TechStack.md](TechStack.md) | 技术栈与工程守护入口 |
| [agent/](agent/README.md) | Agent 架构、契约、Workspace、Tool、Journal、LLM Gateway 与测试策略 |
| [../api/](../api/README.md) | `window.__RUSTTAVERN__.api.*` 扩展 API 参考 |
| [../contracts/](../contracts/) | 长期有效的跨模块契约（provider state、角色身份、host resource 缓存…） |
| [../state/](../state/) | 各模块「现状」快照 |
| [../history/](../history/) | 已完成的迁移计划与实施记录（历史，不再维护） |

## 8. 构建与脚本

| 脚本 | 作用 |
| --- | --- |
| `scripts/build-server.mjs` | 前端 bundle → cargo release → 打包 `release/*.zip`（含 start 脚本） |
| `scripts/pack-termux.mjs` | 组装 Termux 运行时包（`tar.gz`） |
| `scripts/dev-server.mjs` | 开发模式：rspack watch + cargo run |
| `scripts/check-frontend-guardrails.mjs` | Host Kernel 规模与依赖边界 |
| `scripts/check-preload-hints.mjs` | preload hint 完整性 |
| `scripts/check-logging-boundaries.mjs` | logging target 使用边界 |
| `scripts/check-rust-crate-boundaries.mjs` | Rust crate 依赖方向 |
| `scripts/pack-dist.mjs` | 分发前的隐私/本机路径泄漏扫描 |
| `scripts/ci/verify-release-version.mjs` | 发布 tag 与各清单版本一致性 |

常用入口见根 `README.md`；本机编译节奏（少编译、迭代用 debug）见
[AGENTS.md](../../AGENTS.md)。
