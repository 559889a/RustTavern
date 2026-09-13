# Linux 分发现状

本文记录 RustTavern 当前的 Linux 分发方式、信任身份和维护边界。

> ⚠️ 状态（2026-09）：APT/DNF/Zypper 软件源的**发布流水线已移除**（迁移收尾
> Wave 1 删除了 `distribution/apt-rpm/`），当前 CI 只维护 Flatpak 软件源与
> Nix cache。下文 APT/RPM 相关内容为**历史记录**，`packages.rusttavern.com`
> 不再更新；已有安装可通过 `scripts/install-linux.sh --uninstall` 或包管理器
> 正常卸载。

## 支持范围

| 系统 | 入口 | 状态 |
| --- | --- | --- |
| Flatpak | 独立软件源 | **当前维护**，`x86_64`；`aarch64` 待验证 |
| Nix / NixOS | flake | **当前维护**，`x86_64-linux` / `aarch64-linux` |
| Debian/Ubuntu (APT)、Fedora (DNF)、openSUSE (Zypper) | 历史软件源 | **已停止发布**（历史记录见下文） |

Flatpak/Nix 形态没有 GUI/WebView 依赖（浏览器在客户端设备上），运行时仅需标准系统库与网络。

## 分发与信任（Flatpak，当前维护）

```text
https://flatpak.rusttavern.com
```

## 历史记录：APT/DNF/Zypper 软件源（已停止发布）

```text
https://packages.rusttavern.com
```

OpenPGP 身份：

```text
RustTavern Linux Repository <packages@rusttavern.com>

主密钥
C752 84E7 8972 F19A 0DDD 88C4 87F5 B853 0682 A857

签名子密钥
D609 D0B1 74E0 073B B398 0A1B EDC6 CEF9 24B6 C529
```

- 主密钥有效至 2036-07-22
- 签名子密钥有效至 2029-07-24
- 公钥：<https://packages.rusttavern.com/keys/rusttavern-archive-keyring.asc>
- 发布清单：<https://packages.rusttavern.com/repository-manifest.json>

Flatpak 软件源复用同一 OpenPGP 身份：

```text
https://flatpak.rusttavern.com/rusttavern.flatpakrepo
```

Nix binary cache：

```text
https://nix-cache.rusttavern.com
nix-cache.rusttavern.com-1:mOl/sCsfndubNIhnLODjA7GPqk1qw5iknbayZLRn92U=
```

OpenPGP 和 Nix cache key 是两套独立的信任机制，不复用私钥。

## 一键安装

所有受支持的 Linux 用户均可运行：

```sh
curl -fsSL https://raw.githubusercontent.com/559889a/RustTavern/main/scripts/install-linux.sh | sh
```

脚本使用 POSIX `sh` 语法，支持 sh、Dash、Bash 与 Zsh。Debian、Ubuntu、Fedora 和 openSUSE 会使用原生软件源；NixOS 自动使用 flake。其他已经安装 Nix 的 Linux 可通过 `sh -s -- --method nix` 显式选择 Nix。可先下载脚本并以 `--dry-run` 查看执行计划。

Canary 使用同一入口：

```sh
curl -fsSL https://raw.githubusercontent.com/559889a/RustTavern/main/scripts/install-linux.sh \
  | sh -s -- --channel canary
```

## 公钥验证

如需手动配置，先下载公钥并核对完整主密钥指纹：

```bash
key_file="$(mktemp)"
curl -fsSL \
  https://packages.rusttavern.com/keys/rusttavern-archive-keyring.asc \
  -o "$key_file"
gpg --show-keys --with-fingerprint "$key_file"
```

以下操作仅应在指纹一致后继续。

## Debian / Ubuntu

```bash
sudo install -d -m 0755 /etc/apt/keyrings
sudo install -m 0644 "$key_file" \
  /etc/apt/keyrings/rusttavern-archive-keyring.asc

architecture="$(dpkg --print-architecture)"
sudo tee /etc/apt/sources.list.d/rusttavern.sources >/dev/null <<EOF
Types: deb
URIs: https://packages.rusttavern.com/apt
Suites: stable
Components: main
Architectures: $architecture
Signed-By: /etc/apt/keyrings/rusttavern-archive-keyring.asc
EOF

sudo apt-get update
sudo apt-get install tauri-tavern
```

## Fedora / openSUSE

导入已经核对的公钥：

```bash
sudo install -d -m 0755 /etc/pki/rpm-gpg
sudo install -m 0644 "$key_file" \
  /etc/pki/rpm-gpg/RPM-GPG-KEY-rusttavern
sudo rpm --import /etc/pki/rpm-gpg/RPM-GPG-KEY-rusttavern
```

Fedora 仓库配置：

```ini
# /etc/yum.repos.d/rusttavern.repo
[rusttavern]
name=RustTavern
baseurl=https://packages.rusttavern.com/rpm/fedora/stable/$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-rusttavern
sslverify=1
```

```bash
sudo dnf makecache --refresh
sudo dnf install tauri-tavern
```

openSUSE Leap 16.0 仓库配置：

```ini
# /etc/zypp/repos.d/rusttavern.repo
[rusttavern]
name=RustTavern
type=rpm-md
baseurl=https://packages.rusttavern.com/rpm/opensuse/16.0/$basearch
enabled=1
autorefresh=1
gpgcheck=1
repo_gpgcheck=1
pkg_gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-rusttavern
```

```bash
sudo zypper --gpg-auto-import-keys refresh rusttavern
sudo zypper install tauri-tavern
```

## Nix / NixOS

项目根目录的 `flake.nix` 提供 packages、apps 与 checks，并由 `flake.lock` 固定 Nixpkgs。直接运行或构建：

```bash
nix run github:559889a/RustTavern
nix build github:559889a/RustTavern
```

Rust 依赖直接由 `Cargo.lock` 中的版本和 checksum 固定，不额外维护容易与 lockfile 漂移的整体 `cargoHash`。如果以后加入 Git 来源的 crate，需要在 `cargoLock.outputHashes` 中显式固定相应源码。
`pnpmDeps` 仍是一个整体的离线依赖仓库；`pnpm-lock.yaml` 变化后，需要按 Nix 构建报告的实际值手动更新其固定哈希。

通过安装脚本加入当前用户 profile：

```sh
curl -fsSL https://raw.githubusercontent.com/559889a/RustTavern/main/scripts/install-linux.sh \
  | sh -s -- --method nix
```

脚本不使用 sudo，也不修改声明式 NixOS 配置或 `/etc/nix/nix.conf`。

当前固定输入应使用包含 flake 的提交 SHA；后续 release tag 包含 flake 后，也可固定到 tag：

```bash
nix run github:559889a/RustTavern/<commit-sha>
nix run github:559889a/RustTavern/<release-tag>
```

multi-user Nix 需要由管理员配置信任：

```text
extra-substituters = https://nix-cache.rusttavern.com
extra-trusted-public-keys = nix-cache.rusttavern.com-1:mOl/sCsfndubNIhnLODjA7GPqk1qw5iknbayZLRn92U=
```

若 Nix 报告 `ignoring untrusted substituter`，缓存没有生效，仍会回退到本地构建。

当前验证状态：

- `x86_64-linux` 已完成离线依赖恢复、release 构建、动态库检查和隔离启动。
- `aarch64-linux` flake output 已通过求值，仍需原生 ARM64 runner 完成构建与启动验证。
- 服务器形态无 GUI 依赖，不需要 NixGL / `nix-system-graphics` 连接 GPU。

## Flatpak

`packaging/flatpak/` 提供基于 GNOME SDK 的源码构建 manifest。pnpm 与 Cargo
依赖由 lockfile 生成固定清单，构建阶段不访问网络；构建只产出服务器二进制并
安装 `src/`、`default/` 与模板资源，不复用任何桌面壳层。

安装 Stable：

```bash
flatpak remote-add --user --if-not-exists \
  rusttavern \
  https://flatpak.rusttavern.com/rusttavern.flatpakrepo
flatpak install --user rusttavern com.rusttavern.client
```

本地构建：

```bash
./packaging/flatpak/build.sh
```

当前 `x86_64` 已完成离线构建、AppStream/desktop 校验、动态库检查、Flatpak
bundle 导出、临时安装与隔离启动。服务器形态需要监听端口并把数据目录写到可写
位置：flatpak 沙箱内由 `rusttavern-wrapper` 启动包装传入 `--data-root`
指向 `$XDG_DATA_HOME/rusttavern`（manifest 放行 `xdg-data/rusttavern`），
manifest 使用 portal-first 权限，不开放整个 Home 目录。导入导出、拖放、
通知、媒体和 LAN Sync 仍需在真实桌面会话中逐项验收；`aarch64` 尚未作为正式
发布架构。桌面入口（`.desktop`）只是快捷方式，实际使用方式仍是终端启动 +
浏览器访问。

## 发布与缓存

正式版由 `.github/workflows/stable-release.yml` 在 Release 发布后构建。CI
保留人工编写的 Release notes，只追加各平台产物；APT/RPM、Flatpak 与 Nix
cache 在 Release 资产上传成功后更新，其中仓库上传均为非阻塞后置任务。

Canary 也在 GitHub Release 成功更新后发布 Linux 产物：

- APT suite：`canary`
- Fedora：`rpm/fedora/canary/$basearch`
- openSUSE：`rpm/opensuse/16.0/canary/$basearch`
- Nix：`nix run github:559889a/RustTavern/Canary#canary`

Canary CI 自动读取 GitHub 最新正式 Release 作为版本基线。以 2.1.1 为例，APT 使用 `2.1.1+canary.<run>.g<sha>`，RPM 使用 `2.1.1-2.canary.<run>.g<sha>`；它们高于当前正式包，同时低于任何后续正式版本。Canary 与 Stable 共用 Nix binary cache；Nix store path 按内容寻址，因此不会发生渠道覆盖。

## 维护边界

1. APT 通过 `InRelease` / `Release.gpg` 建立信任；RPM 同时签署软件包和 `repomd.xml`。
2. 私钥不进入 Git、R2、构建产物或日志。日常 OpenPGP 发布环境只持有签名子密钥。
3. 版本化负载和普通索引先上传，各仓库的签名指针最后更新；公钥独立维护，清单在发布完成后更新。
4. 可变入口使用 `Cache-Control: no-cache`；版本化软件包和哈希对象使用 `public, max-age=31536000, immutable`。
5. Nix cache 只接收从确定 Git revision 构建并由专用 Nix key 签名的 runtime closure、Cargo lockfile 依赖闭包和 pnpm dependency paths。
6. Stable tag 发布后不移动。Canary Nix 使用显式 `canary` 输出和提交身份，不替代默认 Stable 包。
7. 每次发布后从公开域名验证签名、索引和实际软件下载，不只验证 R2 私有端点。
8. OpenPGP 签名子密钥应在 2029-07-24 前完成轮换。
9. pnpm 或 Cargo lockfile 变化时同步更新并校验 Flatpak 离线依赖清单。
10. Flatpak 的哈希对象和静态增量长期缓存；summary、refs、描述文件和发布清单
    使用 `no-cache`，并在对象上传完成后更新。
