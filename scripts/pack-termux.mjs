#!/usr/bin/env node

// Assemble a minimal Termux runtime package: the aarch64 binary, the frontend
// (src/), packaged default content (default/), config.yaml and start.sh.
// No sources, no workspace files, no build tooling.
//
// Usage:
//   node scripts/pack-termux.mjs [--binary <path>] [--out <dir>]

import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync, copyFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');

const DEFAULT_BINARY = path.join(repoRoot, 'target', 'aarch64-linux-android', 'release', 'rusttavern');
const PACKAGE_NAME = 'rusttavern-termux-arm64';

// The frontend boots from these; a package without them serves a page that
// cannot start (lib.js imports ./dist/lib.core.bundle.js).
const REQUIRED_WEB_BUNDLES = [
    'src/dist/lib.core.bundle.js',
    'src/dist/lib.optional.bundle.js',
    'src/scripts/extensions/agent-system/dist/index.bundle.js',
    'src/scripts/app/setting/dist/settings.bundle.js',
    'src/scripts/app/setting/dist/sync.bundle.js',
    'src/scripts/app/setting/dist/dev-logs.bundle.js',
];

// Everything the server reads at runtime lives under src/ and default/. These
// subtrees are development-only and would double the transfer for nothing.
const EXCLUDED_SRC_ENTRIES = new Set(['.gitignore']);

function parseArgs(argv) {
    const options = { binary: DEFAULT_BINARY, out: path.join(repoRoot, 'release') };
    for (let index = 0; index < argv.length; index += 1) {
        const flag = argv[index];
        if (flag === '--binary') {
            options.binary = argv[index + 1];
            index += 1;
        } else if (flag === '--out') {
            options.out = argv[index + 1];
            index += 1;
        } else {
            throw new Error(`Unknown option: ${flag}`);
        }
        if (!options.binary || !options.out) {
            throw new Error(`Missing value for ${flag}`);
        }
    }
    return options;
}

function copyTree(sourceDir, targetDir, { skip = new Set() } = {}) {
    mkdirSync(targetDir, { recursive: true });
    for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
        if (skip.has(entry.name)) {
            continue;
        }
        const source = path.join(sourceDir, entry.name);
        const target = path.join(targetDir, entry.name);
        if (entry.isDirectory()) {
            copyTree(source, target, { skip });
        } else if (entry.isFile()) {
            copyFileSync(source, target);
        }
    }
}

function directorySize(dir) {
    let total = 0;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        total += entry.isDirectory() ? directorySize(full) : statSync(full).size;
    }
    return total;
}

// Termux config: bind loopback by default. A LAN bind without authentication is
// rejected by the server itself, so the template documents both knobs together.
function termuxConfigYaml() {
    return [
        '# RustTavern server configuration (Termux / Android).',
        '#',
        '# Edit and restart (./start.sh restart) for changes to take effect.',
        '# Command-line flags override these values.',
        '',
        'listen:',
        '  # 127.0.0.1 is reachable from this device only.',
        '  #',
        '  # To reach the server from other devices on the LAN, set 0.0.0.0 AND',
        '  # give it at least one access control below (authMode: basic, or a',
        '  # whitelist). Without either, startup is refused rather than exposing',
        '  # the data directory to the whole network.',
        '  host: 127.0.0.1',
        '  # SillyTavern uses 8000 by default; pick another port so both can run.',
        '  port: 8100',
        '',
        '# Data directory. Relative paths resolve against the process working',
        "# directory, so keep it absolute or leave it unset (defaults to the",
        '# binary directory + /data).',
        '# dataRoot: /data/data/com.termux/files/home/rusttavern/data',
        '',
        '# Termux has no browser to open.',
        'autoOpenBrowser: false',
        '',
        'security:',
        '  # none | basic. With basic, every request authenticates (a browser is',
        '  # prompted once, then keeps an HttpOnly session cookie).',
        '  authMode: none',
        '  # username: tavern',
        '  # password: change-me',
        '  #',
        '  # Peers allowed to connect at all. A single IP, a CIDR range, or a',
        '  # trailing wildcard. Empty means allow all (subject to authMode).',
        '  #',
        '  # For a home LAN this is usually enough on its own: it keeps the',
        '  # server unreachable from outside your subnet without a password',
        '  # prompt on every device. Check your own subnet with `ifconfig`.',
        '  whitelist: []',
        '  # whitelist:',
        '  #   - 192.168.1.*',
        '  #   - 127.0.0.1',
        '',
    ].join('\n');
}

function main() {
    const options = parseArgs(process.argv.slice(2));

    const binaryPath = path.resolve(options.binary);
    if (!existsSync(binaryPath) || !statSync(binaryPath).isFile()) {
        throw new Error(`Android binary not found: ${binaryPath}\nBuild it first: bash scripts/local-android-build.sh`);
    }

    const missingBundles = REQUIRED_WEB_BUNDLES.filter((rel) => !existsSync(path.join(repoRoot, rel)));
    if (missingBundles.length > 0) {
        throw new Error(`Missing frontend bundles (run pnpm run web:build):\n  ${missingBundles.join('\n  ')}`);
    }

    const startScript = path.join(repoRoot, 'packaging', 'termux', 'start.sh');
    if (!existsSync(startScript)) {
        throw new Error(`Missing ${startScript}`);
    }
    const importScript = path.join(repoRoot, 'packaging', 'termux', 'import-sillytavern.sh');
    if (!existsSync(importScript)) {
        throw new Error(`Missing ${importScript}`);
    }

    const outDir = path.resolve(options.out);
    const stagingRoot = path.join(outDir, PACKAGE_NAME);
    mkdirSync(outDir, { recursive: true });
    rmSync(stagingRoot, { recursive: true, force: true });
    mkdirSync(stagingRoot, { recursive: true });

    console.log('[pack-termux] staging binary + frontend + default content...');
    copyFileSync(binaryPath, path.join(stagingRoot, 'rusttavern'));
    copyTree(path.join(repoRoot, 'src'), path.join(stagingRoot, 'src'), { skip: EXCLUDED_SRC_ENTRIES });
    copyTree(path.join(repoRoot, 'default'), path.join(stagingRoot, 'default'));

    // LF endings matter: Termux bash rejects a CRLF shebang line.
    for (const [source, name] of [[startScript, 'start.sh'], [importScript, 'import-sillytavern.sh']]) {
        const body = readFileSync(source, 'utf8').replace(/\r\n/g, '\n');
        writeFileSync(path.join(stagingRoot, name), body, { mode: 0o755 });
    }
    writeFileSync(path.join(stagingRoot, 'config.yaml'), termuxConfigYaml());

    const sizeMb = (directorySize(stagingRoot) / 1024 / 1024).toFixed(1);
    console.log(`[pack-termux] staged ${stagingRoot} (${sizeMb} MB)`);

    const tarName = `${PACKAGE_NAME}.tar.gz`;
    const tarPath = path.join(outDir, tarName);
    rmSync(tarPath, { force: true });
    console.log(`[pack-termux] creating ${tarPath} ...`);
    // Run in the output directory and pass bare names: a Windows absolute path
    // contains a colon, which tar reads as a remote `host:path` spec.
    const result = spawnSync('tar', ['-czf', tarName, PACKAGE_NAME], {
        cwd: outDir,
        stdio: 'inherit',
        shell: process.platform === 'win32',
    });
    if (result.status !== 0) {
        throw new Error(`tar failed with status ${result.status}`);
    }

    const tarMb = (statSync(tarPath).size / 1024 / 1024).toFixed(1);
    console.log(`[pack-termux] done: ${tarPath} (${tarMb} MB)`);
    console.log('[pack-termux] ship it with:');
    console.log(`  cat "${tarPath}" | ssh -p <termux-ssh-port> <host> 'cd ~ && tar -xzf -'`);
}

try {
    main();
} catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
}
