# HTTP Server 迁移方案：去 Tauri 化 → 终端启动 HTTP 服务器 + 浏览器 WebUI

> 状态：**已实施完成（2026-08-31）**。Phase 0→6 全部落地并提交；各阶段实施记录见
> `docs/history/handoff/HttpServerMigrationLog.md`，本文档保留为迁移蓝图与决策依据。
>
> 背景：RustTavern 当前是 Tauri 2 原生 App（桌面 + Android/iOS），数据锁在单机。目标是改造成原版 SillyTavern 的使用模式：终端一条命令启动服务 → 任意设备浏览器访问 WebUI → 数据集中在服务端互通。

## 已拍板的决策

1. **彻底移除 Tauri**（含 Android/iOS 打包），仓库只保留纯 server 形态
2. **安全对标原版 ST**：config.yaml 单管理员账号（HTTP Basic + 会话 cookie）+ IP 白名单；绑定非回环地址且未配置认证时 fail-fast 拒绝启动
3. **前端最小改造**：保留前端全部功能与结构，只换通信传输层
4. **数据无缝沿用**：数据目录布局零改动
5. **分发**：可执行 zip（二进制 + 同目录静态资源），三平台 CI 构建

## 核心策略（为什么这么做）

**保留前端 Host Kernel 不动，只替换传输层。** 关键架构事实：前端已有一层"Host Kernel"（`src/host/main/`，约 150 文件），它把上游 ST 前端的同源 HTTP 请求（`/api/*`、`/thumbnail`、`/characters/*` 等）在浏览器里拦截（patch fetch/jQuery.ajax）并翻译成 Tauri invoke；上游前端和第三方扩展从不直接碰 invoke。因此：

- **前端改动集中在 1 个文件**：`src/host-bridge.js` 是 invoke/listen/createChannel/convertFileSrc 的唯一收口，把它换成 HTTP/SSE 实现即可
- **Rust 侧改动集中在 host crate**：`rusttavern` 从 Tauri shell 换成 axum HTTP 服务器；`presentation/commands/*` 的 340 个命令去掉 `#[tauri::command]` 属性，改由宏生成的分发表通过 `POST /__tt/invoke/{command}` 调用
- **Clean Architecture 核心 9 个 crate（tt-domain/contracts/ports/application/adapter-*）零改动**——这是本项目分层的直接回报

替代方案（把 30 个 JS 路由文件语义移植成 Rust REST 端点）被否决：工作量大数倍、上游兼容回归风险高，且违背"前端最小改造"。

## 目标架构

```text
浏览器（任意设备）
  ├─ 静态页面/JS/CSS  ←── GET /*           （axum 静态托管 src/）
  ├─ 上游 ST 兼容请求   ←── fetch/ajax 被 Host Kernel 拦截（前端逻辑不变）
  │       └→ safeInvoke ─→ POST /__tt/invoke/{command}  （JSON args，通用分发）
  ├─ 聊天流式/Channel  ←── GET /__tt/stream/{stream_id} （SSE）
  ├─ 后端事件 listen   ←── GET /__tt/events              （SSE 全局事件流）
  └─ Host 资源         ←── GET /thumbnail、/characters/*、/backgrounds/*、
                            /User Avatars/*、/assets/*、/user/images/*、/user/files/*、
                            /scripts/extensions/third-party/*、/css/user.css
全部经过中间件：IP 白名单 → Basic 认证 + 会话 cookie

终端：rusttavern(.exe) [--host 0.0.0.0] [--port 8000] [--data-root <dir>] [--config <file>]
启动打印监听地址，可选自动打开浏览器
```

## 可复用的既有资产（不重造）

| 资产 | 位置 | 迁移中的角色 |
| --- | --- | --- |
| HostResourceService + delivery 抽象 | `tt-application` 服务 + `presentation/web_resources/tauri_resource_adapter.rs` | `try_serve(request, delivery)` 已是准 HTTP 形态，新增 axum delivery 即可 |
| 事件发布 ports | `tt-ports` 的 `SyncJobEventPublisher`/`SyncAutomationEventPublisher` 等 | Tauri emit 适配器（`app/composition/adapters/sync_events.rs`）换成 EventBus 适配器 |
| invoke-broker transport 注入 | `src/host/main/brokers/invoke-broker.js`、`context/index.js` | transport 函数换实现，dedupe/writeBehind 策略层不动 |
| 分块上传 base64 路径 | `presentation/commands/chunk_body.rs` | 已支持 `chunk-encoding: base64` JSON 分块，HTTP 走此路径 |
| 多用户数据 + login.html | `user_commands.rs`、`src/login.html` | 本期不接入 HTTP 登录，留作二期 |
| 契约测试 | `tests/*.test.mjs`（160+ 个，纯 JS 层） | 大部分继续有效，是回归主力 |

## 实施阶段

### Phase 0：基线验证
- 跑通 `pnpm run check`，记录全绿基线（frontend guardrails / tsc / boundaries / contracts / rust tests / clippy）
- 若基线本身红，先修复再迁移

### Phase 1：HTTP 宿主骨架 + 命令分发切换（最大的原子改动）

**Rust 侧：**
1. `rusttavern/Cargo.toml`：新增 `axum`、`tower`、`tower-http`（fs/compression 可选）、`serde_yaml`、`clap`（轻量 CLI）；暂留 tauri 依赖（Phase 5 删）
2. 新增 `crates/rusttavern/src/server/` 模块：
   - `main.rs` / `lib.rs`：CLI 参数解析 → 读 `config.yaml` → 解析 RuntimePaths（复用 `infrastructure/paths.rs`，`--data-root` 优先）→ 构建 AppState → 启动 axum
   - `config.rs`：config.yaml 结构（listen host/port、security、dataRoot、autoOpenBrowser；缺省生成带注释的模板文件）
   - `router.rs`：组装所有路由 + 静态托管（web 根 = 仓库 `src/`，release 时为 exe 同目录 `src/`）+ 中间件骨架
   - `dispatch.rs`：命令分发表（见下方设计）
   - `events.rs`：EventBus（tokio broadcast）+ `/__tt/events` SSE 端点（本阶段先接 app-ready/app-error）
   - `host_resources.rs`：axum 路由 → `HostResourceService::try_serve(request, HTTP_DELIVERY)`（新 delivery 常量），覆盖 `/thumbnail`、`/characters/*`、`/User Avatars/*`、`/backgrounds/*`、`/assets/*`、`/user/images/*`、`/user/files/*`、`/scripts/extensions/third-party/*`、`/css/user.css`；保留现有 404/ETag/Cache-Control/Range 语义（`docs/contracts/HostResourceCaching.md`、`MediaAssetContract.md` 是验收依据）
3. **命令层批量机械改造**（编译器兜底）：
   - 删除全部 `#[tauri::command]`；`State<'_, Arc<AppState>>` 参数改为 `state: Arc<AppState>`（40/49 文件是此模式）
   - `app.rs`：`AppState::new` 去掉 AppHandle 参数，改传 `Arc<EventBus>`；`tauri::async_runtime::spawn` → `tokio::spawn`
   - 删除 `registry.rs` 的 `generate_handler!`，替换为 `dispatch.rs` 的宏注册表
4. **分发宏设计**（`server/dispatch.rs`）：
   ```rust
   // 宏展开后：从 JSON args 取参（camelCase/snake_case 双兼容，与现在
   // host-bridge.js 的 withTauriArgumentAliases 行为一致），类型由命令
   // 函数签名推断，返回值 serde 序列化
   register!(registry, get_character, state, name);
   ```
   - 特殊命令不走通用宏，逐个专门处理（完整清单见下方"特殊命令清单"）
5. `BackendReadiness`、`contract_tests` 保留；`composition/adapters/sync_events.rs` 等事件适配器暂改指向 EventBus

**验证**：`cargo check`/`cargo test` 绿；启动服务后浏览器打开 → 应用加载、角色列表含头像、设置可读写（无流式生成、无实时日志，属 Phase 2）

### Phase 2：流式与事件

1. **聊天流式**（替代 `ipc::Channel`，前端唯一消费方是 `src/host/main/routes/ai-routes.js`）：
   - Rust：`start_chat_completion_stream(stream_id, dto, on_event: Channel<…>)` → `on_event: StreamSink<ChatCompletionStreamEvent>`（自建类型，接口与 Channel 的 `send` 对齐，命令体逻辑不变）；StreamSink 注册进全局 `StreamRegistry`（broadcast channel，带容量缓冲防订阅竞态）
   - 新端点 `GET /__tt/stream/{stream_id}`：SSE 输出事件；加 `X-Accel-Buffering: no` + 心跳防代理缓冲
2. **分块命令**（chat_payload_commit、upload_staging）：走 base64-JSON 通用 invoke 路径（`chunk_body.rs` 已支持）；`stage_upload_chunk` 的 `tauri::ipc::Request` 参数改为从 JSON args 取 `data` 字段。raw 二进制端点留作后续优化
3. **事件全面迁移**：全仓 `grep -rn "\.emit(" crates` 的 10 个文件逐一改为 `EventBus::publish`（app.rs、backend_errors、observability、shutdown、infrastructure/logging/devtools、llm_api_logs、bridge.rs、composition/adapters/sync_events、guidance、sync_automation）
4. 前端 `listen()` 垫片（Phase 3 完成闭环）：SSE 订阅 `/__tt/events` 按事件名过滤，覆盖 `rusttavern-backend-log`/`rusttavern-llm-api-log`/sync 事件等

**验证**：浏览器真实聊天流式输出逐字上屏（需一个可用 LLM key）；Dev 面板后端日志实时滚动；上传头像/背景成功

### Phase 3：前端传输层切换

1. `src/host-bridge.js` 重写传输实现（保持导出签名不变）：
   - `invoke(cmd, args)` → `fetch('/__tt/invoke/' + cmd, {method:'POST', body: JSON})`，错误映射回现有 `CommandError` 文本协议（`kernel/host-error-response.js` 依赖错误文本形状）
   - `listen(name, handler)` → SSE 订阅 + 按名过滤，返回兼容的 unlisten
   - `createChannel(onmessage)` → 生成 stream_id、先建立 SSE 订阅再返回 channel 标记对象（保证不丢首批事件）
   - `convertFileSrc` → 同源路径直通；`isTauri()` 语义改为"TT 宿主运行时"
2. `src/init.js`：继续无条件设置 `window.__TAURI_RUNNING__ = true`（页面只由 TT 服务器提供，此 flag 现在表示"宿主内核应激活"），避免全库散改判断
3. **Tauri 插件调用兼容**（`grep plugin:` 的全部调用点）：
   - `plugin:fs|open/read/close`（`readable-file-stream-service.js`、`asset-io.js`）→ 服务端实现等价 fs 兼容命令（同走通用 invoke 分发，rid 语义不变），前端零改动
   - `plugin:fs|write_file/mkdir/remove`（`file-export.js`、`upload-service.js`）→ 服务端实现 + 前端已有的浏览器下载桥（download-bridge）兜底
   - `plugin:dialog|open`（`character-cards.js`、`skill.js`、`host-bridge.openDialog`）→ 返回 null，现有 `isNativePickerAvailable()` 逻辑自动回退浏览器 file input
   - `plugin:opener|*`（`data-migration` 扩展）→ 返回明确错误，扩展已有失败提示
   - 通知（`bridge.rs` 的 notification 命令）→ 前端改用 Web Notifications API（`system-notification-service.js` 已有浏览器分支则直通）
4. 逐一处理 9 个直接访问 `window.__TAURI__` 的文件（init.js、data-migration、host-ready、file-export、slash-commands、asset-io、pairing-listener、sync-listeners、sync-popup）

**验证**：`pnpm run check`（含全部契约测试）绿；更新 `host-bridge-contract.test.mjs`、`host-ready-contract.test.mjs` 等传输相关测试；浏览器完整走查：角色 CRUD、聊天、世界书、预设、扩展安装/更新、Quick Replies、主题、数据导入导出

### Phase 4：安全（认证 + 白名单）

1. `config.yaml` 增加 `security` 段：`authMode: none|basic`、`username`、`password`、`whitelist: [ip/cidr]`
2. 中间件链（tower layer）：白名单 → 认证（Basic 校验通过后签发 HttpOnly 会话 cookie；后续请求 cookie 优先）→ 业务路由；`/__tt/health` 仅白名单内可见
3. **Fail-fast**：绑定非 127.0.0.1 且 `authMode: none` → 启动报错退出并给出配置指引
4. CSRF：invoke/stream 端点要求自定义头 `x-tt-invoke`（同源 fetch 自动携带，跨站表单无法伪造）；不开 CORS（纯同源架构）
5. 密码存储：config.yaml 明文对齐原版 ST basicAuth 习惯，但文档强烈建议文件权限收紧；说明 cookie 为 HttpOnly + SameSite=Strict

**验证**：无凭据 curl 全端点 401/403；正确凭据全功能；白名单外 IP 拒绝；绑定 0.0.0.0 未配认证时拒绝启动

### Phase 5：Tauri 移除大扫除

1. **Rust**：删 `tauri`/`tauri-build`/全部 `tauri-plugin-*`/`objc2*`/`tauri-winrt-notification` 依赖；删 `app/host/{window,plugins,shutdown(改造为 axum graceful shutdown+ctrl-c),observability(Tauri 部分)}.rs`、`platform/`、`presentation/main_window_presenter.rs`、`windows_notifications.rs`、`ios_file_bridge_commands.rs`、`presentation/web_resources/dev_protocol_endpoint.rs`、`tauri.conf.json`、`tauri.pilot.conf.json`、`capabilities/`、`icons/`、build.rs 的 tauri-build；`lib.rs` mobile entry 删；`bundled_resources.rs` 从 Tauri resource 改为 exe 相对目录解析（`--resources` 可覆盖）
2. **前端**：删 Android/iOS 专属服务（`android-archive-service`、`adapters/android/`、iOS picker bridge 等）及其契约测试；保留 `compat/mobile/*`（手机浏览器访问仍受益）；`manifest.json`/SW 按同源服务调整
3. **脚本/CI**：删 `scripts/tauri-app.mjs`、`tauri-before-build.mjs`、`tauri-dev-server.mjs`、`ios-policy.mjs`、`ios-*.swift`、`build-portable.mjs`；新增 `scripts/build-server.mjs`（web:build → cargo build --release → 打 zip：binary + `src/` + `default/` + `src/scripts/templates/`）；`package.json` scripts 重写（`dev` = rspack watch + cargo run 并行；`build` = build-server）；workflows：删 `canary-release.yml`/`stable-release.yml`/`public-testflight.yml` 的 tauri-action 部分、重写为三平台 release 二进制发布，`pr-quality-gate.yml` 保留，`auto-prerelease-scheduler`/`fastools-build` 相应调整
4. 全仓 `grep -ri tauri` 清残留（代码零残留；文档在 Phase 6 处理）

**验证**：干净目录解压 release zip → 启动 → 浏览器完整可用；`pnpm run check` 全绿

### Phase 6：文档与收尾 —— ✅ 已完成

- 更新：`docs/architecture/BackendStructure.md`（host = axum，presentation = HTTP 分发）、`docs/architecture/FrontendHostContract.md`（传输层变更：invoke over HTTP、SSE 事件；`window.__TAURI_*` ABI 语义标注"宿主内核"而非"Tauri"）、`docs/architecture/FrontendGuide.md`、`README*.md`（新使用方式：下载/构建 → 启动 → 浏览器访问；局域网/公网部署指引含防火墙与 HTTPS 反代建议）、`AGENTS.md`/`agents.md`、`ExtensionDEV.md`、`docs/state/*`（DataDirectorySelection 补 server 模式、UpdateChannels、HostResourceCaching、LinuxRepository 等）
- 删除：`docs/AndroidDevelopment.md`、`docs/iOSDevelopment.md`、`docs/state/iOSPolicy.md`
- `scripts/check-rust-crate-boundaries.mjs` 规则更新（host 允许 axum/tower；移除 tauri 相关规则）；`check-frontend-guardrails.mjs` 基线更新
- 新增：局域网多设备访问说明（含 Windows 首次绑定 0.0.0.0 的防火墙提示）

## 特殊命令清单（不走通用分发宏）

| 命令 | 现状 | HTTP 方案 |
| --- | --- | --- |
| `start_chat_completion_stream` | `Channel<ChatCompletionStreamEvent>` | StreamSink + `GET /__tt/stream/{id}` SSE |
| `begin/append/finish/abort_chat_commit` | Channel 分块 | base64-JSON 走通用分发（chunk_body 已支持） |
| `stage_upload_begin/chunk/finish/discard` | `tauri::ipc::Request` raw body | base64-JSON 走通用分发；raw 端点留作优化 |
| `bridge::emit_event` | `Window` 参数 | EventBus 直发 |
| `bridge::get/show_notification*` | tauri-plugin-notification | 前端 Web Notifications API，命令删 |
| `bridge::read_dev_web_resource` | dev 专用 | 删（dev 由静态托管覆盖） |
| `ios_file_bridge_commands::*` | iOS only | 随移动端删除 |
| `plugin:fs\|*`、`plugin:dialog\|*`、`plugin:opener\|*` | Tauri 插件命令 | 服务端兼容实现或回退（见 Phase 3） |

## 明确不变的部分

- `tt-domain`、`tt-contracts`、`tt-ports`、`tt-application`、全部 `tt-adapter-*`：零改动
- 前端 `src/host/main/routes/*`（30 个上游兼容路由）、invoke-broker 策略层、`__RUSTTAVERN__` ABI 形状：不动
- 数据目录布局（`default-user/characters/chats/...`、`_tauritavern/`）：不动，旧数据直接沿用
- `fastools/`：不动

## 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| 340 命令批量机械改造出错 | 宏统一取参 + 编译期类型检查；Phase 1 拆成多个可编译的小提交（先基建后分批替换命令组）；契约测试 + 浏览器实测兜底 |
| `window.__TAURI__` 直接使用点遗漏 | 已 grep 出完整清单（9 文件 + plugin: 调用点），Phase 3 逐文件核对；全仓 grep 复查 |
| SSE 订阅竞态丢事件 | 前端先订阅后 invoke；broadcast 容量缓冲；心跳 + no-buffering 头 |
| 公网经反代 SSE 被缓冲 | 文档给出 Nginx/Caddy 配置；X-Accel-Buffering: no |
| base64 分块上传大文件开销 | MVP 接受；后续加 raw body 端点（不影响协议） |
| 第三方扩展直接访问 `window.__TAURI__` | 极少数；`__RUSTTAVERN__` ABI 保持；ExtensionDEV.md 说明变更 |
| 移动端浏览器行为回归 | 手机浏览器真机抽查（mobile compat 层保留是有利项） |
| 契约测试对 tauri 字样的断言 | Phase 3/5 同步更新相关测试文件（已列出） |

## 端到端验证

1. 每阶段：`pnpm run check` 全绿（guardrails + tsc + boundaries + 160+ 契约测试 + rust tests + clippy）
2. Phase 1-5 各自的浏览器验收（自动化 + 人工）：加载应用、角色卡 CRUD 与头像、聊天流式生成、世界书、预设、扩展安装/更新、设置持久化、数据导入导出
3. 局域网实测：另一设备（手机浏览器）访问 `http://<server-ip>:8000`，验证认证、聊天、数据互通
4. 数据兼容：用现有 Tauri App 的数据目录启动 server 版，角色/聊天/设置全部可见
5. 安全：401/403/fail-fast 用 curl 逐项验证

## 工作量与提交策略

在独立分支上按 Phase 0→6 顺序推进，每 Phase 一个或多个可编译可验证的提交；Phase 1 是唯一的大原子改动（命令签名批量切换）。预计涉及：Rust 侧 ~60 文件改/删 + 新增 server 模块 ~6 文件；前端 ~15 文件改/删；脚本/CI/文档 ~40 文件。
