import assert from 'node:assert/strict';
import test from 'node:test';

import { WorldInfoCache } from '../src/scripts/world-info-cache.js';

test('evicts least-recently-used inactive books over the cap; pinned and just-set survive', () => {
    const pinned = new Set(['active']);
    const cache = new WorldInfoCache({
        cloneOnGet: false,
        isRetained: (name) => pinned.has(name),
        maxBytes: 100,
    });

    cache.set('old', {}, { bytes: 60 });
    cache.set('active', {}, { bytes: 60 });
    cache.set('new', {}, { bytes: 60 });

    assert.equal(cache.has('old'), false);
    assert.equal(cache.has('active'), true);
    assert.equal(cache.has('new'), true);
});

test('reads refresh recency so the least recently used book is evicted first', () => {
    const cache = new WorldInfoCache({ cloneOnGet: false, maxBytes: 100 });

    cache.set('a', {}, { bytes: 40 });
    cache.set('b', {}, { bytes: 40 });
    cache.get('a');
    cache.set('c', {}, { bytes: 40 });

    assert.equal(cache.has('b'), false);
    assert.equal(cache.has('a'), true);
    assert.equal(cache.has('c'), true);
});

test('measures unknown sizes lazily, honors delete, keeps a lone over-cap entry', () => {
    const cache = new WorldInfoCache({ cloneOnGet: false, maxBytes: 50 });
    const value = { entries: { 0: { uid: 0, content: 'y'.repeat(30) } } };
    assert.ok(JSON.stringify(value).length > 50);

    // No size hint: only a lazily measured size can push the total over the cap
    // and evict 'lazy' (a null size would count as 0 and keep it).
    cache.set('lazy', value);
    cache.set('small', {}, { bytes: 1 });
    assert.equal(cache.has('lazy'), false);
    assert.equal(cache.has('small'), true);

    const cache2 = new WorldInfoCache({ cloneOnGet: false, maxBytes: 100 });
    cache2.set('x', {}, { bytes: 10 });
    cache2.set('y', {}, { bytes: 10 });
    assert.equal(cache2.has('x'), true);

    cache2.delete('y');
    assert.equal(cache2.has('y'), false);
    cache2.set('y', {}, { bytes: 500 });
    assert.equal(cache2.has('y'), true);
});

test('keeps StructuredCloneMap clone-on-get semantics', () => {
    const cache = new WorldInfoCache({ cloneOnGet: true, cloneOnSet: false });
    const data = { entries: { 0: { uid: 0 } } };

    cache.set('a', data);
    const copy = cache.get('a');
    assert.notEqual(copy, data);
    assert.deepEqual(copy, data);

    copy.entries[0].uid = 99;
    assert.equal(cache.get('a').entries[0].uid, 0);
});
