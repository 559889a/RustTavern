# RustTavern 项目文档

本文件夹包含RustTavern项目的完整文档，用于指导开发和维护工作。

## 文档目录

1. [技术栈文档](./TechStack.md) - 当前技术栈、Clean Architecture 约束和工程守护入口
2. [前端指南](./FrontendGuide.md) - 前端代码结构、宿主注入启动链与模块化路由开发指南
3. [前端宿主契约](./FrontendHostContract.md) - Host Kernel 对上游/插件/脚本可观察行为的契约清单（重构必读）
4. [后端结构](./BackendStructure.md) - Rust workspace crate 边界、依赖方向和 Clean Architecture 实践
5. [现状说明](./CurrentState/README.md) - 当前实现状态快照与持续开发约束
6. [扩展 API 文档](./API/README.md) - `window.__RUSTTAVERN__.api.*` 的参考与适配指南（面向扩展作者）
7. [Agent 架构文档](./AgentArchitecture.md) - Agent Runtime 的高层架构入口
8. [Agent 细节文档](./Agent/README.md) - Workspace、Journal、Tool、LLM Gateway、MCP/SKILL 与测试策略
9. [HTTP Server 迁移方案](./HttpServerMigrationPlan.md) - 去 Tauri 化、终端启动 HTTP 服务器 + 浏览器 WebUI 的迁移蓝图（已实施完成）
10. [迁移后收尾计划](./PostMigrationOptimizationPlan.md) - Wave 0-6 收尾任务书：服务端/前端修复、性能优化与发布链路（已执行完毕，含 webui invoke 契约根因修复记录）

## 项目概述

RustTavern 是 SillyTavern 的 HTTP 服务器重构版本。项目保留上游前端体验，用 Rust（axum）workspace 后端替代原 Node/Express 后端：终端一条命令启动服务，任意设备浏览器访问 WebUI，并通过 Clean Architecture 约束长期可维护边界。

## 文档维护

这些文档应随着项目的发展而更新，确保它们始终反映项目的当前状态和目标。

当前前端文档已基于 SillyTavern 1.18.0 同步后的模块化注入架构更新。
其中：

- `docs/CurrentState/` 记录“当前已经落地的实现状态”和后续维护约束
- 后端总边界以 `docs/BackendStructure.md` 为准；入口文档只保留概览，不重复维护 crate 细节
- Agent 系统已落地 canonical model IR、provider native metadata 保真、provider_state continuation、上下文只读工具、workspace 读改工具循环与前端 dryRun adapter。当前事实见 `docs/CurrentState/AgentFramework.md` 与 `docs/CurrentState/AgentProviderState.md`；高层文档放在 `docs/AgentArchitecture.md` / `docs/AgentContract.md` / `docs/AgentImplementPlan.md`，细节文档放在 `docs/Agent/`
- Agent 的实时开发进度跟踪见 `docs/CurrentState/AgentFramework.md`
