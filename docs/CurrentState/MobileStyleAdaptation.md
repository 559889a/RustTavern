# 移动端样式适配现状（Safe‑Area / 沉浸模式）

本文档描述移动端样式与布局适配现状。项目已从 Tauri 原生应用（Android / iOS）迁移为「本地 HTTP 服务器 + 浏览器 WebUI」（B/S 形态）：Android Kotlin 与 iOS WKWebView 的 native 注入代码已全部删除，但**前端移动端兼容层保留**——手机 / 平板浏览器访问 WebUI 时仍受益。以下只描述浏览器 / 前端视角的现状。

## 1. 范围与结论

1. **Insets 是纯 CSS 契约**：`--tt-inset-*` 完全来自 `env(safe-area-inset-*)` 默认值（iOS Safari / Android Chrome 均原生支持），没有任何 native `WindowInsets` 监听与注入。
2. **IME 不再有 inset 注入**：`--tt-ime-bottom` 恒为 `0`；移动浏览器键盘弹出是 viewport resize 行为（`src/index.html` meta viewport 含 `interactive-widget=resizes-content`），surface-local IME 注入机制已失去输入方。
3. **沉浸模式不可用**：native system bars 控制已删；`src/scripts/mobile-system-ui.js` 模块保留但其 bridge 调用恒不可用（见 §5）。
4. **第三方浮层 surface classifier + geometry firewall 仍生效**：`data-tt-mobile-surface` 契约与 `<style id="tt-mobile-geometry-firewall">` 几何修正均保留，浏览器形态下手机 / 平板浏览器照常受益。

本目录记录「现状快照」。历史 native 形态的推导与实现文档已随迁移删除。

## 2. 安装链路与「兼容保留」边界

`src/init.js` 无条件设置 `window.__TAURI_RUNNING__ = true`，因此浏览器形态下 `src/host-main.js` → `bootstrapTauriMain()` 仍会执行；当 `navigator.userAgent` 命中移动端（android / iphone / ipad / ipod，或 MacIntel + 触摸屏）时安装以下模块（位于 `src/host/main/compat/mobile/`）：

- 无条件安装：`mobile-runtime-compat.js`、`mobile-geometry-firewall.js`、`mobile-overlay-compat-controller.js`
- 仅 Android UA（`isAndroidRuntime()`，纯 UA 判定）：`android-ime-layout-host.js`、`mobile-ime-surface-controller.js`
- 仅移动 UA：`mobile-iframe-viewport-contract-bridge.js`、`mobile-window-open-compat.js`

注意：`android-ime-layout-host.js` 虽以 android 命名，但代码是纯前端 DOM 逻辑（composer lift / spacer），无 native 依赖。

## 3. CSS 变量契约（纯浏览器）

`src/style.css` 提供默认值：

- `--tt-inset-top/right/left/bottom: env(safe-area-inset-*, 0px)`
- `--tt-ime-bottom: 0px`（无 native 注入方，恒为 0）
- `--tt-viewport-bottom-inset: max(var(--tt-inset-bottom), var(--tt-ime-bottom))`
- `--tt-base-viewport-height` 无定义处；所有消费点都以 `var(--doc-height, 100vh/100dvh)` 兜底。`--doc-height` 由 `src/index.html` 在 `load`/`resize` 时更新为 `window.innerHeight`（已核实仍有效）。

`viewport-fit=cover` 由 `src/index.html` 的 meta viewport 提供（已核实）。`src/css/mobile-styles.css`（`<link>` 直接引入，≤1000px 生效）与 `mobile-geometry-firewall.js` 消费以上变量。

## 4. 保留的前端机制（仍生效）

### 4.1 surface classifier + geometry firewall（第三方浮层契约）

- **分类**：`mobile-overlay-surface-admission.js` 对可见的 `position: fixed` 节点分类，输出 `data-tt-mobile-surface="backdrop|viewport-host|fullscreen-window|free-window|edge-window"`、host-private `data-tt-mobile-surface-admitted="1"`，edge-window 场景写 `--tt-original-top`；排除 `body/#sheld/#chat` 等核心容器；尊重显式 `data-tt-mobile-surface` opt-in。
- **观察**：`mobile-overlay-compat-controller.js` 只观察 `document.body` 直系子节点（`subtree: false`），对带 `script_id` 的 portal root 扫描子树；只监听候选 surface 的 `class/style/hidden/open/aria-hidden` 属性，按 animation frame 合并；不监听 `visualViewport/resize/orientationchange` 噪声；`revalidate()` 保留为手动兜底；以 `window.__RUSTTAVERN_MOBILE_OVERLAY_COMPAT__` 暴露 controller。
- **几何落地**：`mobile-geometry-firewall.js` 注入 `<style id="tt-mobile-geometry-firewall">`（keep-last，保证始终为 `<head>` 最后一个 element）：
  - 核心容器：`#top-settings-holder/#top-bar` 按 `--tt-inset-top/left/right` 定位；`#sheld` 的 `height/min/max-height` 以 `topBarBlockSize + inset-top` 与 `--tt-base-viewport-height/--doc-height` 同源计算
  - surface selector 刻意重复 attribute 以提高 specificity（覆盖框架 scoped CSS + `!important`）：edge-window 只修 top；fullscreen-window 修四边并把 width/height 改 auto；viewport-host 强制 full-bleed；backdrop 保持 full-bleed
  - iPad 宽屏（>1000px）块：保持桌面布局，仅强制 safe-area top 偏移
  - 滚动可达性：对常见 scroll container 注入 `::after` spacer，高度用 `--tt-viewport-bottom-inset`
- **iframe 契约**：`mobile-iframe-viewport-contract-bridge.js` 把 `--tt-inset-*`、`--tt-viewport-bottom-inset`、`--tt-base-viewport-height` 快照同步进同源 iframe（`__RUSTTAVERN_MOBILE_IFRAME_VIEWPORT_CONTRACT_BRIDGE__`）。
- **window.open 兼容**：`mobile-window-open-compat.js` 将外部协议（http/https/mailto/tel）走 `openExternalUrl`，同源保留原行为。

### 4.2 IME surface controller（DOM 属性保留，inset 注入已死）

- `mobile-ime-surface-controller.js`（仅 Android UA 安装）：监听 `focusin/focusout/pointerdown`（capture），解析 active surface root 并写 `data-tt-ime-active` / `data-tt-ime-surface="composer|fixed-shell|dialog"`——纯 DOM 标记仍生效。
- **但** `window.__RUSTTAVERN_INSETS__` 已无安装方（全库 grep 仅此一处读取，原为 native 注入）：需要调用 `setImeTarget` 的路径会因 bridge 缺失而抛错（uncaught），`--tt-ime-bottom` 无任何注入、恒为 `0`。
- `android-ime-layout-host.js` 的 lift/spacer 结构仍在 Android UA 下安装（`#form_sheld` 内 `data-tt-android-ime-lift` / `data-tt-android-ime-spacer`），但位移由 `--tt-keyboard-offset`（依赖 `--tt-ime-bottom`）驱动，恒为 `0` → 惰性无位移。
- 结论：浏览器键盘 = viewport resize，surface-local IME 注入机制不再有输入方，属「兼容保留但已死」；geometry firewall 中对应的 IME 规则随 `--tt-ime-bottom = 0` 自然退化为纯 safe-area 行为。

### 4.3 runtime compat polyfills

`mobile-runtime-compat.js`：补齐 `requestIdleCallback`/`cancelIdleCallback`、`Array/String.prototype.at`、`findLast`/`findLastIndex`、`toSorted`/`toReversed`、`Object.hasOwn`；`window.__RUSTTAVERN_MOBILE_RUNTIME_COMPAT__` 哨兵保证只执行一次；同源 iframe / `window.open` 窗口也会安装。

### 4.4 聊天输入焦点策略

`src/scripts/chat-input-focus.js`（保留）：移动 UA 下拒绝 `navigation` / `restoration` 意图的程序化聚焦（切角色、读历史、welcome screen 等不会弹键盘），`editing` 仍允许；文档进入 `hidden` 时若输入框持焦则 `blur()` 并清空 restoration 状态（`__TAURI_RUNNING__ === true` 且 Android UA，B/S 形态下仍触发）。

## 5. 已随 native 删除、不再生效

- `crates/rusttavern/gen/android/**` 已整体删除（`gen/` 下仅剩 `schemas/`）；全部 Kotlin 源文件、iOS WKWebView 相关处理（`ios_webview.rs`、`apple_webview_js_dialogs` 等）已删，Rust 侧无任何残留引用（已 grep 核实）。
- native bridge：`RustTavernAndroidSystemUiBridge`、`WebViewInsetsStyleApplier`、`AndroidInsetsBridge`、`WebViewReadinessPoller`——全部删除。
- `window.__RUSTTAVERN_INSETS__`（曾由 native 注入）：已无安装方，`apply/setImeTarget/reapply` 不存在。
- **沉浸模式开关**：`src/scripts/mobile-system-ui.js` 模块与 power-user 入口（`mobile_immersive_fullscreen`）保留，但 `getAndroidSystemUiBridge()` 永远拿不到 bridge → `isMobileImmersiveFullscreenSupported()` 返回 false、`setMobileImmersiveFullscreenEnabled()` 返回 false（无操作）、`getMobileImmersiveFullscreenEnabled()` 返回 null。B/S 形态下没有隐藏 / 显示 system bars 的能力，`--tt-inset-*` 不会因沉浸模式归零。
- `--tt-ime-bottom` 恒为 `0`：键盘弹出不再产生 IME inset，只有 viewport resize。

## 6. 已支持 / 明确不支持（浏览器形态）

已支持：

- `env(safe-area-inset-*)` 驱动的 `--tt-inset-*` 契约（iOS Safari / Android Chrome 均原生支持），`viewport-fit=cover` 由 meta 提供。
- 第一方顶部 UI 与 `#sheld` 的 safe-area 几何约束（geometry firewall，≤1000px 与 iPad 宽屏两条路径）。
- 第三方脚本 fixed 浮层的 surface classifier + safe-area 修正（`data-tt-mobile-surface`）。
- 移动浏览器键盘弹出 / 收起（viewport resize）下布局稳定：`--doc-height` + `--tt-viewport-bottom-inset`。
- 移动端聊天输入焦点策略（不自动弹键盘、后台返回不恢复焦点）。
- 旧浏览器能力补齐（runtime compat polyfills）。

明确不支持 / 不承诺：

- **Android 沉浸模式开关**：native bridge 已删，B/S 下不可用（见 §5）。
- **IME inset 注入**：`--tt-ime-bottom` 恒为 0，surface-local IME 路由（`setImeTarget`）不可用。
- 不做第三方 `<style>` 文本 rewrite（风险高、回归面大）。
- overlay compat 不保证覆盖「非 body 直系子节点插入」的浮层；只处理 top safe-area，不做 left/right/bottom 通用兜底。

## 7. 最小回归与调试

建议最小回归（手机浏览器）：

1. 刘海 / 挖孔机型：第一方顶部 UI 与第三方脚本浮层避让顶部安全区（`--tt-inset-top` 反映 env 值）。
2. 键盘弹出 / 收起：`#sheld` 高度与输入框不被遮挡（viewport resize 行为）。
3. 旋转屏幕：safe-area 与布局无抖动回归。

快速调试点：

- `getComputedStyle(document.documentElement).getPropertyValue('--tt-inset-top')`——期望反映当前 safe-area（px）。
- `window.__RUSTTAVERN_MOBILE_OVERLAY_COMPAT__` 是否存在（controller 已安装）。
- `window.__RUSTTAVERN_MOBILE_RUNTIME_COMPAT__ === true`。
- `window.__RUSTTAVERN_INSETS__`——**应不存在**（native 已删，B/S 下无安装方）。
- 当前 active surface 是否正确打标：`[data-tt-ime-active][data-tt-ime-surface]`。
