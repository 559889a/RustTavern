// @ts-check

import { listen } from '../../../../host-bridge.js';

/**
 * @template T
 * @param {{
 *   safeInvoke: (command: any, args?: any) => Promise<any>;
 *   enableCommand: any;
 *   eventName: string;
 * }} deps
 */
export function createTauriEventStreamBridge({ safeInvoke, enableCommand, eventName }) {
    /** @type {Set<(entry: T) => void>} */
    const subscribers = new Set();
    /** @type {(() => void) | null} */
    let unlisten = null;
    /** @type {Promise<void> | null} */
    let starting = null;
    /** @type {Promise<void> | null} */
    let stopping = null;

    /**
     * @param {T} entry
     */
    function dispatch(entry) {
        for (const handler of subscribers) {
            handler(entry);
        }
    }

    async function ensureStarted() {
        // Serialize with a concurrent stopIfIdle: its disable invoke is still
        // in flight when `unlisten` is already null, so racing a fresh enable
        // against it can let the disable land last, leaving the backend stream
        // off while subscribers wait for events that never come.
        while (!unlisten) {
            if (starting) {
                await starting;
                continue;
            }
            if (stopping) {
                await stopping;
                continue;
            }

            starting = (async () => {
                await safeInvoke(enableCommand, { enabled: true });
                try {
                    unlisten = await listen(eventName, /** @param {{ payload: T }} event */ (event) => {
                        dispatch(/** @type {T} */ (event.payload));
                    });
                } catch (error) {
                    await safeInvoke(enableCommand, { enabled: false });
                    throw error;
                }
            })();

            try {
                await starting;
            } finally {
                starting = null;
            }
        }
    }

    async function stopIfIdle() {
        if (subscribers.size > 0) {
            return;
        }

        if (starting) {
            await starting;
            if (subscribers.size > 0) {
                return;
            }
        }

        if (!unlisten) {
            return;
        }

        const stopListening = unlisten;
        unlisten = null;
        stopListening();
        stopping = (async () => {
            await safeInvoke(enableCommand, { enabled: false });
        })();
        try {
            await stopping;
        } finally {
            stopping = null;
        }
    }

    /**
     * @param {(entry: T) => void} handler
     */
    async function subscribe(handler) {
        if (typeof handler !== 'function') {
            throw new Error('handler must be a function');
        }

        subscribers.add(handler);
        await ensureStarted();

        return () => {
            subscribers.delete(handler);
            void stopIfIdle();
        };
    }

    return { subscribe };
}
