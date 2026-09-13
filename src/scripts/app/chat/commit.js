import { invoke } from '../../../host-bridge.js';
import { encodeBytesToBase64 } from '../../../host/main/binary-utils.js';
import { payloadToJsonlByteChunks } from './jsonl.js';

function positiveSafeInteger(value, label) {
    const number = Number(value);
    if (!Number.isSafeInteger(number) || number <= 0) {
        throw new Error(`${label} must be a positive safe integer`);
    }
    return number;
}

export async function commitChatPayload({ target, payload, force, commitReason, baseline }) {
    let sessionId = '';
    const normalizedCommitReason = commitReason ?? 'mutation';

    try {
        // `baseline` ({size, mtimeSec} of the file as this client last saw
        // it) is omitted entirely when absent: the server treats a missing
        // key as "no baseline known" and skips the concurrent-editor check.
        const begin = await invoke('begin_chat_commit', baseline
            ? { target, force, baseline }
            : { target, force });
        sessionId = String(begin?.sessionId || '').trim();
        if (!sessionId) {
            throw new Error('Host chat commit did not return a session id');
        }

        const maxFrameBytes = positiveSafeInteger(begin?.maxFrameBytes, 'Host chat commit frame limit');
        let offset = 0;

        for (const frame of payloadToJsonlByteChunks(payload, { maxChunkBytes: maxFrameBytes })) {
            // Chunks always travel as base64 JSON: the server-side
            // `append_chat_commit_chunk` signature takes `{ session_id,
            // offset, data }` (the legacy raw-bytes + headers IPC form no
            // longer exists).
            const nextOffset = Number(await invoke('append_chat_commit_chunk', {
                session_id: sessionId,
                offset,
                data: encodeBytesToBase64(frame),
            }));
            const expectedNextOffset = offset + frame.byteLength;
            if (nextOffset !== expectedNextOffset) {
                throw new Error(`Host chat commit returned unexpected offset ${nextOffset}`);
            }
            offset = nextOffset;
        }

        const finished = await invoke('finish_chat_commit', {
            sessionId,
            expectedSize: offset,
            commitReason: normalizedCommitReason,
        });
        sessionId = '';

        if (Number(finished?.size) !== offset) {
            throw new Error(`Host chat commit returned unexpected size ${finished?.size}`);
        }

        return {
            baseline: Number(finished?.mtimeSec) > 0
                ? { size: Number(finished.size), mtimeSec: Number(finished.mtimeSec) }
                : null,
        };
    } catch (error) {
        if (sessionId) {
            try {
                await invoke('abort_chat_commit', { sessionId });
            } catch (abortError) {
                throw new AggregateError(
                    [error, abortError],
                    String(error?.message || error || 'Chat commit failed'),
                );
            }
        }
        throw error;
    }
}
