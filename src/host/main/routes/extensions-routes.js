function parseJobId(value) {
    const jobId = String(value || '').trim();
    return jobId || '';
}

function errorMessage(error) {
    return error instanceof Error ? error.message : String(error || '');
}

export function registerExtensionRoutes(router, context, { jsonResponse }) {
    async function startImportJobFromFileInfo(fileInfo) {
        if (!fileInfo?.filePath) {
            const reason = fileInfo?.error ? `: ${fileInfo.error}` : '';
            return jsonResponse({ error: `Unable to access uploaded archive${reason}` }, 400);
        }

        try {
            const jobId = parseJobId(await context.safeInvoke('start_import_data_archive', {
                archive_path: fileInfo.filePath,
                archive_is_temporary: Boolean(fileInfo.isTemporary),
            }));
            if (!jobId) {
                return jsonResponse({ error: 'Import job id is missing' }, 500);
            }

            return jsonResponse({
                ok: true,
                job_id: jobId,
            });
        } finally {
            await fileInfo.cleanup?.();
        }
    }

    async function loadCompletedExportJobStatus(jobId) {
        const status = await context.safeInvoke('get_data_archive_job_status', {
            job_id: jobId,
        });

        if (status.kind !== 'export') {
            return {
                error: jsonResponse({ error: 'Invalid export job' }, 400),
                status: null,
            };
        }

        if (status.state !== 'completed') {
            return {
                error: jsonResponse({ error: 'Export job is not completed yet' }, 409),
                status: null,
            };
        }

        const artifactState = String(status?.result?.artifact_state || 'available');
        if (artifactState === 'disposed') {
            return {
                error: jsonResponse({
                    error: 'Export archive has already been handled',
                    saved_target: String(status?.result?.saved_path || ''),
                }, 409),
                status: null,
            };
        }
        if (artifactState === 'missing') {
            return {
                error: jsonResponse({ error: 'Export archive is missing' }, 410),
                status: null,
            };
        }
        if (artifactState !== 'available') {
            return {
                error: jsonResponse({ error: `Invalid export artifact state: ${artifactState}` }, 500),
                status: null,
            };
        }

        return {
            error: null,
            status,
        };
    }

    async function finalizeExportDelivery(jobId, savedTarget = '') {
        try {
            const finalizationError = await context.safeInvoke('finalize_export_data_archive_delivery', {
                job_id: jobId,
                saved_path: savedTarget || null,
            });
            return finalizationError ? String(finalizationError) : null;
        } catch (error) {
            return errorMessage(error);
        }
    }

    router.all('/api/extensions/discover', async () => {
        const extensions = await context.safeInvoke('get_extensions');
        const mapped = extensions.map((extension) => ({
            name: extension.name,
            type: String(extension.extension_type || 'local').toLowerCase(),
        }));

        return jsonResponse(mapped);
    });

    router.post('/api/extensions/install', async ({ body }) => {
        const result = await context.safeInvoke('install_extension', {
            url: body?.url || '',
            global: Boolean(body?.global),
            branch: typeof body?.branch === 'string' && body.branch.trim() ? body.branch.trim() : null,
        });

        return jsonResponse({
            display_name: result?.display_name || body?.url || 'Extension',
            author: result?.author || 'Unknown',
            version: result?.version || '0.0.0',
            extensionPath: result?.extension_path || '',
            folderName: result?.folder_name || '',
        });
    });

    router.post('/api/extensions/update', async ({ body }) => {
        const result = await context.safeInvoke('update_extension', {
            extensionName: body?.extensionName || '',
            global: Boolean(body?.global),
        });

        return jsonResponse({
            isUpToDate: Boolean(result?.is_up_to_date),
            shortCommitHash: result?.short_commit_hash || 'unknown',
        });
    });

    router.post('/api/extensions/delete', async ({ body }) => {
        await context.safeInvoke('delete_extension', {
            extensionName: body?.extensionName || '',
            global: Boolean(body?.global),
        });

        return jsonResponse({ ok: true });
    });

    router.post('/api/extensions/version', async ({ body }) => {
        const result = await context.safeInvoke('get_extension_version', {
            extensionName: body?.extensionName || '',
            global: Boolean(body?.global),
        });

        return jsonResponse({
            currentBranchName: result?.current_branch_name ?? '',
            currentCommitHash: result?.current_commit_hash ?? '',
            isUpToDate: Boolean(result?.is_up_to_date),
            remoteUrl: result?.remote_url ?? '',
        });
    });

    router.post('/api/extensions/move', async ({ body }) => {
        await context.safeInvoke('move_extension', {
            extensionName: body?.extensionName || '',
            source: body?.source || 'local',
            destination: body?.destination || 'global',
        });

        return jsonResponse({ ok: true });
    });

    router.post('/api/extensions/data-migration/import', async ({ body }) => {
        if (!(body instanceof FormData)) {
            return jsonResponse({ error: 'Expected multipart form data' }, 400);
        }

        const archive = body.get('archive');
        if (!(archive instanceof Blob)) {
            return jsonResponse({ error: 'No data archive provided' }, 400);
        }

        const preferredName = archive instanceof File && archive.name
            ? archive.name
            : 'data.archive';
        const fileInfo = await context.materializeUploadFile(archive, {
            kind: 'data-archive',
            preferredName,
        });

        return startImportJobFromFileInfo(fileInfo);
    });

    router.post('/api/extensions/data-migration/export', async () => {
        const jobId = parseJobId(await context.safeInvoke('start_export_data_archive'));
        if (!jobId) {
            return jsonResponse({ error: 'Export job id is missing' }, 500);
        }
        return jsonResponse({
            ok: true,
            job_id: jobId,
        });
    });

    router.post('/api/extensions/data-migration/export/save', async ({ body }) => {
        const jobId = parseJobId(body?.job_id);
        if (!jobId) {
            return jsonResponse({ error: 'Missing job id' }, 400);
        }

        const { error } = await loadCompletedExportJobStatus(jobId);
        if (error) {
            return error;
        }

        const savedTarget = await context.safeInvoke('save_export_data_archive', {
            job_id: jobId,
        });

        return jsonResponse({
            ok: true,
            saved_target: String(savedTarget || ''),
        });
    });

    router.get('/api/extensions/data-migration/job', async ({ url }) => {
        const jobId = parseJobId(url?.searchParams?.get('id'));
        if (!jobId) {
            return jsonResponse({ error: 'Missing job id' }, 400);
        }

        const status = await context.safeInvoke('get_data_archive_job_status', {
            job_id: jobId,
        });
        return jsonResponse(status || {});
    });

    router.post('/api/extensions/data-migration/job/cancel', async ({ body }) => {
        const jobId = parseJobId(body?.job_id);
        if (!jobId) {
            return jsonResponse({ error: 'Missing job id' }, 400);
        }

        await context.safeInvoke('cancel_data_archive_job', {
            job_id: jobId,
        });

        return jsonResponse({ ok: true });
    });

    router.post('/api/extensions/data-migration/export/cleanup', async ({ body }) => {
        const jobId = parseJobId(body?.job_id);
        if (!jobId) {
            return jsonResponse({ error: 'Missing job id' }, 400);
        }

        await context.safeInvoke('cleanup_export_data_archive', {
            job_id: jobId,
        });

        return jsonResponse({ ok: true });
    });

    router.post('/api/extensions/branches', async ({ body }) => {
        const result = await context.safeInvoke('get_extension_branches', {
            extensionName: body?.extensionName || '',
            global: Boolean(body?.global),
        });
        return jsonResponse(result);
    });

    router.post('/api/extensions/switch', async ({ body }) => {
        await context.safeInvoke('switch_extension_branch', {
            extensionName: body?.extensionName || '',
            branch: body?.branch || '',
            global: Boolean(body?.global),
        });
        return new Response(null, { status: 204 });
    });
}
