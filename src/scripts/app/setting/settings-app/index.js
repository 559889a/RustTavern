import { createApp } from 'vue/dist/vue.esm-bundler.js';

import { createRustTavernSettingsApp } from './SettingsApp.js';

export function mountRustTavernSettingsApp(mount, options) {
    if (!(mount instanceof HTMLElement)) {
        throw new Error('RustTavern settings mount element is required');
    }

    const app = createApp(createRustTavernSettingsApp(options));
    const vm = app.mount(mount);
    let mounted = true;

    return {
        getDraft: () => vm.getDraft(),
        setChatBackupStorageStats: stats => {
            if (mounted) {
                vm.chatBackupStorageStats = stats;
            }
        },
        unmount: () => {
            mounted = false;
            app.unmount();
        },
    };
}
