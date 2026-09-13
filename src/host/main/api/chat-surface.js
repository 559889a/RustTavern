// @ts-check

import { CHAT_SURFACE_PROTOCOL_VERSION } from '../services/chat-surface/participant-registry.js';
import {
    getChatSurfaceParticipantRegistry,
    isManagedChatSurfaceOwnershipRequired,
} from '../services/chat-surface/runtime.js';

export function installChatSurfaceApi() {
    const hostAbi = window.__RUSTTAVERN__;
    if (!hostAbi || typeof hostAbi !== 'object') {
        throw new Error('Host ABI __RUSTTAVERN__ is missing');
    }
    hostAbi.api ??= {};

    const registry = getChatSurfaceParticipantRegistry();
    hostAbi.api.chatSurface = Object.freeze({
        protocolVersion: CHAT_SURFACE_PROTOCOL_VERSION,
        isManagedOwnershipRequired: isManagedChatSurfaceOwnershipRequired,
        registerParticipant: registry.register,
    });
}
