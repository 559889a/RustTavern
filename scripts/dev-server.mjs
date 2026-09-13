#!/usr/bin/env node

// Development mode: run the frontend bundle watcher and the Rust server in
// parallel. The server serves the repository `src/` directly (static web
// root), so the watcher only rebuilds the rspack sub-bundles (settings apps,
// agent system) that the page loads from disk.

import { spawn } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');

const children = [];
let shuttingDown = false;

function spawnChild(command, args, name) {
    const child = spawn(command, args, {
        cwd: repoRoot,
        stdio: 'inherit',
        shell: process.platform === 'win32',
        env: process.env,
    });
    children.push(child);
    child.on('exit', (code, signal) => {
        if (shuttingDown) {
            return;
        }
        console.error(`[dev-server] ${name} exited (code=${code} signal=${signal}); stopping.`);
        shutdown();
    });
    return child;
}

function shutdown() {
    if (shuttingDown) {
        return;
    }
    shuttingDown = true;
    for (const child of children) {
        child.kill();
    }
    process.exit(0);
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

spawnChild('pnpm', ['run', 'web:dev'], 'web:dev');
spawnChild('cargo', [
    'run',
    '--manifest-path', 'Cargo.toml',
    '-p', 'rusttavern',
    '--',
    ...process.argv.slice(2),
], 'server');
