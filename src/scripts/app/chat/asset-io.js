import { convertFileSrc } from '../../../host-bridge.js';

// The native fs bridge (plugin:fs|open/read/close) only exists inside the
// Tauri WebView. In server mode (plain browser, possibly on an Android
// device) `window.__TAURI__` is absent, so the fs path must not activate.
// Kept as a compatibility marker: no callers today, but the detection is
// required if a Tauri runtime is ever reintroduced.
function hasNativeTauriRuntime() {
    return typeof window !== 'undefined'
        && typeof window.__TAURI__?.core?.invoke === 'function';
}

/**
 * Fetch a chat payload stream plus the file's (size, mtime-seconds) as the
 * server served it. The pair travels back to the chat-commit `begin` on the
 * next save, letting the backend reject a save that would silently overwrite
 * another editor's changes. `baseline` is null when the response headers
 * carry no usable signature (older server, non-HTTP bridge).
 *
 * @param {string} filePath
 * @returns {Promise<{ stream: ReadableStream<Uint8Array>, baseline: { size: number, mtimeSec: number } | null }>}
 */
export async function fetchAssetStreamWithBaseline(filePath) {
    const assetUrl = convertFileSrc(filePath, 'asset');
    const response = await fetch(assetUrl, { cache: 'no-store' });
    if (!response.ok) {
        throw new Error(`Failed to fetch asset payload: ${response.status}`);
    }

    if (!response.body) {
        throw new Error('Asset response body is unavailable');
    }

    const size = Number(response.headers.get('content-length')) || 0;
    const lastModified = response.headers.get('last-modified');
    const mtimeSec = lastModified ? Math.floor(Date.parse(lastModified) / 1000) : 0;
    const baseline = size > 0 && mtimeSec > 0
        ? { size, mtimeSec }
        : null;

    return { stream: response.body, baseline };
}
