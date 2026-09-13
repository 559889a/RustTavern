<div align="center">

# RustTavern

[![CI](https://github.com/559889a/RustTavern/actions/workflows/ci.yml/badge.svg)](https://github.com/559889a/RustTavern/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/559889a/RustTavern)](https://github.com/559889a/RustTavern/releases)
[![License](https://img.shields.io/github/license/559889a/RustTavern)](LICENSE)

[简体中文](README.md) · **English**

</div>

**RustTavern** turns [SillyTavern](https://github.com/SillyTavern/SillyTavern) into a local HTTP server:
**the frontend keeps the upstream experience** (synced with 1.18.0) while **the backend is rewritten from
Node.js into Rust**. Download a package, unpack it, `./start.sh start`, then reach the WebUI from any device's
browser — no Node.js runtime, no command-line wizardry. Character cards, chat history, presets, world info and
frontend extensions all stay compatible.

## Features

- **A plain binary server, no runtime to install** — the backend is a single Rust binary (axum + tokio);
  no Node.js, no npm ecosystem. Frontend assets and default content ship inside the package.
- **Full SillyTavern experience** — frontend synced with upstream 1.18.0; the data layout
  (`default-user`, `characters`, `chats`, `group chats`, and the exact casing and spacing of
  `User Avatars`, `QuickReplies`, …), chat JSONL, character-card PNG metadata, world info, presets and
  themes all keep upstream semantics.
- **Frontend extension ecosystem** — native Git built in (gix smart HTTP plus an embedded worktree):
  install, update, switch branches and delete from the UI, with no system git. Upstream Node-only backend
  plugins are not supported.
- **Built-in multi-device sync** — encrypted LAN pairing (via `tauritavern://` pairing links) or remote
  upload through TT-Sync v2, with sync-job orchestration and status push.
- **Agent framework** — tool calls, Skills, sub-agents, a run timeline, workspace read/write and
  checkpoint/commit; agents can consume chat search, world info and workspace tools.
- **In-app data migration** — import a SillyTavern data archive directly; cards, chats and extensions
  come along.
- **Performance work aimed at long chats** — staged startup, virtualized chat rendering (viewport plus
  true tail), a streaming render policy and incremental token caching keep very long chats responsive.
- **Your data stays yours** — everything is stored locally; point it elsewhere with `--data-root` or the
  `dataRoot` field in `config.yaml`.
- **Secure by default** — listens only on `127.0.0.1` out of the box; binding a non-loopback address
  **requires** `security.authMode: basic` or `security.whitelist` in `config.yaml`, otherwise the server
  refuses to start (fail fast, never a silent downgrade).
- **Service control scripts** — `start.sh` / `start.ps1` give you start, stop, restart, status, health,
  url and logs, with PID verification, port probing and a health wait. See below.

## Quick Start

Download the package for your platform from
[Releases](https://github.com/559889a/RustTavern/releases) and unpack it — you get the `rusttavern`
binary, `src/` (frontend), `default/` (default content), `config.yaml` and the start scripts.

Release asset names:

| Platform | Asset |
| --- | --- |
| Windows x64 | `RustTavern-<version>-windows-x64.zip` |
| Linux x64 | `RustTavern-<version>-linux-x64.zip` |
| macOS arm64 | `RustTavern-<version>-macos-arm64.zip` |
| Android / Termux arm64 | `RustTavern-<version>-android-arm64.tar.gz` |

Every asset ships with a matching `.sha256` file.

### Windows

```powershell
# after unpacking, from inside the directory
.\start.ps1 start      # start and wait for /__tt/health to pass
.\start.ps1 status     # process, memory, port, access control, health, URLs
.\start.ps1 url        # print local and LAN URLs
.\start.ps1 logs 80    # tail the last 80 lines of server.log
.\start.ps1 stop       # stop by PID and confirm the port is released
```

`start.cmd` provides the same commands for people who would rather not use PowerShell. The first time you
bind a non-loopback address, Windows shows a firewall prompt — allow it for private networks.

### Linux / macOS

```bash
unzip RustTavern-<version>-linux-x64.zip
cd RustTavern-<version>-linux-x64
./rusttavern                 # foreground; http://127.0.0.1:8000 by default
```

On Linux you can also use the bundled `packaging/termux/start.sh` — it needs no Termux-specific command
and works the same on any Linux:

```bash
./start.sh start | stop | restart | status | health | url | logs [N]
```

### Termux (Android)

The Termux build is a native `aarch64-linux-android` binary (bionic libc, not a glibc build).

```bash
pkg install libc++
tar -xzf RustTavern-<version>-android-arm64.tar.gz
cd rusttavern-termux-arm64
./start.sh start

# Android is aggressive about killing background apps; the script already takes
# a wake lock while running. Add one more if you need it:
termux-wake-lock
```

No VPN app needed on the phone: put the phone and your computer on the same LAN, start with
`TT_HOST=0.0.0.0 ./start.sh start`, then open `http://<computer-lan-ip>:8000` in the phone's browser.

## Service control scripts

Three scripts, identical functionality, one per platform:

| Script | Platform | Notes |
| --- | --- | --- |
| `packaging/termux/start.sh` | Termux / Linux / macOS | POSIX shell; takes a wake lock under Termux |
| `packaging/windows/start.ps1` | Windows | native PowerShell, recommended |
| `packaging/windows/start.cmd` | Windows | cmd variant, same commands as `start.ps1` |

Commands:

| Command | What it does |
| --- | --- |
| `start` | Start and wait for `/__tt/health`; a crash-on-startup prints the log tail instead of pretending success |
| `stop` | Stop by PID and poll process *and* port until both are actually gone |
| `restart` | stop + start |
| `status` | Process, memory, port, access control, health, data root, log path, URLs |
| `health` | Print just the health HTTP code; exits 0 only on 200 |
| `url` | Print local and LAN URLs |
| `logs [N]` | Tail the last N log lines (default 40) |

Environment overrides (per invocation, no editing required):

```bash
TT_HOST=0.0.0.0 ./start.sh start     # bind all interfaces (configure access control first)
TT_PORT=9000 ./start.sh start        # different port
TT_DATA_ROOT=/srv/tt ./start.sh start
```

```powershell
$env:TT_HOST = '0.0.0.0'; .\start.ps1 start
```

Platform differences the scripts exist to handle: **an upgrade must stop first** (a running executable
cannot be overwritten); **the PID file is cross-checked against the process image name** (a stale PID can be
reused by an unrelated process after a reboot); **port state is probed by connecting rather than by parsing
netstat** (column layout and wording differ across versions and languages); and a **403 from `/__tt/health`
still counts as a successful start** — it means the server is answering and only this machine's address is
missing from `security.whitelist`.

## Command line

```bash
rusttavern [--host 127.0.0.1] [--port 8000] [--data-root <dir>] [--config <file>] [--resources <dir>] [--no-open-browser]
```

| Flag | Meaning |
| --- | --- |
| `--host` | Listen address, default `127.0.0.1` |
| `--port` | Listen port, default `8000` |
| `--data-root` | Data directory; takes precedence over `dataRoot` in `config.yaml` |
| `--config` | Config file path, default `<exe dir>/config.yaml` |
| `--resources` | Resources root (contains `default/` and `src/scripts/templates/`) |
| `--no-open-browser` | Do not open a browser on startup (services and mobile) |

## Configuration

Configuration lives in `config.yaml` next to the executable, generated on first start:

| Field | Meaning |
| --- | --- |
| `listen.host` / `listen.port` | Listen address and port (default `127.0.0.1:8000`) |
| `dataRoot` | Data directory (default `<exe dir>/data`; overridden by `--data-root`) |
| `autoOpenBrowser` | Whether to open a browser on startup |
| `security.authMode` | `basic` enables username/password auth (authenticated on every request), or `none` |
| `security.username` / `security.password` | Credentials for `authMode: basic` |
| `security.whitelist` | Allowed peers: single IPs, CIDR, and `192.168.1.*` wildcards |

> [!IMPORTANT]
> Binding a non-loopback address requires at least one access control (`authMode: basic` or `whitelist`),
> otherwise the server **refuses to start**. That is deliberate: a LAN-reachable, unauthenticated server
> that can read and write local files is not a safe default to accept silently.

Authentication uses Basic / session cookies over plain HTTP. Put an HTTPS reverse proxy in front if you
expose it to the internet.

## Data and compatibility

The compatibility target is the **SillyTavern semantics observable by browsers, extensions and user data** —
not the internals of Node/Express.

RustTavern's private state (agent workspace, agent profiles, Skills, prompt cache, LLM connections) lives
under `_tauritavern/` inside the data root.

> [!NOTE]
> **About the `tauritavern` strings still in this repository**: RustTavern is a fork of a fork — SillyTavern
> upstream, TauriTavern (the original Tauri desktop-shell version) in between. Because the on-disk format has
> to stay backward compatible, the following names are **deliberately preserved**; they are compatibility
> contracts rather than leftovers, and renaming them would orphan existing users' data and pairing links:
>
> | Preserved name | Where |
> | --- | --- |
> | `_tauritavern/` | Private state directory under the data root |
> | `tauritavern-settings.json` | Settings file under `default-user/` |
> | `data.extensions.tauritavern` | Character-card / preset extension key |
> | `extra.tauritavern` | Chat message metadata |
> | `tauritavern://` | LAN-sync pairing link scheme |
> | `SILLYTAVERN_*` | Upstream environment-variable contract |

## Building and developing

**Prerequisites**: Rust stable (edition 2024 support) · Node.js 20.19.x or 22.12+ · pnpm

```bash
git clone https://github.com/559889a/RustTavern.git
cd RustTavern
pnpm install
pnpm run dev            # Rspack watch + cargo run in parallel, opens the browser
```

Common commands:

```bash
pnpm run build          # release build: frontend bundle -> release binary -> release/*.zip
pnpm run check          # frontend guardrails + types + logging + crate boundaries + contract tests + clippy
pnpm run web:build      # build only the frontend bundle (rspack)
pnpm run server:build   # build only the Rust server (debug)
pnpm run test:contracts # frontend contract tests
```

> [!TIP]
> On a stripped Windows image, kernel32 may lack the `WaitOnAddress` family, which makes any Rust binary
> exit with `0xc0000139`. The `stub-synch.c` files at the repository root and `scripts/patch-imports.mjs`
> exist for exactly that environment, are gitignored, and are not part of the product.

## CI/CD

| Workflow | Trigger | What it does |
| --- | --- | --- |
| [`ci.yml`](.github/workflows/ci.yml) | push to `main`, PR | frontend guardrails / types / logging + crate boundaries / contract tests; Rust clippy + `cargo test`; three-platform debug artifacts |
| [`debug-build.yml`](.github/workflows/debug-build.yml) | manual `workflow_dispatch` | three-platform debug artifacts for pre-release smoke testing on a real device |
| [`release.yml`](.github/workflows/release.yml) | `v*` tag push | verify the tag matches `Cargo.toml`/`package.json`, package four platforms, attach sha256, create the GitHub release |

To release, set the same version in `Cargo.toml` and `package.json`, commit, then tag:

```bash
git tag v2.3.0 && git push origin v2.3.0
```

If the tag does not match both manifests, `release.yml` fails before building anything — a release can never
ship a binary that reports a version the repository does not declare.

## Project structure

```
RustTavern/
├── Cargo.toml                    # Rust workspace root (members, deps, dev/release profiles)
├── package.json                  # frontend toolchain and pnpm scripts
├── rspack.config.js              # frontend bundle build (vendor / agent-system / settings panels)
├── default/                      # first-run content scaffold (config.yaml template, presets, themes)
├── resources/                    # assets shipped alongside the binary (Claude tokenizer)
├── .github/workflows/            # ci.yml / release.yml / debug-build.yml
├── crates/                       # Rust workspace members (table below)
├── docs/                         # architecture, frontend, API and implementation-state docs
├── packaging/
│   ├── termux/start.sh           # service control (Termux / Linux / macOS)
│   ├── windows/start.ps1         # service control (Windows PowerShell)
│   └── windows/start.cmd         # service control (Windows cmd)
├── scripts/
│   ├── build-server.mjs          # frontend bundle -> cargo release -> zip
│   ├── pack-termux.mjs           # assemble the Termux runtime package (tar.gz)
│   ├── dev-server.mjs            # development mode (rspack watch + cargo run)
│   ├── check-*.mjs               # the four engineering guards (frontend, preload, logging, crate boundaries)
│   └── pack-dist.mjs             # pre-distribution privacy/path leak scan
├── tests/                        # frontend contract tests (node --test)
└── src/                          # frontend
    ├── index.html                # WebUI entry
    ├── script.js                 # upstream SillyTavern main script + RustTavern injection
    ├── host-bridge.js            # HTTP invoke bridge (POST /__tt/invoke/{command})
    ├── host/main/                # Host Kernel: intercepts fetch/jQuery.ajax and routes to the Rust server
    ├── scripts/app/              # frontend feature modules (chat, regex, settings panels, startup, perf)
    ├── scripts/rusttavern/       # RustTavern-specific modules (agent, layout-kit, ios-policy)
    ├── scripts/extensions/       # bundled extensions (agent-system, data-migration, code-render, ...)
    └── locales/ css/ img/ sounds/ webfonts/
```

The backend is a Cargo workspace following Clean Architecture — source dependencies may only point inwards
(enforced by `scripts/check-rust-crate-boundaries.mjs`):

| crate | Responsibility |
| --- | --- |
| `rusttavern` | axum HTTP server host, command dispatch layer and composition root |
| `tt-application` | use cases, services, policy orchestration |
| `tt-domain` | domain models, value objects, domain errors, pure rules |
| `tt-contracts` | cross-crate DTOs, events, payloads, host-resource contracts |
| `tt-ports` | repository / gateway / runtime traits |
| `tt-adapter-http` | shared HTTP client pool / profiles |
| `tt-adapter-provider-http` | LLM, SD, Translate, TTS, provider metadata |
| `tt-adapter-tokenization` | tokenizer |
| `tt-adapter-storage-core` | `DataDirectory`, base filesystem and base storage |
| `tt-adapter-storage-userdata` | character cards, world info, agent workspace/profile, Skills |
| `tt-adapter-media` | avatars, backgrounds, user media, host resources |
| `tt-adapter-extension` | third-party extension discovery, install, update, branch switching |
| `tt-adapter-sync` | LAN Sync, TT-Sync v2 runtime and sync jobs |
| `tt-adapter-archive` | data archive import/export |

Request path: `frontend/extension → same-origin fetch → src/host/main/interceptors.js → routes →
host-bridge.js → POST /__tt/invoke/{command} → presentation command → tt-application service →
tt-ports trait → tt-adapter-*`.

## Documentation

The docs tree is layered by **purpose** (architecture / extension API / contracts / current state / history). Full index: **[docs/README.md](docs/README.md)**. Common entry points:

- [docs/architecture/ProjectStructure.md](docs/architecture/ProjectStructure.md) — directory-by-directory map of what lives where and what calls it
- [docs/architecture/TechStack.md](docs/architecture/TechStack.md) — tech stack and engineering guards
- [docs/architecture/BackendStructure.md](docs/architecture/BackendStructure.md) — backend Clean Architecture and crate boundaries
- [docs/architecture/FrontendGuide.md](docs/architecture/FrontendGuide.md) — frontend architecture and extension guide
- [docs/architecture/FrontendHostContract.md](docs/architecture/FrontendHostContract.md) — host-layer contract
- [docs/api/README.md](docs/api/README.md) — `window.__RUSTTAVERN__.api.*` extension API reference
- [docs/contracts/](docs/contracts/) — long-lived cross-module contracts (provider state, character identity, host-resource caching, …)
- [docs/state/](docs/state/) — current implementation snapshots per module
- [docs/history/](docs/history/) — completed migration and implementation records (archive)

## License and credits

Built on top of [SillyTavern](https://github.com/SillyTavern/SillyTavern). Released under
[AGPL-3.0](LICENSE) (the same license family as SillyTavern).
