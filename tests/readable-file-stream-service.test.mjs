import assert from 'node:assert/strict';
import test from 'node:test';

import { createReadableFileStreamService } from '../src/host/main/services/files/readable-file-stream-service.js';

function createFsReadResponse(payload) {
    const trailer = new Uint8Array(8);
    let length = payload.byteLength;
    for (let index = trailer.length - 1; index >= 0; index -= 1) {
        trailer[index] = length & 0xff;
        length = Math.floor(length / 0x100);
    }

    const response = new Uint8Array(payload.byteLength + trailer.byteLength);
    response.set(payload, 0);
    response.set(trailer, payload.byteLength);
    return response;
}

async function readStreamBytes(stream) {
    const reader = stream.getReader();
    const chunks = [];

    while (true) {
        const { done, value } = await reader.read();
        if (done) {
            break;
        }
        chunks.push(...value);
    }

    return chunks;
}

test('readable file stream service reads Tauri fs chunks and closes the resource', async () => {
    const calls = [];
    const readResponses = [
        createFsReadResponse(Uint8Array.from([0x50, 0x4b])),
        createFsReadResponse(Uint8Array.from([0x03, 0x04])),
        createFsReadResponse(new Uint8Array(0)),
    ];
    const service = createReadableFileStreamService({
        invoke: async (command, args) => {
            calls.push({ command, args });
            if (command === 'plugin:fs|open') {
                return 7;
            }
            if (command === 'plugin:fs|read') {
                return readResponses.shift();
            }
            if (command === 'plugin:resources|close') {
                return null;
            }
            throw new Error(`Unexpected command: ${command}`);
        },
    });

    const bytes = await readStreamBytes(service.createReadableFileStream('/tmp/archive.zip'));

    assert.deepEqual(bytes, [0x50, 0x4b, 0x03, 0x04]);
    assert.deepEqual(calls, [
        {
            command: 'plugin:fs|open',
            args: {
                path: '/tmp/archive.zip',
                options: { read: true },
            },
        },
        {
            command: 'plugin:fs|read',
            args: {
                rid: 7,
                len: 512 * 1024,
            },
        },
        {
            command: 'plugin:fs|read',
            args: {
                rid: 7,
                len: 512 * 1024,
            },
        },
        {
            command: 'plugin:fs|read',
            args: {
                rid: 7,
                len: 512 * 1024,
            },
        },
        {
            command: 'plugin:resources|close',
            args: {
                rid: 7,
            },
        },
    ]);
});

test('readable file stream service reads server-mode JSON array chunks', async () => {
    const calls = [];
    const readResponses = [
        // Server mode: `plugin:fs|read` returns a JSON number array with the
        // same 8-byte big-endian trailer (bytes_read as u64).
        Array.from(createFsReadResponse(Uint8Array.from([0x50, 0x4b]))),
        Array.from(createFsReadResponse(new Uint8Array(0))),
    ];
    const service = createReadableFileStreamService({
        invoke: async (command, args) => {
            calls.push({ command, args });
            if (command === 'plugin:fs|open') {
                return 11;
            }
            if (command === 'plugin:fs|read') {
                return readResponses.shift();
            }
            if (command === 'plugin:resources|close') {
                return null;
            }
            throw new Error(`Unexpected command: ${command}`);
        },
    });

    const bytes = await readStreamBytes(service.createReadableFileStream('/data/chat.jsonl'));

    assert.deepEqual(bytes, [0x50, 0x4b]);
    assert.deepEqual(calls.map(call => call.command), [
        'plugin:fs|open',
        'plugin:fs|read',
        'plugin:fs|read',
        'plugin:resources|close',
    ]);
});

test('readable file stream service fails on unexpected fs read payloads', async () => {
    const calls = [];
    const service = createReadableFileStreamService({
        invoke: async (command, args) => {
            calls.push({ command, args });
            if (command === 'plugin:fs|open') {
                return 9;
            }
            if (command === 'plugin:fs|read') {
                return { bytes: [] };
            }
            if (command === 'plugin:resources|close') {
                return null;
            }
            throw new Error(`Unexpected command: ${command}`);
        },
    });

    const reader = service.createReadableFileStream('/tmp/archive.zip').getReader();

    await assert.rejects(() => reader.read(), /Unexpected fs read response/);
    assert.deepEqual(calls.at(-1), {
        command: 'plugin:resources|close',
        args: {
            rid: 9,
        },
    });
});
