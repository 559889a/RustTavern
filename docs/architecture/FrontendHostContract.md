# Frontend Host Contract（RustTavern）

> 目的：把“宿主平台层（Host Kernel）对外承诺的行为”显式化，避免重构 `src/host/main/*` 时误伤上游 SillyTavern / 第三方扩展 / 重脚本 / 角色卡。  
> 范围：仅覆盖前端宿主层（浏览器页面内运行的 Host Kernel）对外可观察的契约；不描述 Rust 后端内部实现。  
> 参考：`docs/architecture/FrontendGuide.md`（集成架构与开发方式）

---

## 1. 稳定性分级（写清楚“哪些能改，哪些不能随便改”）

为了避免“什么都是 API”，本仓库把前端宿主行为按稳定性分为 3 类：

1. **Public Contract（对上游/插件/脚本/角色卡承诺）**
   - 一旦变更，必须在本文件记录，并在 smoke tests 里验证（见第 6 节）。
2. **Project Contract（项目内部约定）**
   - 例如 `init.js` 与 `bootstrap.js` 之间的协调信号；可以演进，但需要同步更新相关模块与文档。
3. **Internal（实现细节）**
   - 可自由重构，但不得改变 Public Contract 的外部可观察行为。

---

## 2. 启动链路与就绪信号（Public + Project）

### 2.1 启动顺序（事实）

当前启动链路（见 `docs/architecture/FrontendGuide.md`）：

1. `src/init.js`：负责最早期的环境标记、可选 perf 开关与动态 import。
2. `src/host-main.js`：薄入口，仅调用 `bootstrapTauriMain()`。
3. `src/host/main/bootstrap.js`：composition root，创建 context、注册 routes、安装拦截器与补丁。
4. `src/script.js`：上游 SillyTavern 主应用入口（vendor）。

### 2.2 就绪信号（Public/Project）

- `window.__RUSTTAVERN_MAIN_READY__ : Promise<void>`
  - 由 `src/host/main/bootstrap.js` 写入，表示宿主层初始化已完成（或失败已被捕获并写入 console）。
  - 在宿主运行时下，resolve 前必须完成 Rust `BackendReadiness` 等待，确保首批依赖 `AppState` 的命令不靠 “state not managed” 文本重试作为正常控制流。
- `window.__RUSTTAVERN_PERF_READY__ : Promise<unknown> | undefined`
  - 仅在 perf-hud 启用时存在（见第 5 节）。
- `globalThis.__RUSTTAVERN_PERF_ENABLED__ : boolean`
  - 由 `src/init.js` 在动态 import 前写入；`bootstrap` 会优先读取它（避免重复计算/时序差异）。
- `window.__TAURI_RUNNING__ : true`
  - 由 `src/init.js` 无条件写入（页面只由 RustTavern 服务器提供）；该 flag 表示“宿主内核应激活”，用于桥接层与上游脚本的运行时分支（B/S 形态下恒为 `true`，不表示存在 Tauri 运行时）。

---

## 3. 全局 API（Public）

> 这些符号被第三方脚本/扩展/角色卡直接调用，变更需极度谨慎。

### 3.1 资源与缩略图（Public）

由 `createTauriMainContext()` 安装（实现：`src/host/main/context/index.js`，兼容入口：`src/host/main/context.js`）：

- `window.__RUSTTAVERN_THUMBNAIL__(type, file, useTimestamp?) -> string`
  - 为 `bg` / `avatar` / `persona` 生成 `/thumbnail?...` Host Resource URL；未知 type 直接抛错。
  - 正常第一方路径不使用 `useTimestamp`；该参数只保留为显式 force/debug cache bust。
- `window.__RUSTTAVERN_BACKGROUND_PATH__(file) -> string`
  - 生成 `/backgrounds/<encoded file>` Host Resource URL。

这些 API 的**可观察行为**必须保持：

- 对同一输入的 URL 形态（路径/查询参数意义）保持一致；
- 失败时的返回值语义保持一致（例如 `null` vs 抛错 vs fallback string）；
- 不得引入同步阻塞（第三方会在渲染路径高频调用）。

### 3.2 返回键处理（Public）

由 `src/host/main/back-navigation.js` 安装：

- `window.__RUSTTAVERN_HANDLE_BACK__() -> boolean`
  - 返回 `true` 表示已消费返回键（例如关闭对话框/浮层/抽屉/聊天等），否则返回 `false`。

> 说明：Android/iOS 原生 Picker 回调符号（`__RUSTTAVERN_IMPORT_ARCHIVE_PICKER__` / `__RUSTTAVERN_EXPORT_ARCHIVE_PICKER__`）已随移动端支持移除，不再安装。

### 3.3 原生分享桥（Public）

由 `src/host/main/share-target-bridge.js` 安装：

- `window.__RUSTTAVERN_NATIVE_SHARE__ = { push(payload), subscribe(handler) }`
  - `push()`：注入分享 payload（url 或 png）。
  - `subscribe()`：订阅消费；若早到则进入 backlog，首次订阅会 drain backlog。

### 3.5 上游库兼容全局（Public）

由 `src/lib.js:initLibraryShims()` 安装：

- `window._ : lodash`
  - SillyTavern 生态中的 third-party 扩展可能把 lodash external 为 `_`，并在 ESM 模块求值阶段直接访问。
  - 该符号必须在 third-party 扩展模块加载前可用；不得依赖 webpack/Rspack 等打包器偶然泄漏全局。

该 ABI 属于 SillyTavern 兼容层，不放入 `window.__RUSTTAVERN__.api`。新 RustTavern 代码仍应从 `src/lib.js` 显式 import `lodash`。

### 3.6 平台 ABI（Public，新）

为避免未来继续扩散 `window.__RUSTTAVERN_*` 零散符号，宿主层额外提供一个**统一出口**：

- `window.__RUSTTAVERN__ : { abiVersion, traceHeader, ready, invoke, assets, api }`
  - `abiVersion: 1`：ABI 版本号（已发布扩展契约发生语义化破坏改动时递增）。
  - `traceHeader: string`：请求追踪 header 名（见 4.4）。
  - `ready: Promise<void> | null`：与 `__RUSTTAVERN_MAIN_READY__` 语义一致。
  - `invoke.safeInvoke(...)` / `invoke.flushAll()`：对 `context` invoke 能力的稳定包装。
  - `assets.thumbnailUrl` / `assets.backgroundPath`：对 3.1 中稳定 Host Resource URL helper 的统一引用。
  - `api.layout`：布局契约 API（safe-area / viewport / Android IME），并配合 `data-tt-mobile-surface` taxonomy 让扩展以几行 opt-in 完成移动端适配。
    - 详细签名与示例见：`docs/api/Layout.md`。
  - `api.chat`：RustTavern 独有的聊天/记忆类扩展 API（聊天摘要、元数据、历史分页、稳定存储、后端定位、纯文本检索）。
    - 详细签名与示例见：`docs/api/Chat.md`。
  - `api.characterCards`：角色卡文件选择宿主 API。用于以浏览器文件选择器选择本地角色卡文件，并返回标准 `File[]` 给上游导入/替换流程继续处理。
    - 当前已落地 Host ABI：`isNativePickerAvailable()`、`pickFiles(options?: { multiple?: boolean; title?: string }) -> Promise<File[] | null>`。
    - 当前 picker 仅暴露 Rust 角色导入器真实支持的 `json/png` 角色卡格式；上游 `processDroppedFiles` 的格式判断保持不变。
    - 语义：只补齐平台文件选择能力，不导入、不覆盖、不改变 `/api/characters/import`、`preserved_name`、角色 avatar identity 或上游 toast/tag/刷新收尾语义。
    - 取消选择返回 `null`；picker/staging/read 失败必须抛错，不得静默回退到浏览器 file input。
  - `api.extension.store`：扩展级**全局持久化**（不绑定 chat），提供 KV JSON + Blob，支持多 table。
    - 详细签名与示例见：`docs/api/Extension.md`。
  - `api.dev`：RustTavern 规范化的开发调试 API。内置 Settings 开发面板与第三方扩展都应消费这一层，而不是直接依赖 SSE 事件名或 Rust 命令名。
    - `api.dev.frontendLogs`
      - `list(options?: { limit?: number }) -> Promise<FrontendLogEntry[]>`
      - `subscribe(handler) -> Promise<unsubscribe>`
      - `getConsoleCaptureEnabled() -> Promise<boolean>`
      - `setConsoleCaptureEnabled(enabled: boolean) -> Promise<void>`
      - 语义：宿主统一负责“运行时开关 + 持久化设置 + 本地 bootstrap flag”同步；调用方不应再自行读写 `localStorage`。
    - `api.dev.backendLogs`
      - `tail(options?: { limit?: number }) -> Promise<BackendLogEntry[]>`
      - `subscribe(handler) -> Promise<unsubscribe>`
      - 语义：宿主负责共享后端日志流；多个订阅者并存时通过引用计数管理 `enable/disable stream`，不得彼此踩踏。
    - `api.dev.llmApiLogs`
      - `index(options?: { limit?: number }) -> Promise<LlmApiLogIndexEntry[]>`
      - `getPreview(id: number) -> Promise<LlmApiLogPreview>`
      - `getRaw(id: number) -> Promise<LlmApiLogRaw>`
      - `subscribeIndex(handler) -> Promise<unsubscribe>`
      - `getKeep() -> Promise<number>`
      - `setKeep(value: number) -> Promise<void>`
      - 语义：宿主统一负责历史索引、实时索引流与 keep 设置持久化；调用方不应直接操作 `devlog_*` 命令。

`api.dev.*` 的长期契约要求：

- DTO 字段保持 camelCase，新增字段只能做向后兼容扩展。
- `subscribe()` / `subscribeIndex()` 返回的 `unsubscribe` 必须幂等且可安全延迟调用。
- SSE 事件名 `rusttavern-backend-log` / `rusttavern-llm-api-log` 与命令名 `devlog_*` 属于 Internal 实现细节，不是第三方 Public Contract。

- `api.worldInfo`：RustTavern 规范化的 World Info / Lorebook 激活与导航 API。
  - `getLastActivation() -> Promise<WorldInfoActivationBatch | null>`
    - 返回最近一次真实生成流程对应的最终激活结果。
    - `null` 仅表示当前会话还没有捕获到任何一次最终激活结果。
  - `subscribeActivations(handler) -> Promise<unsubscribe>`
    - 只推送最终激活结果，不暴露 `WORLDINFO_SCAN_DONE` 的中间循环状态。
    - 不复播历史结果；若需要最近一次结果，应先调用 `getLastActivation()`。
  - `openEntry(ref: { world: string; uid: string | number }) -> Promise<{ opened: boolean }>`
    - Best-effort 导航入口。
    - `opened: true` 表示宿主已成功打开目标世界书并尝试定位到目标条目。
    - `opened: false` 表示目标世界书或条目不存在；其他异常直接抛出，便于调试。

`api.worldInfo` 的 v1 收缩边界：

- 只暴露“最终激活批次”，不直接暴露 `WORLD_INFO_ACTIVATED` / `WORLDINFO_SCAN_DONE` 原始载荷。
- 激活条目 DTO 仅承诺：`world`、`uid`、`displayName`、`constant`、可选 `position`。
- 不把扫描循环控制、预算内部状态、可变中间态对象直接升格为 Public Contract。
- `openEntry()` 必须复用上游 World Info 模块自身的导航能力；宿主 ABI 层不得直接依赖 `#WorldInfo`、`#world_editor_select`、`[uid=\"...\"]` 等 DOM 细节。

- `api.agent`：RustTavern Agent Run API。用于启动 Agent Run、订阅 run event、取消、审批工具、读取 workspace 文件/diff、rollback。
  - 详细参考见：`docs/api/Agent.md`。
  - 当前已落地 Host ABI：`startRunFromLegacyGenerate()`、`startRunWithPromptSnapshot()`、`cancel()`、`readEvents()`、`readWorkspaceFile()`、`subscribe()`。
  - Chat commit 由模型调用 `workspace.commit` 触发；前端内部 host bridge 响应 `chat_commit_requested`，先按完整 raw 目标文本应用上游生成输出保存前后处理，再通过上游 `saveReply()` 写同一消息楼层，最后调用 `resolve_agent_chat_commit`。
  - `persistStateId` 只在 persistent state 已经落盘后写入 chat metadata；host bridge 响应 `persistent_state_metadata_update_requested` 后调用 `resolve_agent_persistent_state_metadata_update`。
  - `startRunFromLegacyGenerate()` 是当前兼容入口：使用 Legacy dryRun 生成 `promptSnapshot`，再进入 Rust-owned Agent loop。
  - `startRunWithPromptSnapshot()` 必须在调用 backend 前解析 `stableChatId`；`workspaceId` 由 `kind + stableChatId` 派生，`runId` 仍表示单次执行。
  - 不存在公共 `startRun()` alias；启动入口必须通过名称表达来源和职责。
  - Legacy `Generate(..., dryRun = true)` 不返回 payload；Agent adapter 必须通过 `GENERATE_AFTER_DATA` 事件捕获 `generate_data`。
  - 当前 Rust registry 包含 `agent_list`、`agent_delegate`、`agent_handoff`、`agent_await`、`task_return`、`chat_search`、`chat_read_messages`、`worldinfo_read_activated`、`dice_roll`、`skill_list`、`skill_search`、`skill_read`、`workspace_list_files`、`workspace_search_files`、`workspace_read_file`、`workspace_write_file`、`workspace_apply_patch`、`workspace_commit`、`workspace_finish`；实际模型可见集合由 Profile 与 invocation exit policy 收窄，`task_return` 只注入 return-mode child。工具注册由 Rust runtime 独占，前端 Legacy ToolManager tools 必须禁用。
  - 当前显式拒绝 `stream: true`、external tools、external tool choice 和已有 tool turns。
  - 可恢复工具错误会写入 Agent journal 并回填下一轮模型；宿主级错误仍让 run failed。
  - Agent event 属于 Agent Run journal/timeline 投影，不得伪装成上游 SillyTavern `GENERATION_*` / `TOOL_CALLS_*` 事件。
  - `subscribe()` 当前是 polling wrapper，必须返回幂等 `unsubscribe`；底层 SSE 事件名与 Rust command 名属于 Internal，不是第三方 Public Contract。
  - `tools.list()` 与 `readModelTurn()` 是 Agent System UI/诊断使用的 Project Contract；其 DTO 可以随实验性 Agent control plane 演进，但必须在 `docs/api/Agent.md` 明确记录。不得为了兼容返回全局 model alias 或从 native name 猜测 canonical identity。
  - Agent Mode off 时，Legacy `Generate()`、`ToolManager`、`api.chat` 行为必须不变。

- `api.llmConnections`：RustTavern LLM Connection 管理 API。用于保存和读取 Agent Profile 可引用的 LLM 连接定义。
  - 详细参考见：`docs/api/LlmConnections.md` 与 `docs/architecture/agent/PromptAssembly.md`。
  - 当前已落地 Host ABI：`list()`、`load()`、`save()`、`delete()`。
  - Profile 只保存 `model.mode = "connectionRef"`、`connectionRef` 与 `modelId`，或保存分享/导入用的 `model.mode = "requiresConfiguration"`；不直接保存 Connection Manager 的 Model Target id。
  - Connection Manager Model Target 可以作为 UI 输入来源，但转换成 LLM Connection 时必须保真；无法表达的字段必须显式报错，不得静默丢弃。
  - Agent System 负责在启动、Model Target 创建/更新、Profile 保存和 Agent run 启动前同步 `model-target-*` LLM Connection；启动 reconcile 或更新无法保真物化时会删除对应派生 connection；删除 Model Target 不隐式删除已物化 LLM Connection。
  - Rust command 名与 repository/file layout 属于 Internal 实现细节，不是第三方 Public Contract。

- `api.skill`：RustTavern Agent Skill 管理 API。用于列出、预览导入、安装、读取和导出本地 Skill。
  - 详细参考见：`docs/api/Skill.md` 与 `docs/architecture/agent/Skill.md`。
  - 当前已落地 Host ABI：`list()`、`listFiles()`、`pickImportArchive()`、`discardPickedImport()`、`downloadImport()`、`previewImport()`、`installImport()`、`readFile()`、`writeFile()`、`move()`、`export()`、`delete()`。
  - Skill scope 分为 `global` / `preset` / `profile` / `character`；未显式传 scope 的历史无归属 Skill 按 `global` 处理。
  - `api.skill` 是 UI / 扩展侧管理入口，不是 Agent run 内的工具入口；模型只能通过 Rust runtime 注册的 `skill.list` / `skill.search` / `skill.read` 消费已安装 Skill。
  - Preset / Character embedded skill 导入必须经过用户确认；同名不同 hash 必须显式 skip 或 replace，不自动改名。
  - Skill import/export 不触发上游 SillyTavern `GENERATION_*`、`TOOL_CALLS_*` 或 regex 事件。

- `api.mcp`（规划中）：MCP Server/Tool/Resource/Prompt 的独立平台 API。Agent Mode 可以消费 MCP，但 MCP 不依附 Agent Mode。
  - 详细草案见：`docs/api/MCP.md`。
  - MCP stdio command/config 不得由 Agent/Preset/角色卡/世界书直接写入；危险工具调用必须经过 capability policy 与审批。

> 注意：`window.__RUSTTAVERN__` 是“平台 ABI”，应保持**小而稳定**；不要把内部实现对象整个暴露出去。

### 3.7 ChatSurface participant（Project Contract）

- `window.__RUSTTAVERN__.api.chatSurface`
  - 当前是实验性的 Project Contract，尚未作为 Public Contract 稳定发布。
  - 只暴露 `protocolVersion: 1`、`isManagedOwnershipRequired()` 与 `registerParticipant()`；ownership query 返回本页已冻结的布尔决策，投影控制器、DOM adapter、内部 revision、admission 预算和虚拟滚动引擎均不外露。
  - participant 必须显式声明协议版本；hook 返回同步 disposer，宿主用 `AbortSignal` 表达 mount/content/runtime 三种真实寿命。
  - mount/remount/content lifecycle 不得伪装为 SillyTavern 消息业务事件。
  - 完整协议与 raw API 接入示例见 `docs/api/ChatSurface.md`。

---

## 4. 请求拦截与路由契约（Public）

### 4.1 拦截范围（事实）

由 `src/host/main/interceptors.js` 安装：

- patch `window.fetch`
- patch `jQuery.ajax`（兼容 jqXHR/Deferred 行为）

拦截生效条件（见 `src/host/main/bootstrap.js`）：

- 仅在 **宿主运行时**启用（`bootstrapTauriMain()` 早退保护；页面由 RustTavern 服务器提供时恒启用）。
- 仅拦截 **same-origin** 请求（包含被 patch 的同源 iframe/window）。
- 是否接管由 `router.canHandle(method, pathname)` 决定（仅看 `url.pathname`）。

### 4.2 未命中行为（Public）

- `fetch`：未命中路由直接透传原生 fetch。
- `ajax`：未命中路由直接透传原始 `$.ajax`。
- 命中但无 handler：返回 `404` JSON（`{ error: "Unsupported endpoint: ..." }`）。

> 这类行为会被上游与第三方依赖：不要改成 silent fail/空响应。

### 4.3 路由表（Public）

路由定义集中在 `src/host/main/routes/*`，其路径本身属于 Public Contract（上游/插件会直接请求）。

最关键的启动依赖：

- `/csrf-token`：返回固定 token（用于兼容上游初始化对 CSRF 的假设）
- `/version`：返回版本信息

高频与高风险路径（示例，不是完整列表）：

- `/api/*`：应用核心 API（settings/chats/characters/ai/worldinfo…）
- `/css/user.css`：用户自定义 CSS 覆盖文件（数据目录 `_css/user.css`）
- `/scripts/extensions/third-party/*`：third-party 扩展静态资源端点（ESM/CSS/url()/字体/图片）
- `/thumbnail`：缩略图端点（与 `__RUSTTAVERN_THUMBNAIL__` 强耦合）
- 用户静态资源端点（通配符路由）：
  - `/characters/*`、`/User Avatars/*`
  - `/backgrounds/*`、`/assets/*`
  - `/user/images/*`、`/user/files/*`

### 4.4 浏览器资源契约（Public）

这些路径必须能被浏览器**原生子资源加载**（`<img src>` / `<link href>` / `<script src>` / `CSS url()`），且 dev/prod 语义一致：

- `/scripts/extensions/third-party/*`
  - `.git` 是 SillyTavern 迁移兼容所需的普通路径组件；显式文件请求不得仅因任一组件名为 `.git` 而被拒绝。`.`、`..`、编码路径分隔符与路径逃逸仍必须拒绝。
- `/css/user.css`
- `/scripts/rusttavern/layout-kit.js`（ESM；扩展可选 DX 糖衣）
- `/thumbnail?type={bg|avatar|persona}&file=...`
- `/characters/*`、`/User Avatars/*`
- `/backgrounds/*`、`/assets/*`
- `/user/images/*`、`/user/files/*`

第三方扩展管理 API 保持上游 install/update/version/branches/switch DTO 与 hook/event 语义，remote transport 为 Rust gitoxide smart HTTP。mutating update 不使用前端 timeout/AbortSignal，因为 invoke 已进入 Rust 后不能据此取消磁盘写入。version 的 UI AbortSignal 仍可用于关闭只读检查结果。branches 只投影唯一远端 heads（空 label，不 fetch object）；switch 成功为 `204`，当前 branch 是 no-op，且不得隐式触发 update hook。

对这些端点的最小可观察语义：

- 仅接受 `GET` / `HEAD` / `OPTIONS`
- 未命中返回真实 `404`（不回退 `index.html`）
- `Content-Type` 正确；成功表示使用 `Cache-Control: private, no-cache`、weak ETag 和适用的 Last-Modified，错误/OPTIONS/416 使用 `no-store`（完整条件请求与平台 delivery 语义见 `docs/contracts/HostResourceCaching.md`）
- 媒体文件（`video/*` / `audio/*`）必须支持 `Range`（单范围）并返回 `206 + Content-Range`（见 `docs/contracts/MediaAssetContract.md`）

背景预览与背景消费是两个不同表示：

- 系统静态图预览使用普通 `/thumbnail`；
- GIF/WebP/APNG 在预览动画开启时使用 raw `/backgrounds/*`，关闭时使用 `/thumbnail?...&static=true` 的 first-frame JPEG；
- MP4 选择器使用占位图，不为 poster 引入视频解码器；
- active background 与 `<video>` 始终保留 raw `/backgrounds/*`，`static=true` 不得进入播放路径。

禁止事项（为了保持契约稳定）：

- 禁止通过 DOM 原型级 monkey patch（例如改写 `HTMLImageElement.src`）来“模拟”这些端点的加载行为；必须补齐真实端点。

### 4.5 Request tracing（Project，建议作为调试常用工具）

对所有被宿主接管的路由响应，都会附带一个追踪 header：

- `x-rusttavern-trace-id: <traceId>`

用途：将 DevTools Network 中的单次请求，与 console 日志 / perf-hud 数据关联起来，定位第三方脚本导致的异常与性能热点。
header 名也可从 `window.__RUSTTAVERN__?.traceHeader` 获取（用于避免硬编码）。

---

## 5. 兼容补丁与观测（Public/Project）

### 5.1 Perf HUD（Project，作为验收工具）

- 开关：
  - `localStorage.setItem('tt:perf','1')` 后 reload
  - 或 URL 参数 `?ttPerf=1`
- 全局对象：
  - `window.__RUSTTAVERN_PERF__`（见 `src/host/main/perf/perf-hud.js`）

### 5.2 移动端浏览器兼容（Public in practice）

移动端旧浏览器（WebView）的 polyfills 与第三方浮层/窗口 surface classifier（配合 geometry firewall 的 safe-area contract）属于“运行环境的一部分”，第三方会依赖其存在：

- `window.__RUSTTAVERN_MOBILE_RUNTIME_COMPAT__`
  - 覆盖移动端旧浏览器的基础 polyfills（例如 `requestIdleCallback` / `cancelIdleCallback`）。
- `window.__RUSTTAVERN_MOBILE_OVERLAY_COMPAT__`
- `window.__RUSTTAVERN_MOBILE_IFRAME_VIEWPORT_CONTRACT_BRIDGE__`：same-origin iframe 的 viewport/inset contract bridge（用于 `viewport-host` boundary；主要用于 debug/幂等安装）
- `window.__RUSTTAVERN_MOBILE_WINDOW_OPEN_COMPAT__`：移动端外链 `window.open()` 通过系统浏览器打开（不创建应用内新窗口）

---

### 5.3 Dialog 兼容（Public in practice）

> 目的：补齐脚本生态高频依赖的“浏览器内置弹窗语义”，避免出现“点击后完全无反应”。

- `window.alert/confirm/prompt` 使用浏览器原生语义；在无法展示 UI 的环境下返回 Cancel/默认值并记录错误，不做 silent noop。
- 若运行环境缺失 `HTMLDialogElement.prototype.showModal`：宿主会安装 `dialog-polyfill` 并覆盖主窗口 + same-origin iframe/window（仅在缺失时启用）。

### 5.4 外链打开与 `window.open()`（Public in practice）

- `window.open(url, name, features)` 使用浏览器原生行为；`size/position` 特性（典型 OAuth popup）在多数浏览器中会创建新窗口/标签页，保持 `window.opener` / `postMessage` 回调语义可用。
- 显式外链打开统一使用 `src/host-bridge.js` 的 `openExternalUrl()`（实现为 `window.open`）；例如 `rusttavern-version` 扩展与自动更新弹窗。

## 6. Smoke Tests（Public 回归用例）

这些用例是“最小但真实”的兼容回归集（来源：你提供的 `.cache` 样本）：

1. **JS-Slash-Runner**
   - 能加载、UI 能打开、至少一条命令可执行（`confirm/prompt` 与 `<dialog>.showModal()` 不应 silent noop）。
2. **database_script**
   - 能注入运行（至少不崩），其 UI/入口可打开。
3. **V1.72（重型角色卡）**
   - iframe 能加载且不被同源 patch/拦截破坏。
4. **浏览器资源契约（端点级）**
   - `/thumbnail?type=bg|avatar|persona&file=...` 能返回图片 bytes（无 `blob:` 魔法）；不存在返回真实 `404`
   - `/thumbnail?type=bg&file=<animated>&static=true` 对可解码动画返回 JPEG；MP4 的 raw `/backgrounds/*` Range 播放不受影响
   - `/css/user.css` 能从数据目录 `_css/user.css` 返回用户 CSS；不存在返回真实 `404`
   - `/characters/*`、`/User Avatars/*`、`/backgrounds/*`、`/assets/*`、`/user/images/*`、`/user/files/*` 作为子资源可直接加载
   - `/scripts/extensions/third-party/*` 的 ESM/CSS/图片/字体均可加载，未命中返回 `404`；无秘密 fixture 的 `.git/HEAD` / `.git/config` 采用同一文件级路径语义
   - 媒体 Range 契约：`/backgrounds/<file>.mp4` 的 `Range: bytes=0-1` 返回 `206` 且包含 `Content-Range`

任何涉及第 3/4 节契约的改动，都必须至少跑通以上 smoke tests。

---

## 7. 工程约束（Project，维护者）

> 这些约束不属于第三方“对外 API”，但属于长期维护的硬门槛：它们用于防止宿主层再次退化为单体与隐式耦合。

- Guardrails：`pnpm run check:frontend`（`scripts/check-frontend-guardrails.mjs`）
  - 行数预算：关键聚合文件受 `scripts/guardrails/frontend-lines-baseline.json` 约束。
  - 依赖边界：`kernel/ports` 不得 import `services/routes/adapters`；`services` 不得 import `routes`。
  - 路由契约：`src/host/main/routes/*` 禁止直接引用 `window`（通过 `adapters/*` 触碰浏览器/DOM/上游 ST）。
- 类型检查：`pnpm run check:types`（`tsc -p tsconfig.host.json`）
- Invoke surface：宿主层已知命令名集中在 `src/host/main/kernel/invokes/host-commands.js`（减少字符串漂移与 typo）
