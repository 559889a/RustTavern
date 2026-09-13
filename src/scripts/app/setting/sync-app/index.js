import { createApp } from 'vue/dist/vue.esm-bundler.js';
import { createRustTavernSyncApp } from './SyncApp.js';
import { createRustTavernSyncProgressApp } from './SyncProgressApp.js';
import { createRustTavernSyncScopeApp } from './SyncScopeApp.js';

export function mountRustTavernSyncApp(mount, options) {
    if (!(mount instanceof HTMLElement)) {
        throw new Error('RustTavern Sync mount element is required');
    }

    const app = createApp(createRustTavernSyncApp(options));
    const instance = app.mount(mount);

    return {
        refresh: () => instance.refresh(),
        refreshAutomationStatus: () => instance.refreshAutomationStatus(),
        unmount: () => app.unmount(),
    };
}

export function mountRustTavernSyncProgressApp(mount, options) {
    if (!(mount instanceof HTMLElement)) {
        throw new Error('RustTavern Sync progress mount element is required');
    }

    const app = createApp(createRustTavernSyncProgressApp(options));
    const instance = app.mount(mount);

    return {
        update: (next) => instance.update(next),
        unmount: () => app.unmount(),
    };
}

export function mountRustTavernSyncScopeApp(mount, options) {
    if (!(mount instanceof HTMLElement)) {
        throw new Error('RustTavern Sync scope mount element is required');
    }

    const app = createApp(createRustTavernSyncScopeApp(options));
    const instance = app.mount(mount);

    return {
        getSelection: () => instance.getSelection(),
        unmount: () => app.unmount(),
    };
}
