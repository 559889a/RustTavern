import { registerSystemRoutes } from './system-routes.js';
import { registerBootstrapRoutes } from './bootstrap-routes.js';
import { registerSettingsRoutes } from './settings-routes.js';
import { registerUserRoutes } from './user-routes.js';
import { registerExtensionRoutes } from './extensions-routes.js';
import { registerQuickReplyRoutes } from './quick-replies-routes.js';
import { registerResourceRoutes } from './resource-routes.js';
import { registerCharacterRoutes } from './character-routes.js';
import { registerChatRoutes } from './chat-routes.js';
import { registerBackupsRoutes } from './backups-routes.js';
import { registerAiRoutes } from './ai-routes.js';
import { registerProviderRoutes } from './provider-routes.js';
import { registerStatsRoutes } from './stats-routes.js';
import { registerWorldInfoRoutes } from './worldinfo-routes.js';
import { registerContentRoutes } from './content-routes.js';
import { registerAssetsRoutes } from './assets-routes.js';
import { registerSdRoutes } from './sd-routes.js';
import { registerTranslateRoutes } from './translate-routes.js';
import { registerTtsRoutes } from './tts-routes.js';
import { registerVectorRoutes } from './vector-routes.js';

/**
 * Last-resort handler for `/api/...` paths no route claims.
 *
 * Without it `canHandleRequest` declines the request, the patched fetch hands it
 * to the network, and the axum server — which has no `/api` surface — answers
 * `405` with an empty body for POST or `200 text/html` (the SPA shell) for GET.
 * The GET case is the damaging one: `response.ok` is true, so every caller's
 * error check passes and the failure surfaces later as
 * `SyntaxError: Unexpected token '<'` far from its cause.
 *
 * Answering `501` with a JSON body keeps unimplemented endpoints legible and
 * uniform across verbs. Prefix resolution prefers the longest match and
 * method-specific wildcards over `*`, so this never shadows a real route.
 *
 * @param {any} router
 * @param {{ jsonResponse: (data: any, status?: number) => Response }} responses
 */
function registerUnroutedApiFallback(router, { jsonResponse }) {
    router.all('/api/*', ({ method, path }) => {
        const message = `Endpoint is not implemented in RustTavern: ${method} ${path}`;
        console.warn(`RustTavern: ${message}`);
        return jsonResponse({ error: message, endpoint: path, method }, 501);
    });
}

export function registerRoutes(router, context, responses) {
    registerSystemRoutes(router, context, responses);
    registerBootstrapRoutes(router, context, responses);
    registerSettingsRoutes(router, context, responses);
    registerUserRoutes(router, context, responses);
    registerQuickReplyRoutes(router, context, responses);
    registerExtensionRoutes(router, context, responses);
    registerResourceRoutes(router, context, responses);
    registerCharacterRoutes(router, context, responses);
    registerChatRoutes(router, context, responses);
    registerBackupsRoutes(router, context, responses);
    registerContentRoutes(router, context, responses);
    registerAssetsRoutes(router, context, responses);
    registerWorldInfoRoutes(router, context, responses);
    registerAiRoutes(router, context, responses);
    registerVectorRoutes(router, context, responses);
    registerProviderRoutes(router, context, responses);
    registerSdRoutes(router, context, responses);
    registerTranslateRoutes(router, context, responses);
    registerTtsRoutes(router, context, responses);
    registerStatsRoutes(router, context, responses);
    // Keep last: it only ever answers paths no route above claimed.
    registerUnroutedApiFallback(router, responses);
}
