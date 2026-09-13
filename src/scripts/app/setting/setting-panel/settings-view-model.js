// @ts-check

import { isMobile } from '../../../RossAscends-mods.js';
import { isAndroidRuntime, isIosRuntime } from '../../../util/mobile-runtime.js';
import {
    getChatBackupStorageStats,
    getRuntimePaths,
    getRustTavernSettings,
} from '../../../../host-bridge.js';
import { getActiveIosPolicyCapabilities } from '../../../rusttavern/ios-policy.js';
import {
    isNativeRegexBackendEnabled,
    syncNativeRegexBackendEnabledFromSettings,
} from '../../regex/native-regex-settings.js';
import { createDataRootState, createRustTavernSettingsState } from './settings-state.js';

export function isWindowsPlatform() {
    return typeof navigator !== 'undefined'
        && /windows/i.test(String(navigator.userAgent || ''));
}

export function resolveRustTavernSettingsCapabilities() {
    const iosCaps = getActiveIosPolicyCapabilities();
    // Data directory selection is a desktop-only feature. Do not gate this on Bowser's `isMobile()`,
    // because iPadOS may present a desktop-like user agent (e.g. platform "MacIntel").
    const supportsDataRootSelection = !isAndroidRuntime() && !isIosRuntime();

    return {
        requestProxyAllowed: iosCaps?.network?.request_proxy !== false,
        lanSyncAllowed: iosCaps?.sync?.lan !== false,
        supportsCloseToTrayOnClose: isWindowsPlatform() && !isMobile(),
        supportsDataRootSelection,
    };
}

function normalizeChatBackupStorageStats(stats) {
    if (stats === null) {
        return null;
    }

    const originalBytes = Number(stats?.original_bytes);
    const storedBytes = Number(stats?.stored_bytes);
    if (
        !Number.isSafeInteger(originalBytes)
        || originalBytes < 0
        || !Number.isSafeInteger(storedBytes)
        || storedBytes < 0
    ) {
        throw new Error('RustTavern settings: invalid chat backup storage stats');
    }

    return { originalBytes, storedBytes };
}

export async function loadChatBackupStorageStats() {
    return normalizeChatBackupStorageStats(await getChatBackupStorageStats());
}

export async function loadRustTavernSettingsViewModel() {
    const settings = await getRustTavernSettings();
    const capabilities = resolveRustTavernSettingsCapabilities();
    const { supportsDataRootSelection } = capabilities;
    const runtimePaths = supportsDataRootSelection ? await getRuntimePaths() : null;

    syncNativeRegexBackendEnabledFromSettings(settings);

    return {
        capabilities,
        dataRoot: createDataRootState(runtimePaths),
        values: createRustTavernSettingsState(settings, {
            nativeRegexBackendEnabled: isNativeRegexBackendEnabled(),
        }),
    };
}
