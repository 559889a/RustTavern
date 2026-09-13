import { createApp } from 'vue/dist/vue.esm-bundler.js';

import { createRustTavernDevLogsApp } from './DevLogsApp.js';

export function mountRustTavernDevLogsApp(mount, options) {
    if (!(mount instanceof HTMLElement)) {
        throw new Error('RustTavern dev logs mount element is required');
    }

    const app = createApp(createRustTavernDevLogsApp(options));
    app.mount(mount);

    return {
        unmount: () => app.unmount(),
    };
}
