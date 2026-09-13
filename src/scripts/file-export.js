const INVALID_FILENAME_CHARS = /[\\/:*?"<>|]+/g;
const TRAILING_DOTS_OR_SPACES = /[. ]+$/g;
const DEFAULT_FALLBACK_FILE_NAME = 'download.bin';
const DEFAULT_MIME_TYPE = 'application/octet-stream';
const MIME_TYPE_PATTERN = /^[A-Za-z0-9][A-Za-z0-9!#$&^_.+-]*\/[A-Za-z0-9][A-Za-z0-9!#$&^_.+-]*$/;

function isMobileRuntime() {
    // NOTE: Intentionally self-contained UA check.
    // `file-export` is used from multiple entry points (web + host kernel). Keeping
    // this local avoids cross-module dependencies/cycles for a small, runtime-only
    // decision. Mobile browsers still benefit from the browser download path.
    if (typeof navigator === 'undefined') {
        return false;
    }

    const userAgent = typeof navigator.userAgent === 'string' ? navigator.userAgent : '';
    return /android|iphone|ipad|ipod/i.test(userAgent);
}

// The server form has no native Tauri runtime; downloads always go through the
// browser. The export is kept for API-shape compatibility with callers that
// branch on it.
export function isNativeMobileDownloadRuntime() {
    return false;
}

function sanitizeDownloadFileName(value, fallback = DEFAULT_FALLBACK_FILE_NAME) {
    const fallbackName = String(fallback || DEFAULT_FALLBACK_FILE_NAME).trim() || DEFAULT_FALLBACK_FILE_NAME;
    const rawName = String(value || '').trim();
    const candidate = (rawName || fallbackName)
        .replace(INVALID_FILENAME_CHARS, '_')
        .replace(TRAILING_DOTS_OR_SPACES, '')
        .trim();

    return candidate || fallbackName;
}

function triggerBrowserDownload(blob, fileName, { fallbackName = DEFAULT_FALLBACK_FILE_NAME } = {}) {
    const payload = blob instanceof Blob ? blob : new Blob([blob ?? '']);
    const normalizedName = sanitizeDownloadFileName(fileName, fallbackName);
    const objectUrl = URL.createObjectURL(payload);
    const anchor = document.createElement('a');

    anchor.href = objectUrl;
    anchor.download = normalizedName;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();

    // Let the browser begin the download before releasing the object URL.
    setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
}

export async function downloadBlobWithRuntime(
    blob,
    fileName,
    {
        fallbackName = DEFAULT_FALLBACK_FILE_NAME,
    } = {},
) {
    const payload = blob instanceof Blob ? blob : new Blob([blob ?? '']);

    triggerBrowserDownload(payload, fileName, { fallbackName });
    return { mode: 'browser', savedPath: '' };
}

// Kept for callers that still reference it (e.g. agent-system extensions). The
// server form has no native file bridge; streamed exports must be converted to
// a Blob and downloaded through the browser path instead.
export async function writeReadableStreamToMobileDownloadFolder(stream, fileName, options = {}) {
    throw new Error('Native mobile download folders are unavailable in server mode');
}
