#!/usr/bin/env node

// Build a redistributable RustTavern archive: sanitized source + the release
// binary + the Windows service script, packed with the 7-Zip CLI.
//
// "Sanitized" means the archive is assembled from `git ls-files` (so nothing
// untracked or gitignored can leak in), minus an explicit deny list of files
// that are specific to this machine or this developer, plus the build outputs a
// recipient actually needs. Every staged file is then scanned for personal
// data patterns and packing aborts if any match survives.
//
// Usage:
//   node scripts/pack-dist.mjs [--out-dir release] [--binary <path>]
//                              [--no-binary] [--name <archive-base-name>]

import { spawnSync } from 'node:child_process';
import {
    copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync,
    rmSync, statSync, writeFileSync,
} from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const PRODUCT = 'RustTavern';

// Files that are tracked but must not be redistributed: they describe this
// machine's toolchain layout, this developer's workflow, or an internal network.
const EXCLUDED_TRACKED = [
    // Local build/verification harness notes: exact local paths, local ports,
    // the phone deployment topology and its LAN addresses.
    'docs/CurrentState/NextHarnessHandoff.md',
    // Same category as the handoff notes above: absolute workspace paths, the
    // phone's ssh key filename and port, and this network's addresses.
    'docs/CurrentState/ClaudeCodeHandoff.md',
    // Frontend-perf handoff brief for the next (browser-capable) harness on
    // this machine: same category — local paths and workflow notes.
    'docs/CurrentState/FrontendPerfHandoff.md',
    // Migration work log: records this machine's cargo mirror config, absolute
    // workspace paths and offline-CRL workarounds. Project history, not
    // something a recipient can act on.
    'docs/CurrentState/HttpServerMigrationLog.md',
    // Termux packaging: the ship command carries an ssh port and key filename,
    // and the verification record names the developer's own subnet.
    'packaging/termux/README.md',
    'scripts/pack-termux.mjs',
    // References /c/tmp/ndk and this machine's rustup/Strawberry layout.
    'scripts/local-android-build.sh',
    // This packer and the doc describing it spell out the deny list below, so
    // they contain every sensitive pattern as literal text. They are maintainer
    // tooling; recipients get README-DIST.md instead.
    'scripts/pack-dist.mjs',
    'packaging/windows/README.md',
];

// Directories whose tracked contents are development-only. Recipients get the
// runnable server plus the source that produces it, not the CI/flatpak/Nix
// plumbing or the Termux deployment scripts (which target another platform and
// are documented only in the file this package omits).
const EXCLUDED_TRACKED_PREFIXES = [
    '.github/',
    '.flatpak/',
    'packaging/flatpak/',
    'packaging/termux/',
    'distribution/',
    'nix/',
];

// Untracked build outputs the archive needs. Without the rspack bundles the
// frontend cannot boot (lib.js imports ./dist/lib.core.bundle.js).
const REQUIRED_WEB_BUNDLES = [
    'src/dist/lib.core.bundle.js',
    'src/dist/lib.optional.bundle.js',
    'src/scripts/extensions/agent-system/dist/index.bundle.js',
    'src/scripts/app/setting/dist/settings.bundle.js',
    'src/scripts/app/setting/dist/sync.bundle.js',
    'src/scripts/app/setting/dist/dev-logs.bundle.js',
];

// Patterns that must not appear anywhere in the staged tree. These are the
// concrete leaks worth guarding: this developer's home directory, the phone
// deployment's ssh identity and port, and any private IPv4 literal.
const FORBIDDEN_PATTERNS = [
    // Any explicit ssh identity file, not one developer's key name.
    { name: 'ssh identity file', regex: /id_(?:rsa|dsa|ecdsa|ed25519)/ },
    // Termux's default sshd port; a bare 8022 is only interesting next to ssh.
    { name: 'Termux ssh port + key', regex: /ssh\s+-p\s*8022/ },
    { name: 'local user profile path', regex: /[Cc]:[\\/]+Users[\\/]+Administrator/ },
    { name: 'local workspace path', regex: /ST_workspace/ },
    { name: 'local NDK path', regex: /[Cc]:[\\/]+tmp[\\/]+ndk/ },
    { name: 'private LAN address', regex: /\b(?:192\.168|10\.(?:\d{1,3})\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}\.\d{1,3}\b/ },
];

// Files that legitimately contain digit sequences resembling IPv4 literals, or
// document the address syntax on purpose. Checked against the pattern name so a
// waiver cannot silently cover an unrelated leak.
const SCAN_WAIVERS = [
    // Config templates and security docs must show what a whitelist entry looks
    // like; these are syntax examples, not this network.
    { file: 'crates/rusttavern/src/server/security.rs', allow: ['private LAN address'] },
    { file: 'crates/rusttavern/src/server/config.rs', allow: ['private LAN address'] },
    { file: 'packaging/windows/start.cmd', allow: ['private LAN address'] },
    { file: 'packaging/windows/start.ps1', allow: ['private LAN address'] },
    { file: 'default/config.yaml', allow: ['private LAN address'] },
    { file: 'README.md', allow: ['private LAN address'] },
    { file: 'README.en.md', allow: ['private LAN address'] },
    // The default proxy-bypass list is RFC1918 ranges by design; these are the
    // private ranges themselves, not this network's address.
    { file: 'crates/tt-domain/src/models/settings.rs', allow: ['private LAN address'] },
    { file: 'src/scripts/app/setting/settings-app/SettingsApp.js', allow: ['private LAN address'] },
    { file: 'tests/rusttavern-settings-patch.test.mjs', allow: ['private LAN address'] },
];

// Binary/minified assets are not scanned line by line: tokenizer vocabularies
// and bundled libraries contain arbitrary digit strings that trip the IPv4
// pattern without being addresses.
const SCAN_SKIP_EXTENSIONS = new Set([
    '.png', '.jpg', '.jpeg', '.gif', '.webp', '.ico', '.svg',
    '.woff', '.woff2', '.ttf', '.otf', '.eot',
    '.exe', '.dll', '.so', '.dylib', '.bin', '.wasm',
    '.mp3', '.ogg', '.wav', '.webm', '.mp4',
    '.zip', '.gz', '.7z', '.tar',
]);

function isScannable(relativePath) {
    const ext = path.extname(relativePath).toLowerCase();
    if (SCAN_SKIP_EXTENSIONS.has(ext)) return false;
    // Vendored/minified bundles and tokenizer data: huge, machine-generated,
    // and full of incidental digit runs.
    if (/\.min\.(js|mjs|css)$/i.test(relativePath)) return false;
    if (relativePath.startsWith('src/lib/')) return false;
    if (relativePath.startsWith('src/dist/')) return false;
    if (/\/dist\//.test(relativePath)) return false;
    if (relativePath.startsWith('resources/tokenizers/')) return false;
    if (relativePath.startsWith('src/scripts/extensions/tts/lib/')) return false;
    return true;
}

function parseArgs(argv) {
    const options = { outDir: 'release', binary: null, includeBinary: true, name: null };
    for (let index = 0; index < argv.length; index += 1) {
        const value = argv[index];
        if (value === '--out-dir') {
            options.outDir = argv[index + 1];
            if (!options.outDir) throw new Error('Missing value for --out-dir');
            index += 1;
        } else if (value === '--binary') {
            options.binary = argv[index + 1];
            if (!options.binary) throw new Error('Missing value for --binary');
            index += 1;
        } else if (value === '--no-binary') {
            options.includeBinary = false;
        } else if (value === '--name') {
            options.name = argv[index + 1];
            if (!options.name) throw new Error('Missing value for --name');
            index += 1;
        } else if (value === '--help' || value === '-h') {
            console.log(`Usage: node scripts/pack-dist.mjs [options]

Options:
  --out-dir <path>   Output directory (default: release)
  --binary <path>    Server binary to include (default: the release build)
  --no-binary        Source-only archive
  --name <base>      Archive base name (default: ${PRODUCT}-dist-<version>)
  --help             Show this message
`);
            process.exit(0);
        } else {
            throw new Error(`Unknown option: ${value}`);
        }
    }
    return options;
}

function run(command, args, cwd = repoRoot) {
    const result = spawnSync(command, args, { cwd, stdio: 'inherit', shell: false });
    if (result.error) throw result.error;
    if (result.status !== 0) {
        throw new Error(`${command} exited with ${result.status}`);
    }
}

function capture(command, args, cwd = repoRoot) {
    const result = spawnSync(command, args, { cwd, encoding: 'utf8', shell: false, maxBuffer: 64 * 1024 * 1024 });
    if (result.error) throw result.error;
    if (result.status !== 0) {
        throw new Error(`${command} exited with ${result.status}: ${result.stderr?.trim() ?? ''}`);
    }
    return result.stdout;
}

function resolveSevenZip() {
    const candidates = [
        'C:/Program Files/7-Zip/7z.exe',
        'C:/Program Files (x86)/7-Zip/7z.exe',
    ];
    for (const candidate of candidates) {
        if (existsSync(candidate)) return candidate;
    }
    // Fall back to PATH (chocolatey/scoop shims, or 7z on Linux/macOS).
    for (const name of ['7z', '7za']) {
        const probe = spawnSync(name, ['i'], { encoding: 'utf8', shell: true });
        if (!probe.error && probe.status === 0) return name;
    }
    throw new Error('7-Zip CLI not found. Install 7-Zip, or put 7z on PATH.');
}

function trackedFiles() {
    return capture('git', ['ls-files', '-z'])
        .split('\0')
        .filter(Boolean)
        .filter((file) => !EXCLUDED_TRACKED.includes(file))
        .filter((file) => !EXCLUDED_TRACKED_PREFIXES.some((prefix) => file.startsWith(prefix)));
}

function copyInto(stagingRoot, relativePath) {
    const source = path.join(repoRoot, relativePath);
    if (!existsSync(source)) return false;
    const target = path.join(stagingRoot, relativePath);
    mkdirSync(path.dirname(target), { recursive: true });
    copyFileSync(source, target);
    return true;
}

function walkFiles(root, prefix = '') {
    const out = [];
    for (const entry of readdirSync(path.join(root, prefix), { withFileTypes: true })) {
        const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
        if (entry.isDirectory()) {
            out.push(...walkFiles(root, relativePath));
        } else {
            out.push(relativePath);
        }
    }
    return out;
}

// Refuse to ship anything carrying this machine's or this developer's details.
// A waiver only silences the named pattern for the named file.
function scanForLeaks(stagingRoot) {
    const findings = [];
    for (const relativePath of walkFiles(stagingRoot)) {
        if (!isScannable(relativePath)) continue;
        const absolute = path.join(stagingRoot, relativePath);
        if (statSync(absolute).size > 8 * 1024 * 1024) continue;
        let text;
        try {
            text = readFileSync(absolute, 'utf8');
        } catch {
            continue;
        }
        const waiver = SCAN_WAIVERS.find((entry) => entry.file === relativePath);
        for (const pattern of FORBIDDEN_PATTERNS) {
            if (waiver?.allow.includes(pattern.name)) continue;
            const match = text.match(pattern.regex);
            if (!match) continue;
            const line = text.slice(0, match.index).split('\n').length;
            findings.push(`${relativePath}:${line}  ${pattern.name}: ${match[0]}`);
        }
    }
    return findings;
}

function distConfigYaml() {
    return [
        '# RustTavern server configuration.',
        '#',
        '# Edit and restart (start.cmd restart) for changes to take effect.',
        '# Command-line flags override these values.',
        '',
        'listen:',
        '  # 127.0.0.1 is reachable from this machine only.',
        '  #',
        '  # To reach the server from other devices on the LAN, set 0.0.0.0 AND',
        '  # give it at least one access control below (authMode: basic, or a',
        '  # whitelist). Without either, startup is refused rather than exposing',
        '  # the data directory to the whole network. A LAN bind also needs the',
        '  # port allowed through Windows Firewall.',
        '  host: 127.0.0.1',
        '  port: 8000',
        '',
        '# Data directory. Defaults to the data folder next to the executable.',
        '# dataRoot: D:/RustTavern-data',
        '',
        '# Open the default browser after startup.',
        'autoOpenBrowser: true',
        '',
        'security:',
        '  # none | basic. With basic, every request authenticates (a browser is',
        '  # prompted once, then keeps an HttpOnly session cookie).',
        '  authMode: none',
        '  # username: admin',
        '  # password: change-me',
        '  #',
        '  # Peers allowed to connect at all: a single IP, a CIDR range, or a',
        '  # trailing wildcard. Empty means allow all (subject to authMode).',
        '  #',
        '  # For a home LAN this is usually enough on its own: it keeps the server',
        '  # unreachable from outside your subnet without a password prompt on',
        '  # every device. Check your own subnet with `ipconfig`.',
        '  whitelist: []',
        '  # whitelist:',
        '  #   - 192.168.1.*',
        '  #   - 127.0.0.1',
        '',
    ].join('\r\n');
}

function distReadme(version, hasBinary) {
    const lines = [
        `# RustTavern ${version} — 分发包`,
        '',
        'SillyTavern 1.18.0 的 Rust 重写：一个本地 HTTP 服务器，浏览器就是客户端。',
        '',
        '## 快速开始',
        '',
    ];
    if (hasBinary) {
        lines.push(
            '```',
            'start.cmd start      启动并等待健康检查，成功后打印访问地址',
            'start.cmd status     进程 / 内存 / 端口 / 访问控制 / 健康 / 数据目录',
            'start.cmd stop       按 PID 停止并确认端口释放',
            'start.cmd restart    stop + start',
            'start.cmd logs 60    查看日志末尾 60 行',
            'start.cmd url        打印本机（和可达时的局域网）地址',
            '```',
            '',
            '默认监听 `127.0.0.1:8000`，浏览器打开 http://127.0.0.1:8000 即可。',
            '数据放在 `data\\`，配置是 `config.yaml`，整个文件夹可以直接搬走。',
            '',
            '## 局域网访问',
            '',
            '把 `config.yaml` 的 `listen.host` 改成 `0.0.0.0`，并**至少配一种访问控制**，',
            '否则服务器会拒绝启动（这是有意的：无认证的公开绑定等于把数据目录敞开）：',
            '',
            '```yaml',
            'listen:',
            '  host: 0.0.0.0',
            'security:',
            '  authMode: none',
            '  whitelist:',
            '    - 192.168.1.*      # 换成你自己的网段，ipconfig 可以看到',
            '    - 127.0.0.1        # 保留本机访问',
            '```',
            '',
            '或者用密码：`authMode: basic` + `username`/`password`（浏览器只弹一次，',
            '之后走 HttpOnly session cookie）。两种可以叠加：白名单先按地址拦，通过的再认证。',
            '',
            '白名单条目支持单个 IP、CIDR（`192.168.1.0/24`）和尾部通配（`192.168.1.*`）。',
            '`0.0.0.0/0`、`*` 这种等于放开全部的写法不算访问控制，仍会拒绝启动。',
            '',
            '局域网绑定还需要在 Windows 防火墙放通该端口，否则别的设备连不上。',
            '',
            '注意：传输是明文 HTTP。放到公网请前置 HTTPS 反向代理。',
            '',
            '## 包里有什么',
            '',
            '| 内容 | 说明 |',
            '|---|---|',
            '| `tauritavern.exe` | 服务器（release 构建，无需额外运行时） |',
            '| `start.cmd` | 服务控制脚本 |',
            '| `config.yaml` | 配置，首次可直接用 |',
            '| `src/` | 前端（含预打包 bundle），服务器直接伺服 |',
            '| `default/` | 随包默认内容（角色、预设、主题等） |',
            '| `` | Rust 源码（workspace crates） |',
            '| `docs/` | 架构与 API 文档 |',
            '',
            '## 自己从源码构建',
            '',
            '```',
            'pnpm install',
            'pnpm run web:build',
            'cargo build --release --manifest-path Cargo.toml -p rusttavern',
            '```',
            '',
            '构建产物在 `target/release/rusttavern.exe`，放回本目录即可替换。',
        );
    } else {
        lines.push(
            '本包只含源码，不含二进制。构建：',
            '',
            '```',
            'pnpm install',
            'pnpm run web:build',
            'cargo build --release --manifest-path Cargo.toml -p rusttavern',
            '```',
        );
    }
    lines.push('', '## 许可', '', '见 `LICENSE`。上游 SillyTavern 版权归其作者所有。', '');
    return lines.join('\r\n');
}

function main() {
    const options = parseArgs(process.argv.slice(2));
    const sevenZip = resolveSevenZip();
    const version = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')).version;

    let binaryPath = null;
    if (options.includeBinary) {
        binaryPath = path.resolve(repoRoot, options.binary
            ?? path.join('target', 'release',
                process.platform === 'win32' ? 'tauritavern.exe' : 'rusttavern'));
        if (!existsSync(binaryPath) || !statSync(binaryPath).isFile()) {
            throw new Error(`Server binary not found: ${binaryPath}\nBuild it first, or pass --no-binary.`);
        }
    }

    const missingBundles = REQUIRED_WEB_BUNDLES.filter((rel) => !existsSync(path.join(repoRoot, rel)));
    if (missingBundles.length > 0) {
        throw new Error(`Frontend bundles missing (run 'pnpm run web:build'):\n  ${missingBundles.join('\n  ')}`);
    }

    const baseName = options.name ?? `${PRODUCT}-dist-${version}`;
    const outputDirectory = path.resolve(repoRoot, options.outDir);
    mkdirSync(outputDirectory, { recursive: true });
    const stagingRoot = path.join(outputDirectory, baseName);
    const archivePath = path.join(outputDirectory, `${baseName}.7z`);

    if (existsSync(stagingRoot)) rmSync(stagingRoot, { recursive: true, force: true });
    if (existsSync(archivePath)) rmSync(archivePath, { force: true });
    mkdirSync(stagingRoot, { recursive: true });

    console.log('[pack-dist] staging tracked source...');
    const tracked = trackedFiles();
    let staged = 0;
    for (const relativePath of tracked) {
        if (copyInto(stagingRoot, relativePath)) staged += 1;
    }
    console.log(`[pack-dist] ${staged} tracked files (${tracked.length - staged} missing on disk, skipped)`);

    console.log('[pack-dist] staging frontend bundles...');
    for (const relativePath of REQUIRED_WEB_BUNDLES) {
        copyInto(stagingRoot, relativePath);
    }
    // The bundle directories hold more than the entry files listed above
    // (chunks, source maps, css); copy each one wholesale.
    for (const directory of new Set(REQUIRED_WEB_BUNDLES.map((rel) => path.posix.dirname(rel)))) {
        const sourceDirectory = path.join(repoRoot, directory);
        if (!existsSync(sourceDirectory)) continue;
        for (const relativePath of walkFiles(repoRoot, directory)) {
            copyInto(stagingRoot, relativePath);
        }
    }

    // Runtime-facing files: the service script at the archive root (its source
    // copy under packaging/windows/ comes along with the tracked tree), a fresh
    // config template, and a recipient-oriented README.
    //
    // start.cmd is normalized to CRLF, matching the .gitattributes rule for
    // *.cmd: cmd.exe is unreliable with LF-only batch files, particularly around
    // `goto` targets, and the working copy here stores LF.
    const startCmd = readFileSync(path.join(repoRoot, 'packaging/windows/start.cmd'), 'utf8');
    writeFileSync(path.join(stagingRoot, 'start.cmd'), startCmd.replace(/\r?\n/g, '\r\n'));
    writeFileSync(path.join(stagingRoot, 'config.yaml'), distConfigYaml());
    writeFileSync(path.join(stagingRoot, 'README-DIST.md'), distReadme(version, options.includeBinary));

    if (binaryPath) {
        console.log('[pack-dist] staging server binary...');
        copyFileSync(binaryPath, path.join(stagingRoot, path.basename(binaryPath)));
    }

    console.log('[pack-dist] scanning for personal data...');
    const findings = scanForLeaks(stagingRoot);
    if (findings.length > 0) {
        rmSync(stagingRoot, { recursive: true, force: true });
        throw new Error(`Refusing to pack: staged tree contains machine-specific or personal data:\n  ${findings.join('\n  ')}`);
    }
    console.log('[pack-dist] scan clean');

    console.log(`[pack-dist] packing ${archivePath} ...`);
    // -mx=7 keeps the compression cost reasonable on a slow machine while still
    // beating zip substantially on a tree this repetitive.
    run(sevenZip, ['a', '-t7z', '-mx=7', '-mmt=on', archivePath, path.basename(stagingRoot)], outputDirectory);

    const archiveMb = (statSync(archivePath).size / 1024 / 1024).toFixed(1);
    const stagedFiles = walkFiles(stagingRoot).length;
    console.log(`[pack-dist] done: ${archivePath} (${archiveMb} MB, ${stagedFiles} files)`);
    console.log(`[pack-dist] staging kept for inspection: ${stagingRoot}`);
}

try {
    main();
} catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
}
