import { invoke } from '../../../host-bridge.js';
import { stripJsonl } from '../../../host/main/kernel/chat-utils.js';
import {
    characterStemFromAvatarFileName,
    hasCharacterAvatarIdentity,
} from '../../../host/main/services/characters/character-identity.js';
import { fetchAssetStreamWithBaseline } from './asset-io.js';
import { commitChatPayload } from './commit.js';
import { jsonlStreamToPayload } from './jsonl.js';

export const CHAT_COMMIT_REASON = Object.freeze({
    MUTATION: 'mutation',
    PROVIDER_BARRIER: 'providerBarrier',
    GENERATION_CHECKPOINT: 'generationCheckpoint',
    MAINTENANCE: 'maintenance',
});

// Last-seen (size, mtimeSec) per chat file, captured at load and replayed
// into the next commit's `begin` so the backend can reject a save that would
// silently overwrite another editor's changes (the integrity slug never
// rotates, so it cannot detect this). Per-tab by construction — this module
// lives in the browser.
const chatBaselines = new Map();

function rememberChatBaseline(key, baseline) {
    if (baseline) {
        chatBaselines.set(key, baseline);
    } else {
        chatBaselines.delete(key);
    }
}

function currentChatBaseline(key) {
    return chatBaselines.get(key) || null;
}

export function normalizeChatFileName(fileName) {
    return stripJsonl(fileName);
}

/**
 * Chat folders are keyed by the character avatar filename stem.
 * avatarUrl is SillyTavern's avatar_url API field, not a browser asset URL.
 */
export function resolveCharacterDirectoryId(characterName, avatarUrl) {
    if (hasCharacterAvatarIdentity(avatarUrl)) {
        return characterStemFromAvatarFileName(avatarUrl, 'avatar_url', { required: true });
    }

    return String(characterName || '').trim();
}

export async function loadCharacterChatPayload({ characterName, avatarUrl, fileName, allowNotFound = false }) {
    const normalizedCharacter = resolveCharacterDirectoryId(characterName, avatarUrl);
    const normalizedFile = normalizeChatFileName(fileName);
    if (!normalizedCharacter || !normalizedFile.trim()) {
        throw new Error('Invalid character chat payload request');
    }

    const path = await invoke('get_chat_payload_path', {
        characterName: normalizedCharacter,
        fileName: normalizedFile,
        allowNotFound,
    });

    if (!path) {
        if (allowNotFound) {
            return [];
        }
        throw new Error('Chat payload path is empty');
    }

    const { stream, baseline } = await fetchAssetStreamWithBaseline(path);
    rememberChatBaseline(`character\u0000${normalizedCharacter}\u0000${normalizedFile}`, baseline);
    return jsonlStreamToPayload(stream);
}

export async function saveCharacterChatPayload({ characterName, avatarUrl, fileName, payload, force = false, commitReason = CHAT_COMMIT_REASON.MUTATION }) {
    const normalizedCharacter = resolveCharacterDirectoryId(characterName, avatarUrl);
    const normalizedFile = normalizeChatFileName(fileName);
    if (!Array.isArray(payload) || payload.length === 0 || !normalizedCharacter || !normalizedFile.trim()) {
        throw new Error('Invalid chat payload');
    }

    const baselineKey = `character\u0000${normalizedCharacter}\u0000${normalizedFile}`;
    const committed = await commitChatPayload({
        target: {
            kind: 'character',
            characterId: normalizedCharacter,
            fileName: normalizedFile,
        },
        payload,
        force,
        commitReason,
        baseline: force ? null : currentChatBaseline(baselineKey),
    });
    rememberChatBaseline(baselineKey, committed?.baseline || null);
}

export async function loadGroupChatPayload({ id, allowNotFound = false }) {
    const normalizedId = normalizeChatFileName(id);
    if (!normalizedId.trim()) {
        throw new Error('Invalid group chat payload request');
    }

    const path = await invoke('get_group_chat_path', {
        id: normalizedId,
        allowNotFound,
    });

    if (!path) {
        if (allowNotFound) {
            return [];
        }
        throw new Error('Group chat payload path is empty');
    }

    const { stream, baseline } = await fetchAssetStreamWithBaseline(path);
    rememberChatBaseline(`group\u0000${normalizedId}`, baseline);
    return jsonlStreamToPayload(stream);
}

export async function saveGroupChatPayload({ id, payload, force = false, commitReason = CHAT_COMMIT_REASON.MUTATION }) {
    const normalizedId = normalizeChatFileName(id);
    if (!Array.isArray(payload) || payload.length === 0 || !normalizedId.trim()) {
        throw new Error('Invalid group chat payload');
    }

    const baselineKey = `group\u0000${normalizedId}`;
    const committed = await commitChatPayload({
        target: {
            kind: 'group',
            chatId: normalizedId,
        },
        payload,
        force,
        commitReason,
        baseline: force ? null : currentChatBaseline(baselineKey),
    });
    rememberChatBaseline(baselineKey, committed?.baseline || null);
}
