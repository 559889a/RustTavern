#!/usr/bin/env node

// Build the RustTavern server release: frontend bundle -> cargo release
// binary -> distributable zip (binary + src/ + default/ + templates).
//
// Usage:
//   node scripts/build-server.mjs [--skip-web-build] [--out-dir <path>]

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const PRODUCT_NAME = 'RustTavern';
const SERVER_BINARY_NAME = process.platform === 'win32' ? 'rusttavern.exe' : 'rusttavern';

function printHelp() {
    console.log(`Usage: node scripts/build-server.mjs [options]

Options:
  --skip-web-build        Skip the frontend bundle build (web:build)
  --out-dir <path>        Output directory for the release zip (default: release)
  --help                  Show this help message
`);
}

function parseArgs(argv) {
    const options = { skipWebBuild: false, outDir: 'release' };
    for (let index = 0; index < argv.length; index += 1) {
        const value = argv[index];
        if (value === '--skip-web-build') {
            options.skipWebBuild = true;
        } else if (value === '--out-dir') {
            const outDir = argv[index + 1];
            if (!outDir) {
                throw new Error('Missing value for --out-dir');
            }
            options.outDir = outDir;
            index += 1;
        } else if (value === '--help' || value === '-h') {
            printHelp();
            process.exit(0);
        } else {
            throw new Error(`Unknown option: ${value}`);
        }
    }
    return options;
}

// `okStatuses` exists for robocopy, which reports "files were copied" as exit
// code 1 and treats 0-7 as success (only >=8 is failure). Everything else here
// is a normal tool where 0 is the only success.
function run(command, args, cwd, env = process.env, { okStatuses = [0] } = {}) {
    const result = spawnSync(command, args, { cwd, env, stdio: 'inherit', shell: process.platform === 'win32' });
    if (result.error) {
        throw result.error;
    }
    if (!okStatuses.includes(result.status ?? -1)) {
        // Throw rather than exit: the caller prints the message, so a failure
        // says which command failed instead of dying silently.
        throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status}`);
    }
}

// robocopy's success range: 0 no-op, 1-7 various "copied/extra/mismatched"
// combinations, >=8 real failures.
const ROBOCOPY_OK = [0, 1, 2, 3, 4, 5, 6, 7];

function resolveReleaseBinary() {
    const releaseDir = path.join(repoRoot, 'target', 'release');
    const binaryPath = path.join(releaseDir, SERVER_BINARY_NAME);
    if (!existsSync(binaryPath) || !statSync(binaryPath).isFile()) {
        throw new Error(`Server binary not found at ${binaryPath}. Run the cargo release build first.`);
    }
    return binaryPath;
}

// All rspack bundles are gitignored build artifacts; a fresh checkout (CI,
// flatpak sandbox) has none of them. A release zip without them serves a
// frontend that cannot boot (lib.js imports ./dist/lib.core.bundle.js).
const REQUIRED_WEB_BUNDLES = [
    'src/dist/lib.core.bundle.js',
    'src/dist/lib.optional.bundle.js',
    'src/scripts/extensions/agent-system/dist/index.bundle.js',
    'src/scripts/app/setting/dist/settings.bundle.js',
    'src/scripts/app/setting/dist/sync.bundle.js',
    'src/scripts/app/setting/dist/dev-logs.bundle.js',
];

function webBundlesMissing() {
    return REQUIRED_WEB_BUNDLES.filter((rel) => !existsSync(path.join(repoRoot, rel)));
}

function ensureWebBundles(options) {
    const missing = options.skipWebBuild ? webBundlesMissing() : [];
    if (missing.length > 0) {
        console.warn(`[build-server] --skip-web-build was requested but ${missing.length} bundle(s) are missing; running web:build instead:`);
        for (const rel of missing) {
            console.warn(`  - ${rel}`);
        }
        run('pnpm', ['run', 'web:build'], repoRoot);
        const stillMissing = webBundlesMissing();
        if (stillMissing.length > 0) {
            throw new Error(`web:build did not produce expected bundles: ${stillMissing.join(', ')}`);
        }
    }
}

// Stage the release layout in a temp directory, then zip it with the system
// archive tool (bsdtar on Windows/macOS; `zip` on Linux).
function stageReleaseLayout(binaryPath) {
    const stagingRoot = path.join(os.tmpdir(), `rusttavern-release-${process.pid}`);
    mkdirSync(path.join(stagingRoot, 'src'), { recursive: true });
    mkdirSync(path.join(stagingRoot, 'default'), { recursive: true });

    const copyTree = (sourceDir, targetDir) => {
        if (!existsSync(sourceDir)) {
            console.warn(`[build-server] Missing expected directory, skipping: ${sourceDir}`);
            return;
        }
        run(process.platform === 'win32' ? 'robocopy' : 'cp',
            process.platform === 'win32'
                ? [sourceDir, targetDir, '/E', '/NFL', '/NDL', '/NJH', '/NJS', '/NC', '/NS', '/NP']
                : ['-r', sourceDir + '/.', targetDir + '/'],
            repoRoot,
            process.env,
            process.platform === 'win32' ? { okStatuses: ROBOCOPY_OK } : {});
    };

    const copyFile = (source, target) => {
        if (!existsSync(source)) {
            console.warn(`[build-server] Missing expected file, skipping: ${source}`);
            return;
        }
        run(process.platform === 'win32' ? 'copy' : 'cp',
            process.platform === 'win32' ? ['/Y', source, target] : [source, target],
            repoRoot);
    };

    copyFile(binaryPath, path.join(stagingRoot, SERVER_BINARY_NAME));
    copyTree(path.join(repoRoot, 'src'), path.join(stagingRoot, 'src'));
    copyTree(path.join(repoRoot, 'default'), path.join(stagingRoot, 'default'));
    copyTree(path.join(repoRoot, 'src', 'scripts', 'templates'), path.join(stagingRoot, 'src', 'scripts', 'templates'));

    // The service-control scripts, platform-appropriate. The README's Quick
    // Start is written around them, so a package without them is a package the
    // documented first step cannot run.
    if (process.platform === 'win32') {
        for (const name of ['start.ps1', 'start.cmd']) {
            copyFile(path.join(repoRoot, 'packaging', 'windows', name), path.join(stagingRoot, name));
        }
    } else {
        const startScript = path.join(repoRoot, 'packaging', 'termux', 'start.sh');
        copyFile(startScript, path.join(stagingRoot, 'start.sh'));
        // The archive is extracted on Linux/macOS, where the exec bit is what
        // makes `./start.sh` runnable; `cp` alone leaves the umask in charge.
        if (process.platform !== 'win32') {
            run('chmod', ['+x', path.join(stagingRoot, 'start.sh')], repoRoot);
        }
    }

    return stagingRoot;
}

function archiveStaging(stagingRoot, zipPath) {
    // bsdtar creates a zip via -a; Linux needs the `zip` tool. On Windows the
    // `tar` on PATH is frequently GNU tar (Git Bash / MSYS), which can neither
    // write a zip nor accept a drive-letter path — it reads `C:\...` as a remote
    // `host:path` and dies with "Cannot connect to C: resolve failed". Windows
    // ships bsdtar in System32, so name it explicitly.
    const isBsdTar = process.platform === 'win32' || process.platform === 'darwin';
    let command = isBsdTar ? 'tar' : 'zip';
    if (process.platform === 'win32') {
        const systemTar = path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe');
        if (existsSync(systemTar)) {
            command = systemTar;
        }
    }

    const cwd = process.cwd();
    process.chdir(stagingRoot);
    try {
        const args = isBsdTar
            ? ['-a', '-c', '-f', zipPath, '.']
            : ['-r', zipPath, '.'];
        run(command, args, stagingRoot);
    } finally {
        process.chdir(cwd);
    }
}

function main() {
    const options = parseArgs(process.argv.slice(2));

    ensureWebBundles(options);
    if (!options.skipWebBuild) {
        console.log('[build-server] Building frontend bundles...');
        run('pnpm', ['run', 'web:build'], repoRoot);
    }

    console.log('[build-server] Building release binary (cargo build --release)...');
    run('cargo', ['build', '--release', '--manifest-path', 'Cargo.toml', '-p', 'rusttavern'], repoRoot);

    const binaryPath = resolveReleaseBinary();
    const outputDirectory = path.resolve(repoRoot, options.outDir);
    mkdirSync(outputDirectory, { recursive: true });

    const platformTag = `${process.platform}-${process.arch}`;
    const zipName = `${PRODUCT_NAME}-${platformTag}.zip`;
    const zipPath = path.join(outputDirectory, zipName);
    if (existsSync(zipPath)) {
        throw new Error(`Refusing to overwrite existing archive: ${zipPath}`);
    }

    console.log('[build-server] Staging release layout...');
    const stagingRoot = stageReleaseLayout(binaryPath);

    console.log(`[build-server] Packaging ${zipPath} ...`);
    archiveStaging(stagingRoot, zipPath);

    const sizeMb = (statSync(zipPath).size / 1024 / 1024).toFixed(1);
    console.log(`[build-server] Done: ${zipPath} (${sizeMb} MB)`);
}

try {
    main();
} catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
}
