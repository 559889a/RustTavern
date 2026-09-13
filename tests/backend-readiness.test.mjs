import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const READINESS_PATH = path.join(REPO_ROOT, 'src/host/main/bootstrap/backend-readiness.js');

async function importFreshReadiness() {
    const url = `${pathToFileURL(READINESS_PATH).href}?t=${Date.now()}-${Math.random()}`;
    return import(url);
}

let fetchCalls = [];
let fetchResponder = null;

function installFetch() {
    fetchCalls = [];
    global.fetch = async (url, options = {}) => {
        fetchCalls.push({ url, options });
        if (typeof fetchResponder === 'function') {
            return fetchResponder(url, options);
        }
        return {
            ok: true,
            status: 200,
            json: async () => ({ success: true }),
        };
    };
}

function installFakeWindow() {
    global.window = {
        __TAURI_RUNNING__: true,
    };
}

function cleanupGlobals() {
    delete global.window;
    delete global.fetch;
    fetchResponder = null;
}

test('backend readiness follows default content initialization', async () => {
    const source = await readFile(
        path.join(REPO_ROOT, 'crates/rusttavern/src/app.rs'),
        'utf8',
    );
    const contentIndex = source.indexOf('.initialize_default_content("default-user")');
    const slotIndex = source.indexOf('app_state_slot.set(Ok(state.clone()))');
    const readyIndex = source.indexOf('backend_readiness.mark_ready()');

    assert.ok(contentIndex >= 0 && contentIndex < slotIndex && slotIndex < readyIndex);
});

test('backend readiness waits on the explicit readiness command', async () => {
    installFakeWindow();
    installFetch();

    try {
        const { waitForBackendReady } = await importFreshReadiness();
        await waitForBackendReady();

        assert.equal(fetchCalls.length, 1);
        assert.equal(fetchCalls[0].url, '/__tt/invoke/wait_for_backend_ready');
        assert.equal(fetchCalls[0].options.method, 'POST');
    } finally {
        cleanupGlobals();
    }
});

test('backend readiness fails fast when the transport is unavailable', async () => {
    global.window = {};
    global.fetch = async () => {
        throw new Error('network down');
    };

    try {
        const { waitForBackendReady } = await importFreshReadiness();
        await assert.rejects(waitForBackendReady(), /network down/);
    } finally {
        cleanupGlobals();
    }
});

test('backend readiness propagates backend startup failure', async () => {
    installFakeWindow();
    installFetch();
    fetchResponder = () => ({
        ok: false,
        status: 500,
        json: async () => ({ InternalServerError: 'backend failed' }),
    });

    try {
        const { waitForBackendReady } = await importFreshReadiness();
        await assert.rejects(waitForBackendReady(), /Internal server error: backend failed/);
    } finally {
        cleanupGlobals();
    }
});
