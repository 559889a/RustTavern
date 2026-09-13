# RustTavern 项目文档

本目录**按文档用途分层**，不按写作时间。目的是让第一次进来的人（或 AI）一眼看出
「哪份是现在还算数的、哪份是历史记录、我该读哪个」。

## 按目的找

| 你想做什么 | 去这里 |
| --- | --- |
| 了解整体架构、加一个新模块 | [`architecture/`](architecture/) |
| 写扩展、调用宿主能力 | [`api/`](api/) |
| 确认某个跨模块契约的细节 | [`contracts/`](contracts/) |
| 看某模块**当前**实现到什么程度 | [`state/`](state/) |
| 查历史决策、已完成的迁移与实施记录 | [`history/`](history/) |
| 只是想把它跑起来 | [../README.md](../README.md) |
| 本机编译节奏 / 改代码的约定 | [../AGENTS.md](../AGENTS.md)、[../CONTRIBUTING.md](../CONTRIBUTING.md) |

## architecture/ —— 长期有效

架构与边界定义，不随某个功能上线而失效。改动前先读。

| 文档 | 内容 |
| --- | --- |
| [ProjectStructure.md](architecture/ProjectStructure.md) | 逐目录导航：crate 职责、请求链路、数据根布局、测试分组 |
| [BackendStructure.md](architecture/BackendStructure.md) | 后端 Clean Architecture、crate 边界与依赖方向 |
| [FrontendGuide.md](architecture/FrontendGuide.md) | 前端结构、宿主注入启动链与模块化路由 |
| [FrontendHostContract.md](architecture/FrontendHostContract.md) | Host Kernel 对上游/插件/脚本可观察行为的契约（重构必读） |
| [TechStack.md](architecture/TechStack.md) | 技术栈与工程守护入口 |
| [agent/](architecture/agent/README.md) | Agent 子系统的架构、契约与实现细节 |

## api/ —— 面向扩展作者

`window.__RUSTTAVERN__.api.*` 平台 ABI 的参考。这些是**对扩展的承诺**，改动即破坏兼容。

[api/README.md](api/README.md) 是入口，以下是各分区：`chat`、`layout`、`dev`、`worldInfo`、
`extension.store`、`agent`、`llmConnections`、`skill`、`mcp`（草案），以及
[Migration.md](api/Migration.md)（从 SillyTavern 扩展迁移过来）。

## contracts/ —— 跨模块契约

描述「多方共同依赖、不能单方面改」的东西。既不是纯架构，也不是进度快照。

| 文档 | 契约内容 |
| --- | --- |
| [AgentProviderState.md](contracts/AgentProviderState.md) | Agent `provider_state`：run-scoped continuation、Responses 增量输入与 `previous_response_id`、内部字段剥离 |
| [CharacterIdentityContract.md](contracts/CharacterIdentityContract.md) | 角色身份：`avatar_url` exact filename、Rust stem key、聊天目录别名与 rename/delete 语义 |
| [ChatPayload.md](contracts/ChatPayload.md) | 聊天 payload：canonical 完整历史、受限 DOM、原子提交与只读分页 |
| [HostResourceCaching.md](contracts/HostResourceCaching.md) | Host Resource 的 representation revision、weak ETag / Last-Modified / Range / If-Range |
| [MediaAssetContract.md](contracts/MediaAssetContract.md) | `<video>/<audio>` 依赖的全平台媒体资源契约（`Range` / `Content-Range`） |
| [UpdateChannels.md](contracts/UpdateChannels.md) | Stable / Canary 更新检测与统一发布契约（用户时间、机器 SHA、产物命名） |

## state/ —— 现状快照

「这个模块**现在**实际怎么工作、边界在哪、哪些能力明确不支持」。会随实现推进而更新；
如果和代码冲突，以代码为准，并顺手更新这里。

| 文档 | 覆盖 |
| --- | --- |
| [AgentFramework.md](state/AgentFramework.md) | Agent 框架实时进度：canonical model IR、native metadata 保真、工具循环、Host ABI |
| [Sync.md](state/Sync.md) | LAN Sync / TT-Sync v2：链路、状态目录、协议与事件语义 |
| [StartupOptimization.md](state/StartupOptimization.md) | 分阶段启动（Shell/Core/Full）、bootstrap 快照、按需加载 |
| [BootstrapOptimization.md](state/BootstrapOptimization.md) | 冷启动内存基线相关优化（如 tokenCache 避免整表加载） |
| [EmbeddedRuntime.md](state/EmbeddedRuntime.md) | 消息内 iframe runtime（JSR/LWB）生命周期：budget/park/hydrate/自愈 |
| [DataDirectorySelection.md](state/DataDirectorySelection.md) | 数据根决议优先级与启动期迁移语义 |
| [NativeApiFormats.md](state/NativeApiFormats.md) | Custom 原生 API 格式兼容（Responses / Claude Messages / Gemini Interactions） |
| [ThirdPartyExtensions.md](state/ThirdPartyExtensions.md) | 第三方前端扩展兼容：加载链路、资源端点、目录语义 |
| [MobileStyleAdaptation.md](state/MobileStyleAdaptation.md) | 移动端适配：safe-area、`--tt-inset-*` 变量契约、surface classifier |
| [MemoryExtensionApi.md](state/MemoryExtensionApi.md) | 记忆类扩展 API（`api.chat`）的落地状态：楼层语义、按需历史、文本检索 |
| [LoggingObservability.md](state/LoggingObservability.md) | tracing 日志、用户可见 backend error、DevObservabilityHub、LLM API log |
| [LinuxRepository.md](state/LinuxRepository.md) | Linux 分发：APT / RPM / Nix 的支持范围与安装入口 |

## history/ —— 历史归档

**已完成的计划与工作记录。保留是为了查「当初为什么这么决定」，不再维护、不要照着做。**

| 文档 | 性质 |
| --- | --- |
| [HttpServerMigrationPlan.md](history/HttpServerMigrationPlan.md) | 去 Tauri 化 → HTTP 服务器的迁移方案（已实施完成） |
| [PostMigrationOptimizationPlan.md](history/PostMigrationOptimizationPlan.md) | Wave 0-6 收尾任务书（已执行完毕） |
| [AgentImplementPlan.md](history/AgentImplementPlan.md) | Agent 实施说明（阶段性） |

`history/handoff/` 下另有若干**本机内部交接文档**（编译/部署速查、工作记录），
它们含本机绝对路径与部署拓扑，已被 `.gitignore` 排除、不进公开仓库，本地开发时才会看到。
