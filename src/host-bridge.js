// Core RustTavern host bridge: HTTP/SSE transport (server mode).
//
// Phase 3: the frontend is served by the RustTavern HTTP server, so the
// transport is plain same-origin HTTP + SSE. The exported API shape is kept
// identical to the old Tauri bridge so the Host Kernel and extensions do not
// change:
//
// - `invoke(cmd, args)`        → POST /__tt/invoke/{command} (JSON args)
// - `listen(name, handler)`    → EventSource on /__tt/events, filtered by name
// - `createChannel(onmessage)` → generate stream_id + subscribe
//                                GET /__tt/stream/{stream_id} BEFORE the
//                                command is invoked (no lost first events)
// - `convertFileSrc(path)`     → /__tt/file?path=... (server file access)
// - `openDialog`               → null (browser file input fallback)
// - `openExternalUrl`          → window.open

import { SILLYTAVERN_COMPAT_VERSION } from './compat-version.js';

function detectTauriEnv() {
    if (typeof window === 'undefined') {
        return false;
    }

    // `__TAURI_RUNNING__` is set unconditionally in init.js: pages are only
    // ever served by the RustTavern host, so the flag now means "host kernel
    // should activate" rather than "Tauri WebView". The `__RUSTTAVERN__` ABI
    // check covers iframes/workers that inherit a patched window.
    return window.__TAURI_RUNNING__ === true
        || typeof window.__RUSTTAVERN__?.invoke?.safeInvoke === 'function';
}

export const isTauriEnv = detectTauriEnv();

function getHostSafeInvoke() {
    if (typeof window === 'undefined') {
        return null;
    }

    const fn = window.__RUSTTAVERN__?.invoke?.safeInvoke;
    return typeof fn === 'function' ? fn : null;
}

function isPlainObject(value) {
    if (Object.prototype.toString.call(value) !== '[object Object]') {
        return false;
    }
    const prototype = Object.getPrototypeOf(value);
    return prototype === null || prototype === Object.prototype;
}

function withTauriArgumentAliases(args) {
    if (!isPlainObject(args)) {
        return args;
    }

    const aliased = { ...args };
    for (const [key, value] of Object.entries(args)) {
        if (!key.includes('_')) {
            continue;
        }

        const camelCaseKey = key.replace(/_+([a-zA-Z0-9])/g, (_, char) => char.toUpperCase());
        if (!Object.prototype.hasOwnProperty.call(aliased, camelCaseKey)) {
            aliased[camelCaseKey] = value;
        }
    }

    return aliased;
}

// Map the serialized CommandError payload back to the legacy Tauri error text
// contract (`kernel/host-error-response.js` matches these prefixes).
function commandErrorText(payload) {
    if (payload === null || typeof payload !== 'object') {
        return String(payload || 'Command failed');
    }
    const keys = Object.keys(payload);
    if (keys.length !== 1) {
        const text = payload.message || payload.error;
        return typeof text === 'string' ? text : JSON.stringify(payload);
    }
    const variant = keys[0];
    const nested = String(payload[variant] ?? '').trim();
    switch (variant) {
        case 'BadRequest': return `Bad request: ${nested}`;
        case 'Conflict': return `Conflict: ${nested}`;
        case 'NotFound': return `Not found: ${nested}`;
        case 'Unauthorized': return `Unauthorized: ${nested}`;
        case 'Cancelled': return nested || 'Operation cancelled';
        case 'TooManyRequests': return `Too many requests: ${nested}`;
        case 'UpstreamFailure': return 'Upstream request failed';
        case 'InternalServerError': return `Internal server error: ${nested}`;
        default: return JSON.stringify(payload);
    }
}

/**
 * Invoke a backend command over HTTP.
 *
 * @param {string} command
 * @param {any} [args]
 * @returns {Promise<any>}
 */
export const invoke = async (command, args) => {
    const response = await fetch('/__tt/invoke/' + encodeURIComponent(String(command)), {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            // CSRF guard: the server rejects state-changing invocations without
            // this custom header (cross-site forms cannot attach it).
            'x-tt-invoke': '1',
        },
        body: JSON.stringify(args === undefined ? {} : withTauriArgumentAliases(args)),
        credentials: 'same-origin',
    });
    let payload = null;
    try {
        payload = await response.json();
    } catch {
        // Non-JSON error body (e.g. proxy error page).
    }
    if (!response.ok) {
        const message = commandErrorText(payload);
        const error = new Error(message);
        if (payload && typeof payload === 'object' && Object.prototype.hasOwnProperty.call(payload, 'UpstreamFailure')) {
            error.details = payload.UpstreamFailure;
        }
        throw error;
    }
    return payload;
};

async function invokeWithHostNormalization(command, args) {
    const safeInvoke = getHostSafeInvoke();
    if (safeInvoke) {
        return safeInvoke(command, args);
    }

    return args === undefined ? invoke(command) : invoke(command, args);
}

// ---------------------------------------------------------------------------
// Events (SSE)
// ---------------------------------------------------------------------------

let sharedEventSource = null;

function getSharedEventSource() {
    if (sharedEventSource) {
        return sharedEventSource;
    }
    if (typeof EventSource === 'undefined') {
        throw new Error('Tauri event listen is unavailable in this environment');
    }

    const source = new EventSource('/__tt/events');
    // EventSource reconnects automatically; keep the shared instance so all
    // listeners survive reconnects. Errors are surfaced per-listener below.
    source.onerror = () => {
        // noop: reconnect is automatic
    };
    sharedEventSource = source;
    return source;
}

/**
 * Subscribe to a backend event. Returns a promise resolving to the unlisten
 * function (same contract as the old Tauri `listen`).
 *
 * @param {string} name
 * @param {(event: { event: string; payload: any }) => void} handler
 * @returns {Promise<() => void>}
 */
export const listen = (name, handler) => {
    try {
        if (!name || typeof handler !== 'function') {
            return Promise.reject(new Error('Invalid listen arguments'));
        }

        const source = getSharedEventSource();
        const wrapped = (event) => {
            let payload = null;
            try {
                payload = JSON.parse(event.data);
            } catch {
                payload = event.data;
            }
            try {
                handler({ event: name, payload });
            } catch (error) {
                console.error(`RustTavern: listener for '${name}' failed:`, error);
            }
        };
        source.addEventListener(name, wrapped);

        return Promise.resolve(() => {
            source.removeEventListener(name, wrapped);
        });
    } catch (error) {
        return Promise.reject(error);
    }
};

// ---------------------------------------------------------------------------
// Command streams (SSE, replaces ipc::Channel)
// ---------------------------------------------------------------------------

function createStreamId() {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        return crypto.randomUUID();
    }

    const timestamp = Date.now().toString(36);
    const random = Math.random().toString(36).slice(2, 10);
    return `${timestamp}-${random}`;
}

// Manual SSE reader for a command stream. Unlike EventSource, it does not
// reconnect: a command stream ends when the server closes it. Passing an
// AbortSignal lets the caller terminate the read loop (and the underlying
// fetch) on demand.
function subscribeStream(streamId, onmessage, signal) {
    void (async () => {
        try {
            const response = await fetch('/__tt/stream/' + encodeURIComponent(streamId), {
                headers: {
                    // CSRF guard: stream subscription is a state-changing
                    // endpoint on the server; same-origin fetch sets this.
                    'x-tt-invoke': '1',
                },
                credentials: 'same-origin',
                signal,
            });
            if (!response.ok || !response.body) {
                throw new Error('Stream unavailable: ' + response.status);
            }

            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            let buffer = '';
            // Offset of the first unconsumed byte in `buffer`: frames are
            // scanned from here and the consumed prefix is trimmed once per
            // chunk instead of rebuilding the whole string per frame.
            let offset = 0;
            for (;;) {
                const { done, value } = await reader.read();
                if (done) {
                    break;
                }
                buffer += decoder.decode(value, { stream: true });
                let boundary;
                while ((boundary = buffer.indexOf('\n\n', offset)) >= 0) {
                    const raw = buffer.slice(offset, boundary);
                    offset = boundary + 2;
                    let data = '';
                    for (const line of raw.split('\n')) {
                        if (line.startsWith('data:')) {
                            data += line.slice(5).trim();
                        }
                    }
                    if (!data) {
                        continue;
                    }
                    let payload = null;
                    try {
                        payload = JSON.parse(data);
                    } catch {
                        payload = data;
                    }
                    try {
                        onmessage(payload);
                    } catch (error) {
                        console.error(`RustTavern: stream '${streamId}' handler failed:`, error);
                    }
                }
                if (offset > 0) {
                    // Keep only the unconsumed tail.
                    buffer = buffer.slice(offset);
                    offset = 0;
                }
            }
        } catch (error) {
            if (error?.name === 'AbortError') {
                return; // intentional cancellation
            }
            console.warn(`RustTavern: stream '${streamId}' disconnected:`, error);
        }
    })();
}

/**
 * Create a command stream channel. The SSE subscription is opened BEFORE the
 * channel marker is returned, so a command invoked with this channel never
 * misses its first events (the server waits briefly for subscribers).
 *
 * @param {(message: any) => void} onmessage
 * @returns {{ id: string; onmessage: (message: any) => void; cancel: () => void }}
 */
export function createChannel(onmessage) {
    const abortController = new AbortController();
    const channel = {
        id: createStreamId(),
        onmessage,
        cancel() {
            abortController.abort();
        },
    };
    subscribeStream(channel.id, (message) => {
        if (typeof channel.onmessage === 'function') {
            channel.onmessage(message);
        }
    }, abortController.signal);
    return channel;
}

// ---------------------------------------------------------------------------
// File/URL helpers
// ---------------------------------------------------------------------------

/**
 * Convert a server-side filesystem path to a browser-loadable URL. In server
 * mode the HTTP server validates the path and streams the file back.
 *
 * @param {string} path
 * @param {string} [_protocol]
 * @returns {string}
 */
export const convertFileSrc = (path, _protocol = 'asset') => {
    const value = String(path ?? '');
    if (!value) {
        return value;
    }
    return '/__tt/file?path=' + encodeURIComponent(value);
};

export function isTauri() {
    return detectTauriEnv();
}

export async function initializeBridge() {
    try {
        return await invoke('is_ready');
    } catch (error) {
        console.error('Failed to initialize RustTavern bridge:', error);
        return false;
    }
}

export async function initializeApp() {
    return initializeBridge();
}

export async function getVersion() {
    try {
        return await invoke('get_version');
    } catch (error) {
        // Fallback for non-host origins (e.g. extension iframes without the
        // invoke transport): ask the server for the version manifest.
        const response = await fetch('/version');
        return response.json();
    }
}

export async function getClientVersion() {
    try {
        return await invoke('get_client_version');
    } catch (error) {
        console.error('Error getting client version from backend:', error);
        const version = await getVersion();
        return {
            agent: `SillyTavern:${SILLYTAVERN_COMPAT_VERSION}:RustTavern`,
            pkgVersion: SILLYTAVERN_COMPAT_VERSION,
            productVersion: version,
            gitRevision: null,
            gitBranch: null,
            defaultUpdateChannel: 'stable',
        };
    }
}

export async function checkForUpdate(channel) {
    return invokeWithHostNormalization('check_for_update', { channel });
}

export async function getRustTavernSettings() {
    return invoke('get_rusttavern_settings');
}

export async function getChatBackupStorageStats() {
    return invoke('get_chat_backup_storage_stats');
}

export async function updateRustTavernSettings(dto) {
    if (!isPlainObject(dto)) {
        throw new Error('Invalid RustTavern settings DTO');
    }

    return invoke('update_rusttavern_settings', { dto });
}

export async function getRuntimePaths() {
    return invoke('get_runtime_paths');
}

export async function setDataRoot(dataRoot) {
    if (typeof dataRoot !== 'string' || dataRoot.trim() === '') {
        throw new Error('Invalid data root path');
    }
    return invokeWithHostNormalization('set_data_root', { data_root: dataRoot });
}

export async function openDialog(options = {}) {
    if (!isPlainObject(options)) {
        throw new Error('Invalid dialog options: expected an object');
    }

    Object.freeze(options);

    // Server mode: native dialogs are unavailable. Callers (character cards,
    // skills, host-bridge.openDialog) fall back to browser file inputs.
    return null;
}

function normalizeExternalUrl(url) {
    const value = String(url instanceof URL ? url.href : url ?? '').trim();
    if (!value) {
        throw new Error('External URL is required');
    }

    try {
        return new URL(value, window.location.href).toString();
    } catch {
        throw new Error(`Invalid external URL: ${value}`);
    }
}

// Captured at module load: on mobile the window-open compat shim replaces
// window.open with a wrapper that routes external URLs back into
// openExternalUrl, so calling the *live* window.open from here would recurse
// into that shim synchronously until the stack overflows, then fall through
// to location.assign and navigate the whole app away.
const nativeWindowOpen = typeof window !== 'undefined' && typeof window.open === 'function'
    ? window.open.bind(window)
    : null;

export async function openExternalUrl(url, openWith) {
    const href = normalizeExternalUrl(url);

    const openedWindow = nativeWindowOpen
        ? nativeWindowOpen(href, '_blank', 'noopener,noreferrer')
        : null;

    if (openedWindow) {
        return;
    }

    if (typeof window.location?.assign === 'function') {
        window.location.assign(href);
        return;
    }

    throw new Error('Unable to open external URL');
}

export function getAssetUrl(path) {
    if (!isTauriEnv || !convertFileSrc || !path) {
        return path;
    }

    try {
        return convertFileSrc(path, 'asset');
    } catch {
        return path;
    }
}
