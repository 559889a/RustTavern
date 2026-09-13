// @ts-check

import { eventSource, event_types } from '../../../../scripts/events.js';
import { getRustTavernSettings } from '../../../../host-bridge.js';
import { activateApiConnectionsSubtreeGates } from '../../adapters/panel-runtime/api-connections-subtree-gates.js';
import { installExtensionsSubtreeGates } from '../../adapters/panel-runtime/extensions-subtree-gates.js';
import { installTopSettingsPanelParking } from '../../adapters/panel-runtime/top-settings-panel-parking.js';
import { createPanelRuntimeService } from './panel-runtime-service.js';
import { validatePanelRuntimeInvariants } from './validate.js';

// Keep in sync with:
// - src/host/main/services/panel-runtime/preinstall.js
// - src/scripts/app/setting/setting-panel.js
const PANEL_RUNTIME_PROFILE_STORAGE_KEY = 'tt:panelRuntimeProfile';

export function installPanelRuntime() {
    const ready = getRustTavernSettings().then((settings) => {
        const profileName = String(settings.panel_runtime_profile || 'off').trim();
        localStorage.setItem(PANEL_RUNTIME_PROFILE_STORAGE_KEY, profileName);

        if (profileName === 'off') {
            return null;
        }

        const service = createPanelRuntimeService({ profileName });

        eventSource.on(event_types.APP_READY, () => {
            activateApiConnectionsSubtreeGates({ manager: service.manager });
            installTopSettingsPanelParking({ manager: service.manager });
            installExtensionsSubtreeGates({ manager: service.manager });
            validatePanelRuntimeInvariants({ profileName: service.manager.profile });
        });

        return service;
    });

    return { ready };
}
