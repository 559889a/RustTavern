import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// Server-mode bridge: transports through HTTP fetch. The window only needs the
// __TAURI_RUNNING__ flag (set by init.js in the real page).
global.window = {
    __TAURI_RUNNING__: true,
};

// Mock fetch: capture invocations, let each test decide the response.
let fetchCalls = [];
let fetchResponder = null;

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

function jsonResponse(status, payload) {
    return {
        ok: status >= 200 && status < 300,
        status,
        json: async () => payload,
    };
}

// Import the bridge under test
const tauriBridgePath = path.join(REPO_ROOT, 'src/host-bridge.js');
const {
    getChatBackupStorageStats,
    updateRustTavernSettings,
    openDialog,
    setDataRoot,
} = await import(pathToFileURL(tauriBridgePath).href);

test('getChatBackupStorageStats posts to the lightweight stats command', async () => {
    fetchCalls = [];
    const result = await getChatBackupStorageStats();

    assert.deepEqual(result, { success: true });
    assert.equal(fetchCalls.length, 1);
    assert.equal(fetchCalls[0].url, '/__tt/invoke/get_chat_backup_storage_stats');
    assert.equal(fetchCalls[0].options.method, 'POST');
    assert.equal(fetchCalls[0].options.body, '{}');
    // CSRF guard: every invoke carries the custom header the server requires.
    assert.equal(fetchCalls[0].options.headers['x-tt-invoke'], '1');
});

test('updateRustTavernSettings contract validation', async (t) => {
    await t.test('accepts valid plain object and posts snake_case args', async () => {
        fetchCalls = [];
        const dto = { theme: 'dark' };
        const result = await updateRustTavernSettings(dto);
        assert.deepEqual(result, { success: true });
        assert.equal(fetchCalls[0].url, '/__tt/invoke/update_rusttavern_settings');
        assert.deepEqual(JSON.parse(fetchCalls[0].options.body), { dto });
    });

    await t.test('rejects null', async () => {
        await assert.rejects(
            updateRustTavernSettings(null),
            /Invalid RustTavern settings DTO/
        );
    });

    await t.test('rejects arrays', async () => {
        await assert.rejects(
            updateRustTavernSettings([]),
            /Invalid RustTavern settings DTO/
        );
    });

    await t.test('rejects primitives (string)', async () => {
        await assert.rejects(
            updateRustTavernSettings('settings_string'),
            /Invalid RustTavern settings DTO/
        );
    });

    await t.test('rejects primitives (number)', async () => {
        await assert.rejects(
            updateRustTavernSettings(42),
            /Invalid RustTavern settings DTO/
        );
    });

    await t.test('rejects primitives (boolean)', async () => {
        await assert.rejects(
            updateRustTavernSettings(true),
            /Invalid RustTavern settings DTO/
        );
    });

    await t.test('rejects class instances', async () => {
        class Settings {}
        await assert.rejects(
            updateRustTavernSettings(new Settings()),
            /Invalid RustTavern settings DTO/
        );
    });
});

test('openDialog contract validation', async (t) => {
    await t.test('returns null in server mode without invoking the backend', async () => {
        fetchCalls = [];
        const options = { title: 'Choose File' };
        const result = await openDialog(options);
        assert.equal(result, null);
        assert.equal(fetchCalls.length, 0);
    });

    await t.test('normalizes omitted options to empty object', async () => {
        fetchCalls = [];
        const result = await openDialog();
        assert.equal(result, null);
    });

    await t.test('normalizes undefined to empty object', async () => {
        fetchCalls = [];
        const result = await openDialog(undefined);
        assert.equal(result, null);
    });

    await t.test('rejects null', async () => {
        await assert.rejects(
            openDialog(null),
            /Invalid dialog options: expected an object/
        );
    });

    await t.test('rejects arrays', async () => {
        await assert.rejects(
            openDialog([]),
            /Invalid dialog options: expected an object/
        );
    });

    await t.test('rejects primitives (string)', async () => {
        await assert.rejects(
            openDialog('some_option'),
            /Invalid dialog options: expected an object/
        );
    });

    await t.test('rejects primitives (number)', async () => {
        await assert.rejects(
            openDialog(123),
            /Invalid dialog options: expected an object/
        );
    });

    await t.test('rejects primitives (boolean)', async () => {
        await assert.rejects(
            openDialog(false),
            /Invalid dialog options: expected an object/
        );
    });
});

test('setDataRoot contract validation', async (t) => {
    await t.test('accepts valid non-empty path string', async () => {
        fetchCalls = [];
        await setDataRoot('/valid/path');
        assert.equal(fetchCalls.length, 1);
        assert.equal(fetchCalls[0].url, '/__tt/invoke/set_data_root');
        // The alias pass keeps both spellings (legacy camelCase behavior).
        const body = JSON.parse(fetchCalls[0].options.body);
        assert.equal(body.data_root, '/valid/path');
        assert.equal(body.dataRoot, '/valid/path');
    });

    await t.test('rejects empty string', async () => {
        await assert.rejects(
            setDataRoot(''),
            /Invalid data root path/
        );
    });

    await t.test('rejects whitespace-only string', async () => {
        await assert.rejects(
            setDataRoot('   '),
            /Invalid data root path/
        );
    });

    await t.test('rejects non-string values', async () => {
        await assert.rejects(
            setDataRoot(42),
            /Invalid data root path/
        );
        await assert.rejects(
            setDataRoot(null),
            /Invalid data root path/
        );
    });
});

test('invoke maps backend CommandError payloads to the legacy error text contract', async (t) => {
    await t.test('BadRequest', async () => {
        fetchCalls = [];
        fetchResponder = () => jsonResponse(400, { BadRequest: 'nope' });
        await assert.rejects(
            getChatBackupStorageStats(),
            /Bad request: nope/,
        );
        fetchResponder = null;
    });

    await t.test('Conflict', async () => {
        fetchCalls = [];
        fetchResponder = () => jsonResponse(409, { Conflict: 'exists' });
        await assert.rejects(
            getChatBackupStorageStats(),
            /Conflict: exists/,
        );
        fetchResponder = null;
    });

    await t.test('UpstreamFailure carries structured details', async () => {
        fetchCalls = [];
        const details = { status: 502, body: 'upstream boom' };
        fetchResponder = () => jsonResponse(502, { UpstreamFailure: details });
        await assert.rejects(
            getChatBackupStorageStats(),
            (error) => error.message === 'Upstream request failed' && error.details === details,
        );
        fetchResponder = null;
    });
});
