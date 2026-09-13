import { StructuredCloneMap } from './util/StructuredCloneMap.js';

// ponytail: 32MB of serialized-JSON bytes. Heuristic cap for LRU eviction,
// not a hard memory bound -- active books are exempt and a single giant book
// alone stays cached. Lower it if mobile OOM reports persist; the ceiling is
// the upstream behavior of retaining every loaded lorebook forever.
export const WORLD_INFO_CACHE_MAX_BYTES = 32 * 1024 * 1024;

/**
 * A `StructuredCloneMap` for world info data that evicts.
 *
 * The upstream cache retains every loaded lorebook forever; one multi-MB book
 * keeps a ~4x larger object graph alive on mobile (handoff 8L). Entries are
 * evicted least-recently-used once the tracked serialized size exceeds the
 * cap, but never books for which `isRetained` returns true (active books are
 * re-read on every generation and evicting them would refetch per
 * generation), and never the entry currently being set.
 */
export class WorldInfoCache extends StructuredCloneMap {
    #meta = new Map();
    #clock = 0;
    #isRetained;
    #maxBytes;

    /**
     * @param {{ cloneOnGet?: boolean, cloneOnSet?: boolean, isRetained?: (name: string) => boolean, maxBytes?: number }} [options]
     */
    constructor({ isRetained = () => false, maxBytes = WORLD_INFO_CACHE_MAX_BYTES, ...cloneOptions } = {}) {
        super(cloneOptions);
        this.#isRetained = isRetained;
        this.#maxBytes = maxBytes;
    }

    /**
     * @param {string} name
     * @param {object} value
     * @param {{ bytes?: number }} [hint] Serialized JSON byte size when the caller already knows it
     */
    set(name, value, hint) {
        const meta = this.#meta.get(name) ?? { bytes: null };
        if (Number.isFinite(hint?.bytes)) {
            meta.bytes = hint.bytes;
        }
        meta.lastUsed = ++this.#clock;
        this.#meta.set(name, meta);
        const result = super.set(name, value);
        this.#evict(name);
        return result;
    }

    get(name) {
        this.#touch(name);
        return super.get(name);
    }

    has(name) {
        const present = super.has(name);
        if (present) {
            this.#touch(name);
        }
        return present;
    }

    delete(name) {
        this.#meta.delete(name);
        return super.delete(name);
    }

    #touch(name) {
        const meta = this.#meta.get(name);
        if (meta) {
            meta.lastUsed = ++this.#clock;
        }
    }

    #evict(reservedName) {
        let total = 0;
        const candidates = [];

        for (const [name, meta] of this.#meta) {
            total += this.#measure(name, meta);
            if (name !== reservedName && !this.#isRetained(name)) {
                candidates.push(name);
            }
        }

        if (total <= this.#maxBytes) {
            return;
        }

        candidates.sort((a, b) => this.#meta.get(a).lastUsed - this.#meta.get(b).lastUsed);
        for (const name of candidates) {
            if (total <= this.#maxBytes) {
                break;
            }

            total -= this.#meta.get(name).bytes;
            this.#meta.delete(name);
            super.delete(name);
        }
    }

    // Sizes unknown at set-time (batch prefetch) are measured once, on the
    // first eviction pass that needs them. Reads the raw stored value -- a
    // regular get() would structuredClone the book just to measure it.
    #measure(name, meta) {
        if (meta.bytes === null) {
            const value = Map.prototype.get.call(this, name);
            meta.bytes = value !== undefined ? utf8ByteLength(JSON.stringify(value)) : 0;
        }
        return meta.bytes;
    }
}

const textEncoder = new TextEncoder();

/** @param {string} s */
function utf8ByteLength(s) {
    return textEncoder.encode(s).byteLength;
}
