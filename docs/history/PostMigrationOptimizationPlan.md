# 迁移后收尾计划（Post-Migration Work Plan）

> 本文档汇总迁移完成后的全面核查结果（两轮：第一轮 5 路只读审计 + 逐文件核实；
> 第二轮全部主张逐行亲验 + 3 路增量审计 + A 档独立复核 + 测试覆盖映射），
> 作为后续执行模型的完整任务书。所有发现均含证据位置与修复指令；措辞采用中性表述。
> 状态：**Wave 1 ✅ 已完成并提交（`f160a8da`）**；**Wave 2 ✅ 已完成并提交（`647339b2`）**；
> **Wave 3 ✅ 已完成并提交（`37ac53c8`）**；**Wave 4 第一批已提交（`d9aa596e`，
> 含 H1/H3/H4/M5/M8/L4/L5/L6；H2/L1/F1/F2 暂缓，理由见第六/七节）**；
> **Wave 5 ✅ 已完成并提交（`7869ca0d`，P0-1~P0-3 + P1-4~P1-8 + P2）**；
> **Wave 6 ✅ 本文档状态更新（含 webui 根因修复记录 `c2ab0253`）**。
> 每完成一个 Wave，在本文件头部勾选并提交。

## 零、交接说明（上下文压缩后恢复必读）

**作者**：本文档由 **planner 架构师**（规划与审计角色）编写，是上下文压缩后的恢复锚点，
直接交给执行模型开工。执行模型按 Wave 顺序实施；对结论有异议时，先核对证据位置再调整，
不得凭印象改动。

**作者已完成**：
1. 全面核查（两轮）：第一轮 5 路只读审计（服务端内存、前端 GC、性能、全仓死代码、发布链路）
   + 核心文件逐行核实；**第二轮**（2026-09-01）全部 P1 主张逐行亲验（与第一轮零偏差）、
   3 路增量审计（Rust 常驻集合扫描、前端资源释放扫描、A 档引用复核 + 测试覆盖映射）、
   新增 R25-R27 与 L6、F17-F21（含 F19 TTS 打断未释放通病——修正了第一轮"仅 3 个文件"
   的判断）、M8 精化（实测 167 处调用点）、A 档 12 项独立复核确认
   （A5-A8 路径纠正为 `scripts/`）、新增第十一章测试覆盖映射。
2. 本文档成稿（十一章，全部发现均含位置、证据、修复指令、验证方法）。
3. 本地清理：根目录 20 个日志文件（约 1.8MB）；`target/debug/incremental`
   编译缓存（约 5GB）；`target/debug/{data,default}`（E2E 数据副本约 15MB）。
   **Wave 0 至此全部完成**。C 盘可用空间约 24.8GB。
   注意：`target/debug/deps`（9GB）与 `target/debug/build`（1.7GB）是功能性构建缓存，
   删除会触发全量重编（1GB 机器上代价极高），**不是垃圾，保留**。
4. 保留决策已定：C 档清单（含对 patch-imports.mjs 的纠正——本机 E2E 必需，保留；
   `__TAURI_RUNNING__` 恒 true，相关分支是活代码；`--tt-ime-bottom` 有定义恒 0，
   `--tt-base-viewport-height` 无定义但消费点均有兜底）。

**作者未完成（全部属于执行模型）**：
1. ✅ **Wave 1（已完成并提交，2026-09-01）**：A 档 16 项全部删除；B 档 11 项验证完毕
   （B1/B3/B4/B5/B6/B8/B10/B11 已处理，B2 按默认保留，**B7/B9 两处按执行验证更正，见下表**）；
   验证门（降档）：契约测试 876 通过 + check:frontend + check:types + cargo check 零警告。
2. Wave 2：服务端修复 R1-R27。
3. Wave 3：前端修复 F1-F21。
4. Wave 4：性能优化 H/M/L（先建基线，再快赢，后结构性）。
5. Wave 5：发布链路修复 P0×3、P1×5、P2 若干。
6. Wave 6：文档收尾（断链、渠道描述、措辞）。
7. 每个 Wave 的验证门（第九节）与 git 提交。

**开工入口**：Wave 0 已完成 → **Wave 1（A 档 git rm）开始** → Wave 2 → Wave 3 → Wave 4 →
Wave 5 → Wave 6。本机环境事实见第一节；执行纪律见第二节；改代码前查第十一章测试覆盖映射。

**开工前必读（执行模型第一动作）**：
1. **先提交本文件**（当前未跟踪）：`git add docs/history/PostMigrationOptimizationPlan.md && git commit`，
   建立基线；否则 Wave 1 的提交会混入本文档。
2. **行号锚点**：本文档所有 file:line 以 HEAD `00fe95b2` 为基准。任何 Wave 改动后行号会漂移，
   定位一律以**符号名**（函数/结构体/常量）为准，行号仅作初始参考。
3. **验证门分档（2026-09-01 修订）**：**cargo check/test/clippy 与完整 `pnpm run check`
   （含 test:rust/clippy）一律不再执行**——本机 i5-3210m + 8GB 内存，harness 运行后实际
   可用编译内存仅约 3GB，增量编译也极慢。验证以「直接 `cargo build` 成功 + 运行时 E2E
   实测」为准（前端改动只跑 `check:frontend` + `check:types` + 受影响 node 契约测试，
   不触发任何 cargo 任务）。
4. **⚠️ 编译纪律（硬件硬约束，必须遵守）**：**不要跑编译测试 不要跑编译测试 不要跑编译测试**
   ——cargo check/test/clippy 一律禁止。需要新二进制时**直接 `cargo build` 一次**
   （串行参数 `$env:CARGO_BUILD_JOBS='1'; $env:RUST_MIN_STACK='16777216'`，重建后必须
   `node scripts/patch-imports.mjs <exe>`）；build 失败则一次性修完全部报错再 build 一次，
   成功即视为 Rust 验证通过（正确性以运行时 E2E 为准）。**除非用户明确要求，禁止启动
   任何 cargo 任务**。

## 一、背景与硬约束

- 形态：B/S。`rusttavern` 二进制 = axum HTTP 服务器（默认 127.0.0.1:8000），浏览器是唯一客户端。
  340 个命令经宏注册表分发 `POST /__tt/invoke/{command}`；SSE 端点 `/__tt/events`、`/__tt/stream/{id}`；
  静态文件 + SPA 回退；`GET /__tt/file?path=` 数据文件访问；安全层：IP 白名单 + Basic 认证 +
  内存会话 cookie + `x-tt-invoke` 请求头校验。
- 硬约束：目标机 1GB 内存、弱 CPU、C 盘可用空间有限（已回收编译缓存后约 24.8GB）。
  所有内存与性能方案以此为前提。
- 本机环境事实（会话恢复必读）：cargo 全局 rsproxy 镜像已生效；编译需串行参数
  `$env:CARGO_BUILD_JOBS='1'; $env:RUST_MIN_STACK='16777216'`；
  clippy 需 `$env:RUSTUP_TOOLCHAIN='1.95.0-x86_64-pc-windows-gnu'` 与 Strawberry PATH；
  **每次 cargo 重建 exe 后必须重跑 `node scripts/patch-imports.mjs <exe>`**（0xc0000139 问题）。

## 二、执行纪律（执行模型必读）

1. 每 Wave 结束后：`pnpm run check`（本机用上述串行参数）+ 受影响契约测试 + E2E smoke（启动、
   `/__tt/health`、`get_client_version`、停止），然后提交，commit message 引用本文件。
2. 进程管理：只允许按精确名称终止 `rusttavern` 进程；禁止宽泛的 node 进程清理。
3. 删除任何文件前先 `git status` 确认；删除后立即跑受影响的测试。
4. 改 Rust 依赖后 Cargo.lock 自动刷新；改前端后跑 `check:types` 与契约测试。
5. 本机 E2E 前先重跑 patch-imports（见上）。
6. 不确定的条目按 B 档流程验证后再动，禁止猜测。

## 三、Wave 规划（按顺序执行）

- **Wave 0 磁盘清理（✅ 已完成，2026-09-01）**：已删除 `target/debug/incremental`
  （约 5GB）、根目录 20 个 `*.log`（约 1.8MB）、`target/debug/{data,default}`
  （E2E 数据副本约 15MB）。`target/debug/{deps,build}` 为功能性构建缓存，保留。
- **Wave 1 删除无用项**：按第四节 A 档执行；B 档逐条验证；C 档不动。
- **Wave 2 服务端内存与并发修复**：按第五节 R1-R27。
- **Wave 3 前端内存与 GC 修复**：按第六节 F1-F21。
- **Wave 4 性能优化**：按第七节（先建基线 → 快赢 → 结构性）。
- **Wave 5 发布链路修复**：按第八节 P0/P1/P2。
- **Wave 6 文档收尾**：断链修复、渠道描述更新、措辞清理；**把本计划文档加入
  `docs/README.md` 索引**（第 15 行迁移方案条目之后，标注"收尾执行中"）；全部 Wave
  完成后把零章状态改为"已执行完毕"并记录各 Wave 提交哈希。

## 四、删除清单（Wave 1）

### A 档：可直接删除（引用链已核实，删除后无同步点）

| 编号 | 位置 | 说明 |
|---|---|---|
| A1 | `src/st.ico` | 旧应用图标，全仓零引用（index.html/login.html 用 favicon.ico 与 apple-icon-*） |
| A2 | `src/st-launcher.ico` | 同上，零引用 |
| A3 | `src/img/logo.svg` | 零引用；实际使用 `img/logo.png` |
| A4 | `src/css/streaming-display.css` | 无人加载（streaming-display.js 是活代码但不引用此 css） |
| A5 | `scripts/assemble-registry.mjs` | 一次性迁移工具，仅历史日志提及（二次复核：全仓零代码引用） |
| A6 | `scripts/convert-commands.mjs` | 同上 |
| A7 | `scripts/fix-state-order.mjs` | 同上 |
| A8 | `scripts/gen-registry.mjs` | 同上 |
| A10 | `.github/codex/prompts/testflight-release-notes.md` | TestFlight 链路已删除，无引用 |
| A11 | `.github/codex/skills/rusttavern-testflight-notes/` | 同上；且引用已删除的 iOSPolicy.md |
| A12 | `crates/rusttavern/Cargo.toml` 的 `tower-http` 依赖 | 全仓无 `use tower_http`，纯冗余依赖 |
| A13 | `tt-adapter-storage-userdata/.../file_character_repository/mod.rs:42-50` 的 `FileCharacterRepository::new()` | `#[allow(dead_code)]` 且零调用 |
| A14 | `tt-adapter-storage-core/.../file_chat_repository/mod.rs:99-118` 的 `FileChatRepository::new()` | 同上 |
| A15 | `src/scripts/app/chat/asset-io.js` 第 14-101 行（native fs 流整块） | 唯一调用点在恒不成立的分支（生产代码从不设置 `window.__TAURI__`）；保留 `hasNativeTauriRuntime()` 检测与 fetch 路径 |
| A16 | `packaging/flatpak/build-app.sh:53-55`（Icon 归一化块） | 匹配的 `Icon=rusttavern` 与 desktop 模板 `Icon={{icon}}` 永不相等，导致构建必然中断；删除该块（与 P0-3 一并处理） |
| A17 | `docs/architecture/agent/PromptAssembly.md:3` 断链 | 指向不存在的 `docs/PromptAssemblyPlan.md`，改为指向现存文档或删除该句 |

> **A 档二次复核记录（planner 第二轮，全部确认可删）**：
> - A1-A4、A5-A8、A10、A11、A12、A15、A17 在 HEAD `00fe95b2` 独立复核，引用计数均为零
>   （仅本计划文档与历史日志提及，不算引用）。
> - A4 注意：`src/scripts/streaming-display.js` 是活代码（connection-manager 引用），但它
>   从未加载此 css（用 CSS_PREFIX 类名，样式缺失是现状）——删除不改变任何行为，
>   提交信息中注明即可。
> - A12 注意：移除直接依赖后 Cargo.lock 仍会保留 `tower-http` 条目——它是 reqwest 0.13.2
>   的传递依赖，属正常，不要误判为删除失败；flatpak 的 cargo-sources.json 中
>   tower-http-0.6.8 条目同理保留。
> - A15 注意：只删 14-101 行私有 helper 与恒不成立分支，**保留** `hasNativeTauriRuntime()`
>   检测、fetch 路径与 `fetchAssetStream` export（transport.js 两处调用）。
>   `src/host/main/services/files/readable-file-stream-service.js` 有同名独立副本（非 import），
>   不受影响，勿混淆。
>
> A9（`scripts/patch-imports.mjs`）原判可删，**纠正为保留（C 档）**：本机 E2E 工作流依赖它，
> 历史日志多处引用；删除会导致本机无法修补导入表。

### B 档：删除前需验证

| 编号 | 位置 | 验证步骤 |
|---|---|---|
| B1 | `src/img/01ai.svg`、`src/img/blockentropy.svg` | `git grep 01ai / blockentropy` 无结果 + 确认存量聊天数据无对应 api id → 可删 |
| B2 | 其余 provider 图标组（约 40 个 svg） | 均被 `script.js:2532` 动态引用，**默认保留**；仅当对应 api id 不再产生时删除 |
| B3 | `src/img/addbg3.png` | 全仓零引用；确认无用户背景配置指向 → 可删 |
| B4 | `src/lib/toastr.js.map` | 确认不需要本地断点源映射后可删（或同步删除 `toastr.min.js:7` 的映射注释） |
| B5 | `distribution/apt-rpm/*` | 确认 APT/RPM 仓库发布流程是否停用；若停用则连同文档描述一起删（关联 P1-7） |
| B6 | `server/config.rs:31-32` 的 `config_path` 字段 | 确认无配置热加载计划 → 删字段与 allow |
| B7 | `tt-application/Cargo.toml:37` 的 `check-cfg = ['cfg(mobile)']` | **执行修正：不可删，声明保留**——`cfg!(mobile)` 在 `host_resource_service/third_party.rs:49` 有实际使用（第一轮 grep 模式 `cfg(mobile)` 漏检 `cfg!(` 调用形式）；删除声明会产生 unexpected_cfgs 警告（实测复现） |
| B8 | `rusttavern/Cargo.toml` 的 `bytes` 依赖 | **已验证可删**（第二轮 grep：src 下无 `bytes::` 使用，仅 `DownloadedBytes` 自有类型同名误报） |
| B9 | `tokio-util` 的 features | **执行修正（原判断不成立）**：tokio-util **0.7.18 已无 `sync` feature**（已从 rsproxy `.crate` 源码验证：features 仅 `__docs_rs, codec, compat, default, full, io, io-util, join-map, net, rt, time`；`pub mod sync;` 无 cfg 门控，`CancellationToken` 由 **`rt`** feature 提供：`rt = ["tokio/rt", "tokio/sync", "futures-util"]`）。原"两 crate 缺 sync、靠 unification 兜底"基于旧版 feature 名，不成立。实际修复：rusttavern `["io","rt"]` → `["rt"]`（`io` 是空 feature 冗余）；tt-application 原 `["rt"]` 本就正确。⚠️ 反陷阱：声明不存在的 feature（如 `["sync"]`）会**直接解析失败**（cargo 报 `does not have that feature`，实测），不存在"改错仍能通过" |
| B10 | `packaging/flatpak/README.md` | 引用的 `pnpm run flatpak:*` 脚本不存在；更新文档命令或补脚本 |
| B11 | `AGENTS.md` 裸文件名引用 | 补 `docs/` 前缀（纯文档） |

### C 档：保留（含理由）

- `window.__TAURI__` / `__TAURI_INTERNALS__` 检测与小回退块：**注意 `__TAURI_RUNNING__` 由 `init.js:3`
  恒置 true，相关分支是活代码**；仅 `window.__TAURI__?.core?.invoke` 类检测恒不成立。
- `tauri:` 协议判断、事件名 `tauri:exit-requested` / `tauri://theme-changed`（SSE 端真实发布）。
- `` 与 `src/host/main/` 目录名、`src/host/main/compat/mobile/*`（移动 UA 下真实安装，
  非死代码）、`ios_policy.rs` / `ios-policy.js`（恒 Ignored，ABI 兼容）、`tauriVersion` 字段（ABI 兼容）、
  `src/dev-sw-bootstrap.js` / `src/tt-ext-sw.js`（注册守卫恒不满足，兼容保留）。
- `scripts/patch-imports.mjs`（本机 E2E 必需，见 A9 纠正）。
- CSS 变量消费点：`--tt-ime-bottom` 在 `style.css:121` 有定义（恒 0）；`--tt-base-viewport-height`
  无定义处但全部消费点带 `var(--doc-height, ...)` 兜底，行为无缺陷，保留。
- `src/css/!USER-CSS-README.md`（有意占位，7 行）。

## 五、服务端内存与并发修复（Wave 2，R 系列）

优先级：P1 = 长时间运行必然累积或明显风险；P2 = 有界/低频累积或小幅浪费。

### P1 级

> **Wave 2 执行状态（2026-09-01）**：R1-R27 全部实现完成，提交 `Wave 2: R1-R27`（见仓库日志）。

- **✅ R1 [会话表无上限增长] `server/security.rs:285-296` + 209-243** — 每个携带 Basic 凭据的请求
  都签发新会话令牌（仅无 cookie 时本应签发），旧令牌只在同一令牌再次被验证且已过期时才移除；
  持续带 Basic 头的客户端会使表项线性累积（1 次/秒 × 24 小时 ≈ 8.6 万条）。
  修复：仅当请求无有效 cookie 时才签发；`create`/`validate` 时顺带清扫过期项；设定表项上限。
- **✅ R2 [文件句柄驻留] `server/fs_resources.rs:28-31,97-100`** — 资源表只在显式 close 时移除；
  浏览器页面关闭不调用 close 时，句柄与表项永久驻留（Windows 下还会阻碍文件删除）。
  修复：记录最近访问时间，open/read 时惰性驱逐超过 5 分钟未访问的项；或设 rid 数量上限。
- **✅ R3 [生成注册项驻留] `presentation/commands/chat_completion_commands.rs:40-42` +
  `tt-application/.../chat_completion_service/mod.rs:878-909`** — 非流式生成在客户端中途断开时
  请求被取消，注册项不清理。修复：用 Drop 守卫或 `select!` 包裹，取消时补清理。
- **✅ R4 [上游流无响应时限] `tt-adapter-http/src/pool.rs:204-206` +
  `tt-adapter-provider-http/.../http_chat_completion_repository/mod.rs:309-348`** — 流式请求只有
  连接超时，无数据到达时限；上游保持连接不发数据时，对应任务与流缓冲永久无响应。
  修复：加空闲数据超时（如 60 秒无数据），由定时器驱动取消。
- **✅ R5 [请求体上限过宽] `server/router.rs:164`** — 请求体上限 512MiB 且全量缓冲，单请求
  内存峰值可超 1.5GB（请求体 + 解析树 + 参数副本 + 解码副本），1GB 机器上是资源耗尽风险。
  修复：上限收紧到 32-64MiB（64MiB 可覆盖约 27MB 的 base64 图片数据），并先按
  Content-Length 预检拒绝超大请求。聊天保存帧为 4MiB（base64 后约 5.3MiB），分块上传
  每块 512KiB，64MiB 余量充足。
- **✅ R25 [WS 会话池无淘汰] `tt-adapter-provider-http/.../http_chat_completion_repository/
  openai_responses.rs:31-80`** — `ResponsesWsSessionPool.sessions` 只按显式 close 回收，
  无空闲超时、无容量上限；每个新 `provider_state.sessionId` 建立一条保持存活的 WebSocket
  连接常驻。普通（非 agent）聊天补全走完 `generate_persistent_ws` 后不关闭会话；
  `close_provider_session` 仅 agent 运行结束调用（且是游离任务，关闭期可能被中断）。
  修复：加空闲超时清扫（定期关闭长时间无活动的 socket 并移除条目）或容量上限（超限逐出
  最旧）；非 agent 流程在流结束时也调用 close。
- **✅ R26 [Agent 宿主等待无超时] `tt-application/.../agent_runtime_service.rs:116-120` +
  `agent_runtime_service/commit.rs:245-259` + `prompt_assembly.rs:202-216`** —
  run 在等待前端宿主确认（chat commit / prompt assembly）的 `select!` 只有 oneshot 与
  cancel 两个分支，无超时；前端断连、异常退出或不回复时，run 永久挂起，
  `active_runs` 与 `active_chat_commits`/`active_prompt_assemblies` 条目、后台任务、
  model WS 会话（叠加 R25）全部驻留。用户连续发起新 run 会持续累积。
  修复：宿主等待的 `select!` 加超时分支（到期移除 pending 条目、取消 run、调
  `clear_pending_host_requests_for_run`）；或宿主侧 SSE 断连时统一取消该 run。

### P2 级

- **✅ R6 [参数深拷贝] `server/dispatch.rs:87-102`** — 每个参数 `value.clone()` 整棵拷贝后再
  `serde_json::from_value`；大参数（base64 块）存在多份副本。修复：改为借用式反序列化
  （`T::deserialize(&value)` 走 `&Value` 的 Deserializer 实现），消除整值克隆。
- **✅ R7 [结果双重序列化] `server/dispatch.rs:122-128` + `server/router.rs:231-246`** —
  命令结果先 `to_value` 再 `to_vec`，两次全量序列化。修复：宏展开改为命令返回后一次
  `to_vec` 直出字节。
- **✅ R8 [整文件读入内存] `server/router.rs:281-315`（静态文件）、330-375（/__tt/file）** —
  `read_to_end` 全量缓冲再 `Body::from`（双份内存），无流式、无 Range。
  修复：改用流式响应体（如 `tokio_util` 的 ReaderStream）；Range 支持见 H1。
- **✅ R9 [读锁跨等待] `server/fs_resources.rs:74-95`** — read 期间持有全局互斥锁，所有文件资源
  读取被全局串行化；且 `len` 无上限校验（前端可传任意值）。修复：锁内取句柄后释放锁再读，
  或按 rid 分锁；`len` 设上限（如 1MiB）。
- **✅ R10 [同步文件调用在异步路径] `server/fs_resources.rs:173`、`upload_staging_commands.rs:114`、
  `router.rs:267/274/334`、`infrastructure/assets.rs:52,97`** — 同步 `std::fs::canonicalize` /
  `is_file` 在异步处理器内执行，可能阻塞运行时线程。修复：改 `tokio::fs` 或 `spawn_blocking`。
- **✅ R11 [关闭标记惰性清理] `server/stream.rs:121-130`** — 流关闭标记只在下次订阅等待时清扫；
  空闲期短暂累积（量小）。修复：close 时顺带清扫过期标记。
- **✅ R12 [异常路径流未关闭] `server/stream.rs:48,84-91` + `chat_completion_commands.rs:77`** —
  正常路径必 close；但生成任务异常退出（panic）时可能跳过。修复：给 StreamSink 加 Drop 守卫，
  仍注册时补 close。
- **✅ R13 [初始化等待无时限] `server/router.rs:64-82`（AppStateSlot::get）** — 后端初始化卡住时
  所有命令无限等待。修复：加等待时限，超时返回明确错误。
- **✅ R14 [模板缓存无上限] `tt-application/.../bundled_template_service.rs:10,46-62`** —
  缓存的模板只增不减。修复：设数量上限（如 256）或按使用频率淘汰。
- **✅ R15 [归档任务表只增不减] `tt-application/.../data_archive_service/job.rs:16-65`** —
  导入/导出任务记录从不移除。修复：终态后移除（前端取完状态即可）。
- **✅ R16 [缩略图生成阻塞且全局串行] `tt-adapter-media/.../thumbnail_cache.rs:333-389` +
  `server/host_resources.rs:48-50`** — 缩略图生成（整图读取 + 解码 + 写临时文件）在异步
  处理器内同步执行，且带全局锁。修复：`spawn_blocking` 包裹；按需放宽锁粒度。
- **✅ R17 [日志快照全量驻留] `infrastructure/logging/llm_api_logs/`（readable/stream.rs:7-10,185；
  repository.rs:123-137）** — 流式响应全文累积到结束；请求数据体快照全量驻留到写盘完成。
  修复：缓冲上限或截断；写盘任务立即完成。
- **✅ R18 [无界消息通道] `chat_completion_commands.rs:127` + `repository.rs:161`** —
  上游分块经两条无界通道传递，日志写盘较慢时可能积压。修复：日志写盘改 `spawn_blocking`
  或批量落盘。
- **✅ R19 [事件按订阅者重复序列化] `server/events.rs:126-131`** — 每个连接各自
  `serde_json::to_string`。修复：发布时序列化一次，广播预序列化文本。
- **✅ R20 [流式每令牌双重转换] `server/stream.rs:34-43` + `events.rs:74-77`** —
  每令牌 `to_value` → 广播拷贝 → Display 再序列化。修复：广播预序列化字符串。
- **✅ R21 [运行时线程数未配置] `server/mod.rs:88`** — `Runtime::new()` 默认工作线程 = CPU 核数。
  修复：`Builder::new_multi_thread().worker_threads(min(核数, 2))`，小内存机器省线程栈。
- **✅ R22 [流式消费不及时则静默丢帧] `server/events.rs:74-77`** — 订阅端积压时丢弃事件帧
  （Lagged 被过滤掉），对 LLM 令牌流意味着输出不完整。修复：积压时向连接发送明确的终止
  提示（如错误注释帧）并关闭连接，让前端能感知不完整而非静默缺字。
- **✅ R23 [整文件 base64 进内存] `presentation/commands/file_commands.rs:227-251`、
  `image_commands.rs`、`background_commands.rs:68-80`** — 头像/背景/文件导入整文件 base64
  进内存再解码写盘。修复：统一走分块暂存路径（关联 R5 的上限收紧）。
- **✅ R24 [上传分块多份副本] `upload_staging_commands.rs:156-194` + `chunk_body.rs:12-18`** —
  每块瞬态约 4 份副本（请求体 + 解析树 + 参数拷贝 + 解码缓冲）。修复：并入 R6 借用式反序列化。
- **✅ R27 [Vertex 凭据缓存无界] `tt-adapter-provider-http/.../vertexai_auth.rs:25-26`** —
  `SERVICE_ACCOUNT_CACHE` 进程级全局静态缓存按 JSON 内容哈希为键，只 insert 不 remove、
  无 TTL；每次更换 service account 配置产生一个新键永久驻留（每条还持有 Authenticator
  网络栈）。低频（配置变更时），但无上界。修复：改有界缓存（容量上限 + 简单逐出），
  或设置变更时主动清除。

### 已核实无需处理的项（勿动）

- SSE 连接生命周期：客户端断开时响应体被丢弃，转发任务与接收端随之释放；关闭信号
  覆盖两个端点；事件总线无订阅者时静默丢弃为既定行为（观察类事件，持久状态在仓储）。
- 事件总线容量 512、日志面板上限 800×3KB、LLM 日志索引上限（默认 5）且落盘淘汰；
  HTTP 客户端按配置单例复用；聊天提交走暂存 + 原子替换且按帧分块。
- **第二轮补充核实（均有界/有清理，勿动）**：BackendErrorHub pending 上限 50；
  ChatHistoryCoordinator pending 上限 32（latest-wins 合并）；ChatPayloadCommitRepository
  会话上限 8 且 finish/abort 必移除；HttpClientPool 键为固定枚举（≤11 项）且代理变更即清空；
  各 MemoryCache（chat/character 100 条 30min TTL、搜索缓存达上限整清）；AgentGuidanceMailbox
  上限 8 条/64K 字符；LanSyncRuntimeState 过期清理；EventBusPairingApproval 三条移除路径齐全；
  常驻 spawn 循环（sync_automation/retention/chat_history）均持 CancellationToken；
  工作区写锁为 `Weak<Mutex>` + retain 清扫；未发现 fs_resources 之外的锁 guard 跨 await。

## 六、前端内存与 GC 修复（Wave 3，F 系列）

> **Wave 3 执行状态（2026-09-01）**：F3-F20 全部实现完成（除 F1/F2 见下方决策记录），
> 提交 `Wave 3: F3-F20`（见仓库日志）。F19 家族 14 个 TTS 文件统一修复；
> F18 新增 AutoComplete.destroy() 并接入 QR 编辑器销毁路径与 MacroAutoComplete 移除清理。

- **F1 [长聊天下全文挂载] `tt-domain/.../settings.rs:377`（默认关闭虚拟化）+
  `src/host/main/services/chat-surface/install.js:102-104,140-155`** — 非受限渲染把所有
  消息节点全量挂载（仅靠截尾 100 条 + 手动展开）。注意：关闭是刻意的兼容性防御
  （第三方渲染扩展需支持 ChatSurface，`script.js:647-687` 有恢复弹窗自动关闭流程）。
  修复（谨慎）：按消息数阈值自动启用受限渲染（如超过 200 条），保留现有恢复弹窗兜底；
  或先做兼容性探测（检测已知不兼容渲染扩展）再启用。
  **⏸️ 执行决策（2026-09-01，不改动）**：两个建议方案均不可行——(a) 消息数阈值需在
  `installEmbeddedRuntime` 的 settings 初始化期判定（`embedded-runtime/install.js:20-24`），
  此时 `chat` 数组尚未加载，阈值不可靠；(b) 虚拟化开关在页面生命周期内不可变
  （`chat-virtualization-state.js:16-18` 运行期切换直接抛错），无法在渲染路径动态启用。
  保持默认关闭为刻意设计；用户可在设置面板手动开启。
- **⏸️ F2 [每次保存全量序列化] `src/script.js:8579-8607` + `tauri/chat/commit.js:27-36` +
  `tauri/chat/jsonl.js:197-243`** — 保存时整份聊天 `slice` + `JSON.stringify` → 拦截器
  整体 `JSON.parse` → 逐条分块 + base64（约 1.3 倍膨胀），一次保存产生 1-3 份全文副本，
  每次生成结束与编辑都触发。修复（结构性）：后端增量提交（只传变更消息）；或前端
  直接按块构造 JSONL 跳过整体 JSON 中转。
  **⏸️ 执行决策（2026-09-01，暂缓）**：`/api/chats/save` 的 `chat` 数组字段是 SillyTavern
  兼容契约（chat-routes.js:68-90 + 契约测试断言），前端跳过整体 JSON 中转需改路由契约，
  后端增量提交需改 Rust 会话状态机——均为跨层契约变更，风险高；保存频率低（生成结束/
  编辑时，非每令牌），且 R5 已把请求体限制在 64MiB（内存有界）。留存待 Wave 4 结构性
  阶段评估（可并入 H2 打包改造一并做）。
- **✅ F3 [观察器未释放] `src/scripts/extensions/quick-reply/src/QuickReply.js:1144-1149`** —
  每次打开编辑器都新建 body 级 MutationObserver 且从不 disconnect。修复：关闭时
  disconnect，或改为观察弹窗容器。
- **✅ F4 [音频对象未释放] `src/scripts/extensions/tts/volcengine.js:82-83`** —
  试听音频的临时对象 URL 从不释放（数百 KB 到数 MB 每次）。修复：onended/onerror 时释放
  （并入 F19 统一修复；volcengine 是唯一完全无 revoke 的文件）。
- **✅ F5 [替换前未释放旧对象] `tts/vits.js:392-393`、`tts/sbvits2.js:335-336`** —
  换源前先释放旧 URL。修复：赋值前 revoke 旧值（并入 F19 统一修复）。
- **✅ F6 [背景缩略图缓存无淘汰] `src/scripts/backgrounds.js:50,518-573`** —
  THUMBNAIL_BLOBS 按背景名缓存 blob URL，仅源变更/移除时单键删除，无总量上限
  （随不同背景数量增长，每项持有一个 blob URL）。修复：设总量上限，超限释放
  最久未用项（revoke 其 blobUrl）。
- **✅ F7 [日期缓存无界] `src/scripts/utils.js:1092-1110`** — dateCache 按时间戳键永不淘汰。
  修复：设上限（如 1000）或按使用频率淘汰。
- **✅ F8 [令牌缓存键累积] `src/scripts/tokenizers.js:165-240`** — IndexedDB 令牌缓存按聊天
  累积，仅手动重置才清。修复：容量上限或旧键清理。
- **✅ F9 [事件存储无界 + 平方级拷贝] `extensions/agent-system/src/run-timeline-event-store.js:5-27` +
  `run-timeline-session.js:72`** — 事件数组只增不减；每事件返回整数组拷贝。修复：按
  页上限裁剪、增量维护视图。
- **✅ F10 [向量缓存无界] `extensions/vectors/index.js:128-133,218-219,407-439,555-572`** —
  摘要与哈希缓存只增不减。修复：设上限淘汰。
- **✅ F11 [弹窗观察器未释放] `src/scripts/popup.js:301-307`** — 无原生对话框分支的
  ResizeObserver 不 disconnect。修复：关闭时 disconnect。
- **✅ F12 [拖拽重复绑定] `src/scripts/RossAscends-mods.js:617-618`** — 每次拖拽直接绑定
  文档级事件。修复：先解除再绑定。
- **✅ F13 [流通道无取消接口] `src/host-bridge.js:220-274,284-295` +
  `src/host/main/routes/ai-routes.js:490-493`** — 手动 SSE 读取循环无中止手段，用户停止
  生成时只能置空回调，读取器继续持有连接与闭包（服务端正常完成时最终关闭，属兜底缺失）。
  修复：createChannel 增加中止控制器，关闭时调用 `reader.cancel()`。
- **✅ F14 [流缓冲平方级拼接] `src/host-bridge.js:243-247`** — 缓冲整串重建 + 逐行拼接。
  修复：改用偏移切片，仅保留未消费尾部。
- **✅ F15 [导出路径平方级拼接] `src/scripts/app/chat/jsonl.js:56-73`** — 全量拼接。
  修复：数组收集后 join。
- **✅ F16 [流式渲染每令牌全文重处理] `src/script.js:4366-4448` + `reasoning.js:579-593`** —
  每令牌对累计全文做完整 markdown 解析与清洗（平方级），弱 CPU 上生成期间明显卡顿。
  修复：渲染降频（默认 15-20 帧/秒，已有 30 帧节流与隐藏页 250ms 档）；对新增片段
  增量格式化，未闭合块延迟到闭合再处理（关联 H4）。

**第二轮前端扫描新增（F17-F21，均已亲验证据）**：

- **✅ F17 [QR 克隆观察器累积] `src/scripts/extensions/quick-reply/src/QuickReply.js:1234-1240`** —
  `getEditorPosition` 在 `!this.clone` 时新建 body 级 MutationObserver（局部变量），
  永不 disconnect；编辑器关闭后 `clone` 置空（:1237），下次会话再建一个新观察器，
  旧的持续驻留并扫描整棵 body。修复：观察器存实例字段，clone 置空时 disconnect。
- **✅ F17b [QR 导出对象未释放] `QuickReply.js:347-354`** — 导出 Quick Reply 为文件时
  `URL.createObjectURL` 后 `a.click()` 无 revoke（每次导出点击驻留一个）。
  修复：click 后 revoke（可延后一拍保证下载启动）。
- **✅ F18 [AutoComplete 实例监听不释放] `src/scripts/autocomplete/AutoComplete.js:145,693-698`** —
  每个实例构造时注册 window resize 监听（无 removeEventListener 路径），`getEditorPosition`
  再建一个 MutationObserver（局部变量永不 disconnect）。QR 编辑器每次打开都克隆新模板
  并创建新实例（QuickReply.js:784）、最大化编辑器同理（chats.js:2242 → MacroAutoComplete
  触发）——弹窗路径实例持续累积（发送框单例正常）。修复：加 `destroy()`（移除 resize
  监听、disconnect 观察器、解绑 textarea 事件），在编辑器销毁路径调用。
- **✅ F19 [TTS 预览打断时旧对象未释放——家族通病] `tts/azure.js:175-179` 为代表，
  同构 11 文件（edge/google-translate/google-native/electronhub/grok/gpt-sovits-adapter/
  mimo/openai-compatible/speecht5/tts-webui）** — 模式：`this.audioElement.src = url`
  直接替换旧值且 `onended` 被覆盖——语音 A 播放中被 B 打断时，A 的 blob URL 无人释放
  （pause 不触发 ended，A 的 onended 处理器已被 B 的覆盖）。F4（volcengine 完全无 revoke）
  与 F5（vits/sbvits2 替换前不 revoke）是此通病的特例。**统一修复**：替换 `src` 前先
  `URL.revokeObjectURL(this.audioElement.src)`（当 src 为 blob: 时），全部 14 个文件
  一并处理。
- **✅ F20 [条件轮询永不清除] `tts/xtts.js:166`、`tts/silerotts.js:55`** — `apiCheckInterval`
  每 2 秒检查 Extras 模块列表，条件（含 tts 模块）不满足则永不 clearInterval，
  页面生命周期内永久轮询；每次 `loadSettings()`（切换 provider）可能叠加。
  修复：轮询加超时兜底（一段时间后强制清除），或改事件驱动。

### 前端已核实无需处理的项（勿动）

- 事件源单例：全仓仅一处；取消订阅精确移除监听；重连不重复注册。
- 绝大多数事件委托为单次注册（带守卫）；定时器均有清理；防抖表为弱引用表。
- 受限渲染路径本身干净（投影替换时正确关闭常驻项、元素索引用弱引用、虚拟化适配器
  有 dispose 流程）。
- 主要 blob 路径（导出、下载、头像预览）均有释放逻辑；`download-bridge` 追踪全部
  临时 URL。
- **第二轮补充核实**：TTS 家族的 `onended = () => URL.revokeObjectURL(url)` 模式只覆盖
  正常播完路径；**打断/替换路径（F19）是全家族通病**（含 azure 等 11 个文件 + F4/F5
  特例），修复时统一处理 14 个文件，勿遗漏。真正完备的对照实现：`chatterbox.js:515`、
  `novel.js:189`、`kokoro.js:165,281`、`pollinations.js:124`（成功路径即 revoke）。

## 七、性能与占用优化（Wave 4）

先建基线（半天）：`?ttPerf=1` 记录 `tt:init:*`；DevTools Performance/Memory 各录一次
（启动 + 一次 500 令牌生成 + 一次聊天切换）；`tasklist` 记录进程内存与线程数；
`curl -w` 记录 5 个代表地址（index.html、lib.core.bundle.js、style.css、/__tt/health、
/__tt/file 聊天文件）的字节数与耗时。此后每项改动前后对比。

> **Wave 4 执行状态（2026-09-01，第一批）**：基线已采集（index.html 739KB、
> lib.core.bundle.js 781KB、style.css 148KB、script.js 562KB，全部 no-cache；
> 进程 27MB/32 线程）。H1/H3/H4/M5/M8 完成，L2/L3/M7(线程部分) 由 Wave 2 覆盖；
> H2/L1 暂缓（理由见下）。提交 `Wave 4: H1/H3/H4/M5/M8`（见仓库日志）。

### 快赢（改动小、收益大）

- **✅ H1 [静态资源与文件访问无缓存语义] `server/router.rs:281-315,330-375`** — 静态文件与
  `/__tt/file` 均为 `Cache-Control: no-cache` 且无校验器（无 ETag/Last-Modified），无 Range；
  每次刷新全量重传约 8MB 前端。**仓库内已有可复用模式**：host 资源服务已实现
  ETag/304/Range（`tt-application/.../host_resource_service/` 的 thumbnail.rs/response.rs）。
  修复：给两处加 Last-Modified + 条件请求（未修改则 304）+ Range；rspack 产物改用内容哈希
  文件名 + 长缓存，index.html 保持 no-cache。验证：`curl -w` 第二次请求应为 304/0 字节。
  **已实现**：`serve_file`/`serve_tt_file` 统一支持 Last-Modified（httpdate crate）+ 304、
  单段 Range（206/416）+ 流式 seek+take；HEAD 含元数据。`Cache-Control: no-cache` 保持
  （内容哈希长缓存随 H2 打包一并做）。验证：第二次 `curl -sI` 带 If-Modified-Since 应 304。
- **⏸️ H2 [主应用未打包] `rspack.config.js` + `src/init.js:283-301`** — 只有 3 个 vendor 包被
  构建；主应用（script.js 约 562KB、536 个模块、约 8MB 未压缩源码）以原始 ESM 逐文件下发，
  且 init 链串行（lib → host-main → script）。修复：把主应用入口纳入 rspack 打包 + 压缩 +
  代码分割，init 链改为并行加载（依赖顺序由模块图保证）。验证：`tt:init:total` 前后对比。
  **⏸️ 执行决策（2026-09-01，暂缓）**：构建链重构 + browser-esm-graph-contract 同步改造
  风险高，且 H1 落地后重复访问已命中 304（首屏 8MB 一次性成本仍在）。留作独立专项，
  与 M7 的 .gz 预压缩一起做。
- **✅ H3 [首屏同步脚本过多] `src/index.html:8384-8403`** — 14 个同步脚本（jquery-ui 249KB 等）。
  修复：非首屏必需库改为按需加载。验证：DevTools 首帧长任务分布。
  **已实现**：polyfill + jquery 保持同步，其余 12 个同步脚本加 `defer`
  （执行顺序保持、DOMContentLoaded 前完成，模块脚本语义不受影响）。
- **✅ H4 [流式渲染降频]（同 F16）** — 默认帧率降为 15-20，隐藏页 250ms。验证：生成时
  Performance 录段对比 `messageFormatting` 自采样时长与 TPS。**已实现**（F16：
  streaming_fps 默认 30→18；隐藏页 250ms 档已有）。
- **✅ M8 [每请求日志格式化] `presentation/commands/helpers.rs:19-25` + 167 处
  `log_command(format!(...))` 调用点** — `log_command` 本身已是 tracing 结构化字段；
  问题在调用点急切求值：日志级别关闭时 `format!` 仍无条件分配字符串（高频命令如
  list_chat_summaries 每次调用分配一次）。修复：`log_command` 增加惰性变体
  （如 `log_command_lazy(name, || format!(...))` 或接受 `&dyn Fn` 闭包），高频命令
  调用点改用之；或低频命令保持现状（改动面可控，不必 167 处全改）。
  验证：高频命令前后 CPU 对比。
  **已实现**：`helpers.rs` 新增 `log_command_lazy`（enabled 检查后惰性求值）；
  三个轮询类高频点改用：`verify_user_files`、`get_group_chat_payload_tail`、
  `get_data_archive_job_status`。其余调用点低频，保持现状。
- **✅ L3 [源映射随包下发] `src/lib/toastr.js.map`** — 构建/打包时剔除（或删除 B4）。
  验证：`curl -sI /lib/toastr.js.map` 应为 404。**已确认**：`src/lib/toastr.js.map`
  不存在（B4 已删），无需动作。

### 中收益

- **M1** 同 R6/R7（参数借用式反序列化 + 结果一次序列化）。
- **M2** 同 R5（请求体上限 32-64MiB + 长度预检）。
- **M3** 同 R19/R20（SSE 广播预序列化）。
- **✅ M4 [切换聊天全量下载] `src/scripts/app/chat/transport.js:34-56`** —
  依赖 H1 落地后，同一聊天再次打开应命中 304；大文件可加 Range。
  **已覆盖**：H1 为 `/__tt/file` 实现 Last-Modified 条件请求 + Range，无需前端改动。
- **✅ M5 [文件读取以数字数组传输] `server/fs_resources.rs:74-95`** — 字节以 JSON 数字数组
  返回（约 4.5 倍膨胀），用于备份/导入/角色卡路径。修复：改 base64 或直接复用
  `/__tt/file` 字节流。验证：备份导出时对比响应大小与磁盘文件大小。
  **已实现**：`plugin_fs_read` 返回 base64（膨胀 1.33 倍）；前端
  `readable-file-stream-service.js` 的 `normalizeFsReadResponse` 新增 base64 分支
  （保留数字数组分支兼容旧响应）。
- **✅ M6 [模板热路径重复编译] `src/script.js:3786-3792`（extractMessageBias）、
  `power-user.js:2536`（renderStoryString）** — 每条消息/每次生成重复编译 Handlebars 模板。
  修复：按内容缓存编译结果或轻量正则探测。验证：发送消息时 Performance 采样。
  **已实现**：`extractMessageBias` 用共享编译器 + 256 条 FIFO 模板缓存（bias helper
  每次调用重置收集数组，复用安全）；`renderStoryString` 32 条 FIFO 模板缓存。
- **⏸️ M7 [运行时与压缩] `server/mod.rs:88` + 构建配置** — 工作线程收敛（同 R21）；静态资源
  构建期预压缩 .gz + 按 Accept-Encoding 返回（避免运行时压缩 CPU）。验证：`tasklist` 线程数、
  `curl -H "Accept-Encoding: gzip"` 响应头。
  **部分完成**：R21 已收敛工作线程（2）。.gz 预压缩是构建产物配置，随 H2 打包专项一并做。

### 低收益（可后置）

- **⏸️ L1** 令牌事件每令牌触发扩展监听（`script.js:4629`）→ 合并到渲染节流。
  **执行决策（2026-09-01，暂缓）**：STREAM_TOKEN_RECEIVED 的每令牌语义是扩展可见契约
  （TTS 逐字朗读等消费方），合并进节流会改变事件频率，收益小（emit 本身廉价），风险高。
- **✅ L2** 同 R10（异步路径同步文件调用）。**已覆盖**（R10）。
- **✅ L4** SSE 心跳固定 15 秒 → 改为空闲心跳（有流量则不 tick）。
  **已实现**：`events.rs` 的 keepalive 改为 tick 时检查连接级活动计数，有事件帧则跳过
  ping（每连接独立计数）。
- **✅ L5** webfonts 合计约 9.5MB → 字体子集化 / `font-display: swap` 延迟。
  **已实现**：两个 webfonts stylesheet link 改 media-swap 非阻塞加载（@font-face 已有
  `font-display: swap`，文本先以回退字体渲染）。
- **✅ L6 [缩略图磁盘缓存累积] `tt-adapter-media/.../thumbnail_cache.rs`** — 缓存目录只增不减
  （仅 image-metadata 清理路径顺带删除；文件内唯一 remove_file 是临时文件清理），
  长期运行磁盘占用持续增长。修复：定期清扫失效条目（源文件已删除/变更的缩略图），
  或按总量上限淘汰最旧。
  **已实现**：`host_resources.rs` 每 256 次缩略图打开惰性清扫一次，三个缩略图目录
  超过 1200 个文件时按 mtime 删除最旧（缩略图可再生，按龄淘汰安全）。

### 已具备的正面机制（勿动）

聊天 DOM 虚拟化实现干净（受开关控制）；角色卡缩略图 + ETag/304/Range 缓存已就绪；
角色列表持久浅索引（启动不全量扫描）；聊天保存链设计良好（1 秒防抖 + 暂存 + 原子替换）；
HTTP 客户端未启用压缩解码（省 CPU）。

## 八、发布链路修复（Wave 5）

> **Wave 5 执行状态（2026-09-01）**：P0-1/P0-2/P0-3、P1-4/P1-5/P1-6/P1-7/P1-8 与
> P2 全部完成，提交 `Wave 5: P0-1..P0-3, P1-4..P1-8, P2`（`7869ca0d`）。
> 附带修复：`--resources` CLI 语义（web root 解析到 `<root>/src`），
> flatpak 布局 E2E 实测通过。另修复 **webui 根因级 bug**（`c2ab0253`，Wave 5 之前置）：
> invoke 命令信封 `{"Ok":...}`/`{"Err":...}` 从未解包，前端按裸值消费 → 全部数据
> 加载失败、首屏初始化崩溃。详见交接手册 §5.3。

### P0（迁移后发布流程从未完整跑通，务必先修）

- **P0-1 [产物重命名不匹配] `stable-release.yml` / `canary-release.yml` 的 Rename 步骤** —
  构建产物名为 `RustTavern-<平台>-<架构>.zip`（小写），workflow 期望大写名称。
  修复：统一两侧命名（建议以 `collect-release-assets.mjs` 的契约为准）。
- **P0-2 [构建产物缺失] CI/flatpak/nix 均以 `--skip-web-build` 运行且从不先执行 web:build** —
  而 rspack 产物（src/dist 等）被忽略、全新检出不存在 → 打包出的前端缺 bundle。
  修复：发布流程先 `web:build` 再打包；或 build-server 内改为"未检出产物则自动构建"。
- **P0-3 [flatpak 构建必然中断] `packaging/flatpak/build-app.sh`** — 安装源指向不存在的
  中间目录（build-server 只产 zip）；desktop 模板未渲染（`Icon={{icon}}` 占位符）；
  第 53-55 行死块（见 A16）。修复：改为从 zip 解包安装 + 渲染 desktop 模板 + 处理图标缺失
  （图标缺项为已知限制，需补图标或从 metainfo 移除图标声明）。

### P1

- **P1-4 [更新日志乱码] `canary-release.yml:181-186,260`** — 中文被双重转码成乱码，且
  260 行的匹配永远失败导致说明静默丢弃。修复：重写为 UTF-8 纯文本。
- **P1-5 [flatpak 依赖清单路径错误] `packaging/flatpak/pnpm-sources.json`** —
  92/95 个目标为反斜杠路径（Windows 生成）。修复：在 Linux 环境重新生成。
- **P1-6 [失效文档引用] `packaging/flatpak/README.md` + `docs/state/LinuxRepository.md`** —
  引用的 `pnpm run flatpak:*` 脚本不存在。修复：更新为实际命令（`packaging/flatpak/build.sh`）。
- **P1-7 [渠道描述与实现不符] 5 份 README 与 LinuxRepository.md 仍宣传 APT/RPM 软件源** —
  但发布流程已删除对应任务。修复：更新文档（或恢复任务，与 B5 二选一）。
- **P1-8 [flatpak 数据目录只读] 无启动包装传入 `--data-root`，沙箱内默认目录只读** —
  修复：包装脚本在启动时传入 `--data-root` 指向授权目录（finish-args 同步放行）。

### P2

- canary 输出多余的 deb/rpm 版本变量（无害，可清）；`.gitignore` 补充
  `stub-*.c`、`.flatpak-release/`、`flatpak-build.tar.zst`、`dist-fastools/`；
  `CONTRIBUTING.md` 残留 WebView/Tauri 措辞；`scripts/ci/verify-release-version.mjs` 已覆盖
  版本一致性（package.json=Cargo.toml=Cargo.lock=nix=2.2.0，`tauriVersion` 非硬编码）。

## 九、验证门（每 Wave 结束）

1. 相关契约测试：`node --test --test-concurrency=1 --test-timeout=30000 "tests/**/*.test.mjs"`
2. `pnpm run check`（本机串行参数见第一节；CI 无需）
3. E2E smoke：启动 → `/__tt/health` → `get_client_version` → 静态页 → 停止
4. 变更文件 `git status` 复核后提交，commit message 引用本文件编号（如 `Wave 1: A1-A17`）
5. 本文件头部更新状态与日期

## 十、核查结论汇总（供回溯）

- 服务端无 P0 级内存问题；24 小时运行场景主要增长源：会话表（R1）、文件句柄表（R2）、
  生成注册残留（R3/R4）。
- 前端无 P0 级确定性驻留；1 小时长聊天下主要压力：全文 DOM 挂载（F1）、每次保存的
  全文序列化（F2）、流式每令牌全文处理（F16）。
- 性能最大快赢：静态资源缓存语义（H1）与主应用打包（H2），两者改动面小、收益直观。
- 发布链路 P0×3 均为迁移提交引入、迁移后从未完整跑通，发布前必须修复。
- **第二轮核查补充**：新增 R25（WS 会话池）、R26（Agent 宿主等待）、R27（Vertex 凭据
  缓存）三个驻留源；前端新增 F17-F21，其中 **F19 是 TTS 预览打断未释放的家族通病
  （14 个文件统一修复，第一轮误判为仅 3 个文件）**、F18 是 AutoComplete 弹窗实例累积；
  A 档 12 项全部独立复核确认（A5-A8 路径纠正为 `scripts/`）；
  M8 实测 167 处调用点；全部 P1 主张逐行亲验与第一轮零偏差。
- **最终巡视（移交前）**：此前仅凭子代理报告的 10 处主张全部补验完毕
  （prompt_assembly.rs:202 无超时、repository.rs:126-137/161 快照驻留与无界通道、
  index.html:8384-8403 十四个同步脚本、power-user.js:2536 重复编译、assets.rs:52,97
  同步读、settings.rs:377 默认 false、background_commands.rs:68 整文件字节、
  ai-routes.js:490-493 置空回调）——全部成立，零偏差；B8 已验证可删、B9 升级为
  确定修复（feature 声明错误，见 B 档表）；开工前必读三条已写入零章。

## 十一、测试覆盖映射（执行模型改代码必查）

> 改动对应修复项时，按下表跑/改测试。路径相对仓库根；Rust 测试指 `#[cfg(test)]` 模块。

### 需更新断言的既有测试

| 修复项 | 测试 | 需更新的原因 |
|---|---|---|
| R19/R20/R22 | `crates/rusttavern/src/server/events.rs` 测试模块（:133-201） | `sse_stream_terminates_on_shutdown_signal` 逐字断言 `format_sse_event` 输出与 `Err(Lagged) => None` 行为——R22 要改静默丢帧、R19/R20 改广播载荷类型，测试须同步 |
| H2 | `tests/browser-esm-graph-contract.test.mjs` | 以 index.html 为入口解析 ESM 图并校验串行 init 链（STARTUP_ENTRY_MODULES）——改主应用打包/并行加载后须更新 |
| R20 | `crates/rusttavern/src/server/stream.rs` 测试模块（:149-201） | token 预序列化会改 StreamSink/广播载荷；实现时复核 3 个既有测试 |

### 既有测试保持不变（但别破坏其断言）

| 修复项 | 测试 | 注意 |
|---|---|---|
| R1 | `server/security.rs` 测试（:374-701，含 4 个会话测试） | 现有断言语义不变（Basic→Set-Cookie、cookie 复用、错误凭据 401） |
| R2/R9 | `tests/readable-file-stream-service.test.mjs`、`tests/chat-backup-route-contract.test.mjs:276-296` | 后者**读 fs_resources.rs 源码**断言含 `validate_server_path`/`data_root`/`starts_with`/`upload_staging_root`——改文件时保留这些符号名 |
| R5 | `tests/upload-service-contract.test.mjs`、`tests/chat-payload-commit-contract.test.mjs`、`chunk_body.rs` 测试 | 分块 512KiB ≪ 新上限，不受影响 |
| R6/R7 | `server/dispatch.rs` 测试（:253-301） | extract_arg 别名语义不变 |
| R16 | `thumbnail_cache.rs` 测试（:565-740）、`host_resource_service/thumbnail.rs` 测试 | 均测同步函数，spawn_blocking 在异步调用方 |
| H1 | `host_resource_service/response.rs` 测试（:325）、`tests/host-resource-*.test.mjs` | response.rs 是 H1 的参考实现（勿动）；前端 SW 契约已支持 304 |
| H2 | `tests/rusttavern-sync-vue-contract.test.mjs` 等 3 个 vue 契约 | 断言 rspack.config.js 含 settings dist 路径——加主应用入口不破坏；引入 contenthash 后复核产物文件名硬编码 |
| F2 | `tests/chat-payload-commit-contract.test.mjs` | 若保持 begin/append/finish 命令契约则不变（实现时定） |
| F13/F14 | `tests/host-bridge-contract.test.mjs` | 现无 createChannel 断言，既有断言保持 |
| F17-F21 | 无既有覆盖 | F17/F17b/F18/F20 修 quick-reply/autocomplete/tts 局部文件，无契约测试断言；实现时以 `pnpm run check:frontend` + tsc 为门 |

### 需新增测试（当前无覆盖）

R1（会话上限/过期清扫/带 cookie 不重签）；R2/R9（rid 惰性驱逐、read 释放锁、len 上限）；
R5（上限收紧 + Content-Length 预检 413）；R7（宏单次序列化）；R20（token 预序列化）；
R22（积压时终止帧并关闭）；F13/F14（createChannel 中止）；H1（静态 304/Range）；
H2（主 bundle 产物 + init 并行契约）。


