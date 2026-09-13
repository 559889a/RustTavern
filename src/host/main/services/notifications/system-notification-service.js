// @ts-check

/**
 * @typedef {'granted' | 'denied' | 'prompt'} NotificationPermissionState
 * @typedef {(command: import('../../context/types.js').TauriInvokeCommand, args?: any) => Promise<any>} SafeInvokeFn
 */

const NOTIFICATION_PERMISSION_STATES = new Set(['granted', 'denied', 'prompt']);
const NOTIFICATION_PERMISSION_REJECTION_COUNT_STORAGE_KEY = 'tt:notification-permission-rejection-count';
const NOTIFICATION_PERMISSION_REJECTION_LIMIT = 3;

/**
 * @param {unknown} value
 * @returns {NotificationPermissionState}
 */
function normalizePermissionState(value) {
    const normalized = String(value || '').trim().toLowerCase();
    if (NOTIFICATION_PERMISSION_STATES.has(normalized)) {
        return /** @type {NotificationPermissionState} */ (normalized);
    }

    throw new Error(`Unsupported notification permission state: ${String(value || '')}`);
}

/**
 * Browser-mode fallbacks for the native notification commands. The backend
 * registry has no notification commands in the B/S form (they belonged to the
 * removed Tauri mobile shell), so the Web Notification API is the real
 * implementation here. The safeInvoke path stays first so a future native
 * provider keeps working.
 */

/** @returns {NotificationPermissionState | null} */
function browserPermissionState() {
    if (typeof globalThis.Notification === 'undefined') {
        return null;
    }
    return normalizePermissionState(globalThis.Notification.permission);
}

/**
 * @param {SafeInvokeFn} safeInvoke
 * @returns {Promise<NotificationPermissionState>}
 */
async function getPermissionStateWithBrowserFallback(safeInvoke) {
    try {
        return normalizePermissionState(await safeInvoke('get_notification_permission_state'));
    } catch (error) {
        const browserState = browserPermissionState();
        if (browserState !== null) {
            return browserState;
        }
        throw error;
    }
}

/**
 * @param {SafeInvokeFn} safeInvoke
 * @returns {Promise<NotificationPermissionState>}
 */
async function requestPermissionWithBrowserFallback(safeInvoke) {
    try {
        return normalizePermissionState(await safeInvoke('request_notification_permission'));
    } catch {
        // Fall through to the browser API below.
    }

    if (typeof globalThis.Notification === 'undefined' || typeof globalThis.Notification.requestPermission !== 'function') {
        throw new Error('Notifications are not supported in this environment');
    }

    return normalizePermissionState(await globalThis.Notification.requestPermission());
}

/**
 * @param {SafeInvokeFn} safeInvoke
 * @param {{ title: string; body: string }} params
 */
async function showWithBrowserFallback(safeInvoke, { title, body }) {
    const payload = {
        dto: {
            title: String(title ?? '').trim(),
            body: String(body ?? '').trim(),
        },
    };

    try {
        await safeInvoke('show_system_notification', payload);
        return;
    } catch {
        // Fall through to the browser API below.
    }

    if (typeof globalThis.Notification === 'undefined') {
        throw new Error('Notifications are not supported in this environment');
    }

    new globalThis.Notification(payload.dto.title, { body: payload.dto.body });
}

/**
 * @param {Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>} storage
 * @returns {number}
 */
function getPermissionRejectionCount(storage) {
    const raw = storage.getItem(NOTIFICATION_PERMISSION_REJECTION_COUNT_STORAGE_KEY);
    const count = Number.parseInt(String(raw ?? ''), 10);
    return Number.isSafeInteger(count) && count > 0 ? count : 0;
}

/**
 * @param {Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>} storage
 * @param {number} count
 */
function setPermissionRejectionCount(storage, count) {
    if (count <= 0) {
        storage.removeItem(NOTIFICATION_PERMISSION_REJECTION_COUNT_STORAGE_KEY);
        return;
    }

    storage.setItem(NOTIFICATION_PERMISSION_REJECTION_COUNT_STORAGE_KEY, String(count));
}

/**
 * @param {{
 *   safeInvoke: SafeInvokeFn;
 *   confirmPermissionRationale: () => Promise<boolean>;
 *   storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
 * }} deps
 */
export function createSystemNotificationService({
    safeInvoke,
    confirmPermissionRationale,
    storage = globalThis.localStorage,
}) {
    /** @type {Promise<NotificationPermissionState> | null} */
    let permissionRequestPromise = null;
    /** @type {Promise<boolean> | null} */
    let permissionRationalePromise = null;

    function resetPermissionRejectionCount() {
        setPermissionRejectionCount(storage, 0);
    }

    function incrementPermissionRejectionCount() {
        const nextCount = getPermissionRejectionCount(storage) + 1;
        setPermissionRejectionCount(storage, nextCount);
        return nextCount;
    }

    async function getPermissionState() {
        return getPermissionStateWithBrowserFallback(safeInvoke);
    }

    async function requestPermission() {
        if (!permissionRequestPromise) {
            permissionRequestPromise = requestPermissionWithBrowserFallback(safeInvoke)
                .then((state) => {
                    if (state === 'granted') {
                        resetPermissionRejectionCount();
                        return state;
                    }

                    incrementPermissionRejectionCount();
                    return state;
                })
                .finally(() => {
                    permissionRequestPromise = null;
                });
        }

        return permissionRequestPromise;
    }

    async function confirmPermissionRationaleOnce() {
        if (getPermissionRejectionCount(storage) >= NOTIFICATION_PERMISSION_REJECTION_LIMIT) {
            return false;
        }

        if (!permissionRationalePromise) {
            permissionRationalePromise = confirmPermissionRationale()
                .then((accepted) => {
                    if (!accepted) {
                        incrementPermissionRejectionCount();
                    }

                    return accepted;
                })
                .finally(() => {
                    permissionRationalePromise = null;
                });
        }

        return permissionRationalePromise;
    }

    async function preparePermission() {
        const currentState = await getPermissionState();
        if (currentState === 'granted') {
            resetPermissionRejectionCount();
            return currentState;
        }

        if (currentState !== 'prompt') {
            return currentState;
        }

        const accepted = await confirmPermissionRationaleOnce();
        if (!accepted) {
            return currentState;
        }

        return requestPermission();
    }

    /**
     * @param {{ title: string; body: string }} params
     */
    async function show({ title, body }) {
        await showWithBrowserFallback(safeInvoke, { title, body });
    }

    return {
        getPermissionState,
        requestPermission,
        preparePermission,
        show,
    };
}
