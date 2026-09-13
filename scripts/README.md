# Scripts

这个目录存放仓库内的开发、构建与工程守护脚本。日常入口都通过根目录 `package.json` 的 pnpm script 调用。

## 构建与开发

- `build-server.mjs`
  构建服务器发布版：前端 bundle（`web:build`）→ cargo release 二进制 → 打 zip（二进制 + `src/` + `default/` + `templates/`）。
  对应 `pnpm run build`；`--skip-web-build` 跳过前端重建，`--out-dir` 指定输出目录。
- `dev-server.mjs`
  开发模式：并行启动 rspack watch（`web:dev`）与 cargo run（`server:dev`）。对应 `pnpm run dev`。
- `pack-dist.mjs`
  把已构建的发行产物打包成对外分发的压缩包。
- `pack-termux.mjs`
  打包 Android/Termux 的 aarch64 发行包。
- `local-android-build.sh`
  本机 Android 交叉编译辅助脚本（依赖本机 NDK 路径，见 `.cargo/config.toml`）。

## 工程守护

- `check-frontend-guardrails.mjs`
  校验前端宿主层的文件规模与依赖边界，避免 Host Kernel 持续膨胀。
  对应 `pnpm run check:frontend`；`--update-baseline` 刷新基线。
- `check-preload-hints.mjs`
  校验样式与模块 preload hint 的完整性。同样属于 `pnpm run check:frontend`。
- `check-logging-boundaries.mjs`
  守 logging target 的使用边界。对应 `pnpm run check:logging-boundaries`。
- `check-rust-crate-boundaries.mjs`
  守 Rust workspace 的 crate 依赖方向。对应 `pnpm run check:rust-boundaries`。
- `guardrails/frontend-lines-baseline.json`
  `check-frontend-guardrails.mjs` 使用的行数基线数据。
- `ci/verify-release-version.mjs`
  校验发布 tag 与前端、Cargo、Cargo lock 和 Nix 包版本一致。

## 本机专用

- `patch-imports.mjs`
  **本机测试环境专用，不属于产品**。本机是精简 Windows 镜像，kernel32 缺少
  `WaitOnAddress` 系列 API，导致 Rust 二进制启动即 `0xc0000139`。该脚本改写 exe 导入表，
  迫使 loader 加载同目录的桩 DLL（`stub-synch.c` 等，见根目录，已被 gitignore）。

## 维护约定

- 面向仓库内部的脚本，优先通过 pnpm script 或 CI 调用，不额外扩散入口。
- 新增守护脚本时，同步在根 `package.json` 的 `check` 链路里注册。
