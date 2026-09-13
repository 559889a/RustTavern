// @ts-check

import {
    normalizeOptionalNonNegativeInteger,
    normalizeSkillImportInput,
    normalizeSkillInstallRequest,
    normalizeSkillMoveRequest,
    normalizeSkillScope,
    normalizeSkillScopeFilter,
    normalizeSkillScopeRetargetRequest,
    requireNonEmptyString,
    requirePlainObject,
    toSkillImportCommandInput,
} from './skill-normalizers.js';

function normalizePickedImportArchivePath(value) {
    if (value === null || value === undefined) {
        return null;
    }

    const path = String(value).trim();
    if (!path) {
        return null;
    }

    return path;
}

/**
 * @param {{
 *   safeInvoke: (command: string, args?: any) => Promise<any>;
 * }} deps
 */
function createSkillApi({
    safeInvoke,
}) {
    /** @type {{ path: string; cleanup: () => Promise<void> } | null} */
    let pendingPickedImport = null;

    function pickedImportPath(input) {
        if (input === null || input === undefined) {
            return null;
        }
        try {
            const normalized = normalizeSkillImportInput(input);
            return normalized.kind === 'archiveFile' ? normalized.path : null;
        } catch {
            return null;
        }
    }

    function rememberPickedImport(input, cleanup) {
        pendingPickedImport = {
            path: input.path,
            cleanup,
        };
        return input;
    }

    async function discardPickedImport(input = null, { throwOnError = true } = {}) {
        if (!pendingPickedImport) {
            return;
        }
        const path = pickedImportPath(input);
        if (input !== null && input !== undefined && path !== pendingPickedImport.path) {
            return;
        }

        const current = pendingPickedImport;
        pendingPickedImport = null;
        try {
            await current.cleanup();
        } catch (error) {
            if (throwOnError) {
                throw error;
            }
            console.warn('Failed to cleanup staged Skill import archive:', error);
        }
    }

    async function list(options = {}) {
        const request = requirePlainObject(options, 'skill list options');
        const scope = normalizeSkillScopeFilter(request.scope ?? request.filter, 'scope');
        return scope ? safeInvoke('list_skills', { scope }) : safeInvoke('list_skills');
    }

    async function listFiles(options) {
        const name = requireNonEmptyString(options?.name, 'skill name');
        const scope = normalizeSkillScope(options?.scope, 'scope');
        return safeInvoke('list_skill_files', {
            name,
            ...(scope ? { scope } : {}),
        });
    }

    async function pickImportArchive() {
        await discardPickedImport();

        const path = normalizePickedImportArchivePath(await safeInvoke('plugin:dialog|open', {
            options: {
                title: 'Import Agent Skill',
                multiple: false,
                directory: false,
                filters: [
                    {
                        name: 'Agent Skill Archive',
                        extensions: ['zip', 'ttskill'],
                    },
                ],
            },
        }));

        return path ? { kind: 'archiveFile', path } : null;
    }

    async function downloadImport(options) {
        const request = requirePlainObject(options, 'skill import download request');
        const url = requireNonEmptyString(request.url, 'skill import URL');
        return normalizeSkillImportInput(await safeInvoke('download_skill_import_url', { url }));
    }

    async function previewImport(options) {
        const request = requirePlainObject(options, 'skill import preview request');
        const input = normalizeSkillImportInput(request.input);
        const targetScope = normalizeSkillScope(request.targetScope ?? request.target_scope, 'targetScope');
        try {
            return await safeInvoke('preview_skill_import', {
                input: toSkillImportCommandInput(input),
                ...(targetScope ? { targetScope } : {}),
            });
        } catch (error) {
            await discardPickedImport(request.input, { throwOnError: false });
            throw error;
        }
    }

    async function installImport(request) {
        try {
            return await safeInvoke('install_skill_import', {
                request: normalizeSkillInstallRequest(request),
            });
        } finally {
            await discardPickedImport(request?.input, { throwOnError: false });
        }
    }

    async function readFile(options) {
        const name = requireNonEmptyString(options?.name, 'skill name');
        const path = requireNonEmptyString(options?.path, 'skill file path');
        const maxChars = normalizeOptionalNonNegativeInteger(options?.maxChars, 'maxChars');
        const startLine = normalizeOptionalNonNegativeInteger(options?.startLine, 'startLine');
        const lineCount = normalizeOptionalNonNegativeInteger(options?.lineCount, 'lineCount');
        const startChar = normalizeOptionalNonNegativeInteger(options?.startChar, 'startChar');
        const scope = normalizeSkillScope(options?.scope, 'scope');
        return safeInvoke('read_skill_file', {
            name,
            path,
            ...(scope ? { scope } : {}),
            maxChars,
            startLine,
            lineCount,
            startChar,
        });
    }

    async function writeFile(options) {
        const name = requireNonEmptyString(options?.name, 'skill name');
        const path = requireNonEmptyString(options?.path, 'skill file path');
        if (typeof options?.content !== 'string') {
            throw new Error('skill file content must be a string');
        }
        const scope = normalizeSkillScope(options?.scope, 'scope');
        const expectedSha256 = String(options?.expectedSha256 ?? options?.expected_sha256 ?? '').trim();
        return safeInvoke('write_skill_file', {
            name,
            path,
            content: options.content,
            ...(scope ? { scope } : {}),
            ...(expectedSha256 ? { expectedSha256 } : {}),
        });
    }

    async function exportSkill(options) {
        const name = requireNonEmptyString(options?.name, 'skill name');
        const scope = normalizeSkillScope(options?.scope, 'scope');
        return safeInvoke('export_skill', {
            name,
            ...(scope ? { scope } : {}),
        });
    }

    async function deleteSkill(options) {
        const name = requireNonEmptyString(options?.name, 'skill name');
        const scope = normalizeSkillScope(options?.scope, 'scope');
        return safeInvoke('delete_skill', {
            name,
            ...(scope ? { scope } : {}),
        });
    }

    async function move(request) {
        return safeInvoke('move_skill', {
            request: normalizeSkillMoveRequest(request),
        });
    }

    async function retargetScope(request) {
        return safeInvoke('retarget_skill_scope', {
            request: normalizeSkillScopeRetargetRequest(request),
        });
    }

    return {
        list,
        listFiles,
        pickImportArchive,
        discardPickedImport,
        downloadImport,
        previewImport,
        installImport,
        readFile,
        writeFile,
        export: exportSkill,
        delete: deleteSkill,
        move,
        retargetScope,
    };
}

/**
 * @param {any} context
 */
export function installSkillApi(context) {
    const hostWindow = /** @type {any} */ (window);
    const hostAbi = hostWindow.__RUSTTAVERN__;
    if (!hostAbi || typeof hostAbi !== 'object') {
        throw new Error('Host ABI __RUSTTAVERN__ is missing');
    }

    const safeInvoke = context?.safeInvoke;
    if (typeof safeInvoke !== 'function') {
        throw new Error('Tauri main context safeInvoke is missing');
    }

    if (!hostAbi.api || typeof hostAbi.api !== 'object') {
        hostAbi.api = {};
    }

    hostAbi.api.skill = createSkillApi({
        safeInvoke,
    });
}
