export const RUN_EVENT_PAGE_LIMIT = 240;
export const RUN_EVENT_TAIL_SEQ = Number.MAX_SAFE_INTEGER;

export function createRunTimelineEventStore() {
    const eventsByKey = new Map();
    let events = [];

    // Bound the retained window: when the run outgrows two pages, drop the
    // oldest page so a very long run can't hold every event in memory.
    // Older pages are still reachable through `loadOlder` (beforeSeq).
    const prune = () => {
        if (events.length <= RUN_EVENT_PAGE_LIMIT * 2) {
            return;
        }
        const excess = events.length - RUN_EVENT_PAGE_LIMIT;
        const dropped = events.splice(0, excess);
        for (const event of dropped) {
            eventsByKey.delete(eventKey(event));
        }
    };

    return {
        add(event) {
            const key = eventKey(event);
            if (eventsByKey.has(key)) {
                return false;
            }
            eventsByKey.set(key, event);
            events.push(event);
            if (events.length > 1 && eventSeq(events[events.length - 2]) > eventSeq(event)) {
                events.sort(compareEvents);
            }
            prune();
            return true;
        },
        /**
         * Shared live view of the retained events, ordered by seq ascending.
         * Consumers must treat the returned array as read-only; the store
         * mutates it in place on add/prune.
         */
        events() {
            return events;
        },
        oldestSeq() {
            return events.length > 0 ? eventSeq(events[0]) : null;
        },
    };
}

function eventKey(event) {
    const id = String(event?.id || '').trim();
    if (id) {
        return `id:${id}`;
    }
    const runId = String(event?.runId || '').trim();
    const seq = eventSeq(event);
    return `seq:${runId}:${seq}`;
}

function eventSeq(event) {
    const seq = Number(event?.seq);
    if (!Number.isInteger(seq) || seq <= 0) {
        throw new Error('Agent run event seq must be a positive integer.');
    }
    return seq;
}

function compareEvents(left, right) {
    return eventSeq(left) - eventSeq(right);
}
