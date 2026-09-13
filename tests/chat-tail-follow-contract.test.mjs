import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// The tail follow used to be a single fire-and-forget frame at the append site: a
// message whose layout landed after that frame grew the scroller behind it and the
// view stayed parked short of the bottom with nothing to correct it. These
// assertions pin the re-asserting shape that replaced it.
test('the chat tail follow re-asserts the bottom on content and box changes', async () => {
    const source = await readFile(path.join(REPO_ROOT, 'src/script.js'), 'utf8');

    const schedule = source.indexOf('const scheduleTailFollow = () => {');
    assert.notEqual(schedule, -1, 'scheduleTailFollow must exist');

    const body = source.slice(schedule, source.indexOf('\n    };', schedule));
    assert.match(body, /!power_user\.auto_scroll_chat_to_bottom/, 'must honour the auto-scroll preference');
    assert.match(body, /tailSettleFramesLeft = TAIL_SETTLE_MAX_FRAMES/, 'a content change must allow the scroller to settle');

    const tick = source.indexOf('const tailFollowTick = () => {');
    assert.notEqual(tick, -1, 'tailFollowTick must exist');
    const tickBody = source.slice(tick, source.indexOf('\n    };', tick));
    assert.match(tickBody, /if \(scrollLock \|\| !power_user\.auto_scroll_chat_to_bottom\)/, 'must stay off while the reader has scrolled away');
    assert.match(tickBody, /querySelector\('textarea, \[contenteditable="true"\]'\)/, 'must not pull an open editor out from under the reader');
    assert.match(tickBody, /scrollChatToBottom\(\)/, 'must re-assert through the shared scroll helper');
    assert.match(tickBody, /tailFollowFrame = requestAnimationFrame\(tailFollowTick\)/, 'must keep re-asserting while the scroller is still growing');

    const wiring = source.slice(source.indexOf('\n    };\n', schedule) + 8);
    assert.match(wiring.slice(0, 1400), /new MutationObserver\(scheduleTailFollow\)\.observe\(chatElementScroll, \{/, 'content changes must drive it');
    assert.match(wiring.slice(0, 1400), /new ResizeObserver\(scheduleTailFollow\)\.observe\(chatElementScroll\)/, 'scroller box changes must drive it');
    assert.match(wiring.slice(0, 1400), /addEventListener\('load', scheduleTailFollow, \{ capture: true, passive: true \}\)/, 'late media growth must drive it');
});

// The gate has to be the reader's intent (wheel/touchmove), never a position.
// Deriving "is the reader at the bottom" from the scroll position reads the
// scroller growing under a new message as the reader leaving the tail, and then
// stops following exactly when the follow is needed.
test('the tail follow is gated by reader intent, not by a derived position', async () => {
    const source = await readFile(path.join(REPO_ROOT, 'src/script.js'), 'utf8');

    const tick = source.indexOf('const tailFollowTick = () => {');
    const tickBody = source.slice(tick, source.indexOf('\n    };', tick));
    assert.match(tickBody, /if \(scrollLock \|\| !power_user\.auto_scroll_chat_to_bottom\)/, 'the scroll lock must be the gate');
    assert.doesNotMatch(tickBody, /readerAtTail|TAIL_ATTACH_PX|TAIL_DETACH_PX/, 'a position-derived gate must not come back');

    // Only the reader's own input may set the lock. Waifu mode is the one other
    // setter (it parks the reader on the last message and nothing follows there).
    const mark = source.indexOf('const markUserScroll = () => { scrollLock = true; };');
    assert.notEqual(mark, -1, 'the lock must be set by reader input');
    const setters = source.match(/scrollLock = true;/g) ?? [];
    assert.equal(setters.length, 2, 'wheel/touchmove and waifu mode are the only setters');
    assert.match(source, /addEventListener\('wheel', markUserScroll, \{ passive: true \}\)/, 'wheel must set it');
    assert.match(source, /addEventListener\('touchmove', markUserScroll, \{ passive: true \}\)/, 'touchmove must set it');

    // ...and nothing inside the tail-follow block may set it.
    const blockStart = source.indexOf('const readTailGap = () =>');
    const blockEnd = source.indexOf('new MutationObserver(scheduleTailFollow)');
    assert.doesNotMatch(source.slice(blockStart, blockEnd), /scrollLock = true/, 'growing content must not be able to arm the lock');
});

test('the tail follow is coalesced to one frame and stops once settled', async () => {
    const source = await readFile(path.join(REPO_ROOT, 'src/script.js'), 'utf8');

    const schedule = source.indexOf('const scheduleTailFollow = () => {');
    const body = source.slice(schedule, source.indexOf('\n    };', schedule));
    assert.match(body, /if \(tailFollowFrame !== null\) \{\s*\n\s*return;/, 'only one frame may be in flight');

    const tick = source.indexOf('const tailFollowTick = () => {');
    const tickBody = source.slice(tick, source.indexOf('\n    };', tick));
    assert.match(tickBody, /tailFollowFrame = null;/, 'the frame slot must be released before the work runs');
    assert.match(
        tickBody,
        /if \(readTailGap\(\) <= TAIL_SETTLE_PX\) \{[\s\S]*?tailSettleFramesLeft = 0;[\s\S]*?return;/,
        'a view already settled on the tail must stop the run instead of re-asserting forever',
    );
    assert.match(tickBody, /chat tail still unpinned after re-asserting/, 'a tail it cannot pin must be reported, not drifted silently');
});
