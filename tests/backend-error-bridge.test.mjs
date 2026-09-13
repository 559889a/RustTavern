import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const BRIDGE_PATH = path.join(REPO_ROOT, 'src/host/main/bootstrap/backend-error-bridge.js');

async function importFreshBridge() {
    const url = `${pathToFileURL(BRIDGE_PATH).href}?t=${Date.now()}-${Math.random()}`;
    return import(url);
}

// The bridge transports through host-bridge (HTTP fetch + EventSource).
let fetchCalls = [];
let fetchResponder = null;

class FakeEventSource {
    static instances = [];

    constructor(url) {
        this.url = url;
        this.listeners = new Map();
        FakeEventSource.instances.push(this);
    }

    addEventListener(type, listener) {
        const set = this.listeners.get(type) ?? new Set();
        set.add(listener);
        this.listeners.set(type, set);
    }

    removeEventListener(type, listener) {
        this.listeners.get(type)?.delete(listener);
    }

    dispatch(type, data) {
        for (const listener of this.listeners.get(type) ?? []) {
            listener({ type, data });
        }
    }
}

function installRuntime({ invoke, consumerReady = false }) {
    const dispatched = [];
    const calls = [];

    // host-bridge caches its EventSource per module instance, so listeners
    // accumulate across tests in this file; clear them before each test.
    FakeEventSource.instances[0]?.listeners?.clear();

    global.CustomEvent = class CustomEvent {
        constructor(type, options = {}) {
            this.type = type;
            this.detail = options.detail;
        }
    };

    global.window = {
        __TAURI_RUNNING__: true,
        __RUSTTAVERN_BACKEND_ERROR_CONSUMER_READY__: consumerReady,
        dispatchEvent(event) {
            dispatched.push(event);
        },
    };

    global.fetch = async (url, options = {}) => {
        calls.push(['invoke', url]);
        return invoke(url, options);
    };

    return {
        calls,
        dispatched,
        dispatchBackendError(data) {
            // host-bridge caches its EventSource per module instance, so the
            // whole test file shares the first created source.
            const source = FakeEventSource.instances[0];
            assert.ok(source, 'an EventSource subscription must exist');
            source.dispatch('rusttavern-backend-error', data);
        },
        window: global.window,
    };
}

function cleanupGlobals() {
    delete global.window;
    delete global.fetch;
    delete global.CustomEvent;
    delete global.EventSource;
    fetchResponder = null;
}

function installEventSource() {
    global.EventSource = FakeEventSource;
}

function okResponse(payload) {
    return {
        ok: true,
        status: 200,
        json: async () => payload,
    };
}

test('backend error bridge registers listener before draining pending errors', async () => {
    installEventSource();
    const fake = installRuntime({
        invoke(url) {
            assert.equal(url, '/__tt/invoke/backend_error_bridge_ready');
            return okResponse([' first ', { message: 'second' }, '', { message: '   ' }]);
        },
    });

    try {
        const { installBackendErrorBridge } = await importFreshBridge();
        await installBackendErrorBridge();

        assert.equal(FakeEventSource.instances.length, 1);
        assert.equal(FakeEventSource.instances[0].url, '/__tt/events');
        assert.deepEqual(fake.window.__RUSTTAVERN_BACKEND_ERROR_QUEUE__, [
            { message: 'first' },
            { message: 'second' },
        ]);
    } finally {
        cleanupGlobals();
    }
});

test('backend error bridge forwards runtime events after consumer is ready', async () => {
    installEventSource();
    const fake = installRuntime({
        consumerReady: true,
        invoke() {
            return okResponse([]);
        },
    });

    try {
        const { installBackendErrorBridge } = await importFreshBridge();
        await installBackendErrorBridge();

        fake.dispatchBackendError(' later ');

        assert.equal(fake.dispatched.length, 1);
        assert.equal(fake.dispatched[0].type, 'tauritavern:backend-error');
        assert.deepEqual(fake.dispatched[0].detail, { message: 'later' });
    } finally {
        cleanupGlobals();
    }
});

test('backend error bridge fails fast when ready command fails', async () => {
    installEventSource();
    const fake = installRuntime({
        invoke() {
            return {
                ok: false,
                status: 500,
                json: async () => ({ InternalServerError: 'ready failed' }),
            };
        },
    });

    try {
        const { installBackendErrorBridge } = await importFreshBridge();
        await assert.rejects(installBackendErrorBridge(), /Internal server error: ready failed/);
        assert.equal(FakeEventSource.instances.length, 1);
    } finally {
        cleanupGlobals();
    }
});
