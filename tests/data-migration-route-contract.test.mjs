import assert from 'node:assert/strict';
import test from 'node:test';

import { jsonResponse } from '../src/host/main/http-utils.js';
import { createRouteRegistry } from '../src/host/main/router.js';
import { registerExtensionRoutes } from '../src/host/main/routes/extensions-routes.js';

function createExtensionRouter(context) {
    const router = createRouteRegistry();
    registerExtensionRoutes(router, context, { jsonResponse });
    return router;
}

function completedExportStatus(result = {}) {
    return {
        kind: 'export',
        state: 'completed',
        result: {
            file_name: 'rusttavern-data.zip',
            archive_path: '/tmp/export-job.zip',
            artifact_state: 'available',
            saved_path: null,
            ...result,
        },
    };
}

test('/api/extensions/data-migration/export/save rejects disposed artifacts before native save', async () => {
    const calls = [];
    const router = createExtensionRouter({
        safeInvoke: async (command, args) => {
            calls.push({ command, args });
            if (command === 'get_data_archive_job_status') {
                return completedExportStatus({
                    artifact_state: 'disposed',
                    saved_path: '/Downloads/rusttavern-data.zip',
                });
            }
            throw new Error(`Unexpected command: ${command}`);
        },
    });

    const response = await router.handle({
        method: 'POST',
        path: '/api/extensions/data-migration/export/save',
        body: { job_id: 'job-1' },
    });

    assert.ok(response);
    assert.equal(response.status, 409);
    assert.deepEqual(await response.json(), {
        error: 'Export archive has already been handled',
        saved_target: '/Downloads/rusttavern-data.zip',
    });
    assert.deepEqual(calls, [
        {
            command: 'get_data_archive_job_status',
            args: { job_id: 'job-1' },
        },
    ]);
});
