declare module 'droll';
declare module '@iconfu/svg-inject';

// Global variables
interface Window {
    // Tauri globals
    __TAURI__?: any;
    __TAURI_INTERNALS__?: any;
    __TAURI_RUNNING__?: boolean;

    __RUSTTAVERN_MAIN_READY__?: Promise<void>;

    // RustTavern host contract (public globals)
    __RUSTTAVERN__?: RustTavernHostAbi;

    // SillyTavern ecosystem library shim ABI
    _?: any;

    __RUSTTAVERN_THUMBNAIL__?: (type: string, file: string, useTimestamp?: boolean) => string;
    __RUSTTAVERN_BACKGROUND_PATH__?: (file: string) => string;

    __RUSTTAVERN_IMPORT_ARCHIVE_PICKER__?: {
        onNativeResult: (payload: any) => void;
    };
    __RUSTTAVERN_EXPORT_ARCHIVE_PICKER__?: {
        onNativeResult: (payload: any) => void;
    };

    __RUSTTAVERN_HANDLE_BACK__?: () => boolean;
    __RUSTTAVERN_NATIVE_SHARE__?: {
        push: (payload: any) => boolean;
        subscribe: (handler: (payload: any) => void) => () => void;
    };
    __RUSTTAVERN_MOBILE_RUNTIME_COMPAT__?: boolean;
    __RUSTTAVERN_MOBILE_OVERLAY_COMPAT__?: {
        dispose: () => void;
        revalidate: () => void;
    };
    __RUSTTAVERN_MOBILE_WINDOW_OPEN_COMPAT__?: boolean;

    __RUSTTAVERN_EMBEDDED_RUNTIME__?: {
        profile: string;
        register: (slot: any) => { id: string; unregister: () => void };
        unregister: (id: string) => void;
        reconcile: () => void;
        getPerfSnapshot: () => any;
    };
}

type RustTavernHostInvokeApi = {
    safeInvoke: (command: any, args?: any) => Promise<any>;
    invalidate: (command: any, args?: any) => void;
    invalidateAll: (command: any) => void;
    flush: (command: any) => Promise<void>;
    flushAll: () => Promise<void>;
    broker: any;
};

type RustTavernHostAssetsApi = {
    thumbnailUrl: (type: string, file: string, useTimestamp?: boolean) => string;
    backgroundPath: (file: string) => string;
};

type RustTavernChatApi = {
    open: (ref: RustTavernChatRef) => RustTavernChatHandle;
    current: {
        ref: () => RustTavernChatRef;
        handle: () => RustTavernChatHandle;
        windowInfo: () => Promise<RustTavernChatWindowInfo>;
    };
};

type RustTavernAgentRunStatus =
    | 'created'
    | 'initializing_workspace'
    | 'assembling_context'
    | 'calling_model'
    | 'dispatching_tool'
    | 'applying_workspace_patch'
    | 'creating_checkpoint'
    | 'awaiting_host_commit'
    | 'finishing'
    | 'completed'
    | 'partial_success'
    | 'cancelling'
    | 'cancelled'
    | 'failed';

type RustTavernAgentRunPresentation = 'foreground' | 'background';

type RustTavernAgentRunEvent = {
    seq: number;
    id: string;
    runId: string;
    timestamp: string;
    level: 'debug' | 'info' | 'warn' | 'error';
    type: string;
    payload?: any;
};

type RustTavernAgentInvocationKind = 'root' | 'subagent' | 'handoff';

type RustTavernAgentInvocationStatus =
    | 'created'
    | 'running'
    | 'completed'
    | 'failed'
    | 'cancelled'
    | 'transferred';

type RustTavernAgentInvocationExitPolicy = 'run_finish_allowed' | 'task_return_required';

type RustTavernAgentDelegationContinuation = 'return_to_parent' | 'transfer_control';

type RustTavernAgentTaskStatus = 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';

type RustTavernAgentRunTimelineInvocation = {
    invocationId: string;
    parentInvocationId?: string;
    profileId: string;
    kind: RustTavernAgentInvocationKind;
    status: RustTavernAgentInvocationStatus;
    exitPolicy: RustTavernAgentInvocationExitPolicy;
    createdAt: string;
    updatedAt: string;
};

type RustTavernAgentRunTimelineDelegationEdge = {
    taskId: string;
    sourceInvocationId: string;
    targetInvocationId: string;
    targetProfileId: string;
    workspaceKey: string;
    continuation: RustTavernAgentDelegationContinuation;
    status: RustTavernAgentTaskStatus;
    resultRef?: string;
    error?: string;
    createdAt: string;
    updatedAt: string;
};

type RustTavernAgentRunTimelineProjection = {
    foregroundInvocationIds: string[];
    invocations: RustTavernAgentRunTimelineInvocation[];
    delegationEdges: RustTavernAgentRunTimelineDelegationEdge[];
};

type RustTavernAgentRunHandle = {
    runId: string;
    workspaceId: string;
    stableChatId: string;
    generationType: string;
    status: RustTavernAgentRunStatus;
};

type RustTavernAgentGuidanceResult = {
    runId: string;
    guidanceId: string;
    clientGuidanceId?: string;
    status: 'queued';
    preview: string;
    chars: number;
    words: number;
    pendingCount: number;
};

type RustTavernAgentRunListCursor = {
    createdAt: string;
    runId: string;
};

type RustTavernAgentRunSummary = {
    runId: string;
    workspaceId: string;
    stableChatId: string;
    chatRef: RustTavernChatRef;
    generationType: string;
    profileId?: string;
    skillScopeRefs?: {
        preset?: RustTavernAgentPresetRef;
        characterId?: string;
    };
    persistBaseStateId?: string;
    inputMessageCount?: number;
    presentation: RustTavernAgentRunPresentation;
    status: RustTavernAgentRunStatus;
    createdAt: string;
    updatedAt: string;
    commitCount: number;
    committedMessage?: {
        commitId: string;
        messageId: string;
        messageIndex?: number;
        committedAt: string;
    };
    terminalAt?: string;
};

type RustTavernAgentRunPruneRetention = {
    keepRecentTerminalRuns: number;
    keepFullRecentRuns: number;
};

type RustTavernAgentRunRetentionSettings = RustTavernAgentRunPruneRetention & {
    autoPruneEnabled: boolean;
};

type RustTavernAgentRunPruneAction = 'slim_heavy_artifacts' | 'delete_run';
type RustTavernAgentRunPruneReason = 'outside_full_retention_window' | 'outside_history_retention_window';
type RustTavernAgentRunPruneBlockReason = 'active_run' | 'missing_terminal_event' | 'invalid_journal' | 'invalid_storage';

type RustTavernAgentRunPruneCandidate = {
    runId: string;
    workspaceId: string;
    stableChatId: string;
    chatRef: RustTavernChatRef;
    status: RustTavernAgentRunStatus;
    createdAt: string;
    updatedAt: string;
    action: RustTavernAgentRunPruneAction;
    reason: RustTavernAgentRunPruneReason;
    fileCount: number;
    byteCount: number;
};

type RustTavernAgentRunPruneBlockedRun = RustTavernAgentRunPruneCandidate & {
    blockReason: RustTavernAgentRunPruneBlockReason;
    message?: string;
};

type RustTavernAgentRunPruneFailedRun = RustTavernAgentRunPruneCandidate & {
    message: string;
};

type RustTavernAgentRunPrunePlan = {
    retention: RustTavernAgentRunPruneRetention;
    detailLimit: number;
    terminalRunCount: number;
    nonTerminalRunCount: number;
    blockedRunCount: number;
    fullRetainedRunCount: number;
    coreRetainedRunCount: number;
    slimCandidateCount: number;
    deleteCandidateCount: number;
    totalSlimFileCount: number;
    totalSlimByteCount: number;
    totalDeleteFileCount: number;
    totalDeleteByteCount: number;
    totalCandidateFileCount: number;
    totalCandidateByteCount: number;
    candidateDetailsTruncated: boolean;
    candidates: RustTavernAgentRunPruneCandidate[];
    blockedDetailsTruncated: boolean;
    blockedRuns: RustTavernAgentRunPruneBlockedRun[];
};

type RustTavernAgentRunPruneApplyResult = {
    retention: RustTavernAgentRunPruneRetention;
    detailLimit: number;
    slimmedRunCount: number;
    deletedRunCount: number;
    failedRunCount: number;
    removedFileCount: number;
    removedByteCount: number;
    failedDetailsTruncated: boolean;
    failedRuns: RustTavernAgentRunPruneFailedRun[];
    afterPlan: RustTavernAgentRunPrunePlan;
};

type RustTavernAgentModelTurn = {
    runId: string;
    round: number;
    modelResponsePath: string;
    provider: {
        source?: string;
        format?: string;
        model?: string;
        responseId?: string;
        usage?: any;
    };
    assistant: {
        text: string;
        totalChars: number;
        totalWords: number;
        truncated: boolean;
    };
    narration?: {
        source: 'assistantText';
        text: string;
        totalChars: number;
        totalWords: number;
        truncated: boolean;
    } | null;
    reasoning: Array<{
        source: string;
        text: string;
        totalChars: number;
        totalWords: number;
        truncated: boolean;
    }>;
    toolCalls: Array<{
        callId: string;
        toolId: string;
        name: string;
        modelAlias?: string;
    }>;
};

type RustTavernAgentProfileSummary = {
    id: string;
    displayName: string;
    description?: string;
    directRunnable: boolean;
};

type RustTavernAgentToolCatalogItem = {
    name: string;
    title: string;
    description: string;
    inputSchema: any;
    outputSchema?: any;
    annotations?: any;
    source: string;
};

type RustTavernAgentProfileDefinition = {
    schemaVersion: number;
    kind: 'tauritavern.agentProfile';
    id: string;
    displayName: string;
    description?: string;
    preset: {
        mode: 'currentPromptSnapshot' | 'ref' | 'none';
        ref?: {
            apiId: string;
            name: string;
        };
        required?: boolean;
    };
    model: {
        mode: 'currentPromptSnapshot' | 'connectionRef' | 'requiresConfiguration';
        connectionRef?: string;
        modelId?: string;
    };
    run: {
        presentation: RustTavernAgentRunPresentation;
        directRunnable: boolean;
        modelRetry: {
            maxRetries: number;
            intervalMs: number;
        };
    };
    instructions: {
        agentSystemPrompt?: string | null;
    };
    tools: {
        allow: string[];
        deny?: string[];
        toolDescriptions?: Record<string, {
            description?: string;
            properties?: Record<string, string>;
        }>;
        maxRounds: number;
        maxCallsPerRun: number;
        maxCallsPerTool?: Record<string, number>;
    };
    skills: {
        visible: string[];
        deny?: string[];
        maxReadCharsPerCall: number;
        maxReadCharsPerRun: number;
    };
    workspace: {
        visibleRoots: string[];
        writableRoots: string[];
    };
    plan: {
        mode: 'none' | 'free' | 'strict' | 'hybrid';
        beta?: boolean;
        nodes?: Array<{
            id: string;
            title: string;
            locked: boolean;
        }>;
    };
    output: {
        artifacts: Array<{
            id: string;
            path: string;
            kind: string;
            target: 'messageBody';
            required?: boolean;
            assemblyOrder?: number;
        }>;
    };
};

type RustTavernAgentPresetRef = {
    apiId: string;
    name: string;
};

type RustTavernAgentProfileStorageIssue = {
    profileId: string;
    fileName: string;
    kind: 'invalidJson' | 'invalidFileIdentity' | 'invalidProfile';
    recommendedAction?: 'delete' | 'normalizeIdentity';
    message: string;
};

type RustTavernAgentProfileDiagnostic = {
    code: string;
    severity: 'error';
    path: string;
    message: string;
    resource?: {
        kind: 'preset' | 'llmConnection' | 'model';
        apiId?: string;
        name?: string;
        id?: string;
        modelId?: string;
    };
    blocks?: Array<'preview' | 'promptAssembly' | 'directRun' | 'subAgent'>;
    repairActions?: Array<'selectPreset' | 'selectModel' | 'setModelRequiresConfiguration' | 'openJsonEditor'>;
};

type RustTavernAgentProfileHealth = {
    profileId: string;
    previewAvailable: boolean;
    promptAssemblyAvailable: boolean;
    directRunAvailable: boolean;
    subAgentAvailable: boolean;
    diagnostics: RustTavernAgentProfileDiagnostic[];
};

type RustTavernAgentProfilesApi = {
    list: () => Promise<{
        profiles: RustTavernAgentProfileSummary[];
        issues: RustTavernAgentProfileStorageIssue[];
    }>;
    load: (input: string | { profileId: string }) => Promise<{ profile: RustTavernAgentProfileDefinition | null }>;
    diagnose: (input: string | { profileId: string }) => Promise<RustTavernAgentProfileHealth>;
    resolveSystemPrompt: (input?: string | { profileId?: string | null }) => Promise<{ agentSystemPrompt: string }>;
    repairFile: (input: { profileId: string; action: 'delete' | 'normalizeIdentity' }) => Promise<void>;
    retargetPresetRefs: (input: {
        from: RustTavernAgentPresetRef;
        to: RustTavernAgentPresetRef;
    }) => Promise<{ updated: number; profileIds: string[] }>;
    save: (input: RustTavernAgentProfileDefinition | { profile: RustTavernAgentProfileDefinition }) => Promise<void>;
    delete: (input: string | { profileId: string }) => Promise<void>;
};

type RustTavernAgentToolsApi = {
    list: () => Promise<{ tools: RustTavernAgentToolCatalogItem[] }>;
};

type RustTavernAgentPromptAssemblyApi = {
    prepare: (input: {
        profileId?: string | null;
        generationType?: string;
        frozenRunInputSnapshot: Record<string, any>;
        jsonSchema?: any;
    }) => Promise<{
        mode: 'currentPromptSnapshot' | 'frontendPromptAssembly';
        request?: any;
        assembly?: any;
    }>;
    buildSnapshot: (input: {
        generationType?: string;
        frozenRunInputSnapshot: Record<string, any>;
        settings?: Record<string, any>;
        presetSettings?: Record<string, any>;
        modelId?: string | null;
        profileId?: string | null;
        agentContextPolicy?: Record<string, any>;
        contextPolicy?: Record<string, any>;
        agentSystemPrompt?: string | null;
        agentTaskPrompt?: string | null;
        requiredAgentPromptComponents?: string[];
        jsonSchema?: any;
    }) => Promise<{
        promptSnapshot: any;
        frozenRunInputSnapshot: any;
        generationIntent: any;
        assembly: any;
    }>;
    buildCurrentModelConnectionSnapshot: (input: {
        settings: Record<string, any>;
        model: string;
        secretId?: string | null;
    }) => Promise<Record<string, any>>;
    applyCurrentModelConnectionSnapshot: (input: {
        settings: Record<string, any>;
        currentModelConnection: Record<string, any>;
    }) => Promise<Record<string, any>>;
};

type RustTavernAgentRetentionApi = {
    readSettings: () => Promise<RustTavernAgentRunRetentionSettings>;
    updateSettings: (input: Partial<RustTavernAgentRunRetentionSettings>) => Promise<RustTavernAgentRunRetentionSettings>;
    planPrune: (input?: {
        retention?: RustTavernAgentRunPruneRetention | RustTavernAgentRunRetentionSettings;
        detailLimit?: number;
    }) => Promise<RustTavernAgentRunPrunePlan>;
    applyPrune: (input?: {
        retention?: RustTavernAgentRunPruneRetention | RustTavernAgentRunRetentionSettings;
        detailLimit?: number;
    }) => Promise<RustTavernAgentRunPruneApplyResult>;
};

type RustTavernAgentApi = {
    startRunWithPromptSnapshot: (input: {
        chatRef: RustTavernChatRef;
        stableChatId?: string;
        generationType?: string;
        profileId?: string | null;
        promptSnapshot: any;
        frozenRunInputSnapshot?: any;
        generationIntent?: any;
        presentation?: RustTavernAgentRunPresentation;
        options?: { presentation?: RustTavernAgentRunPresentation; stream?: boolean };
    }) => Promise<RustTavernAgentRunHandle>;
    startRunFromLegacyGenerate: (input?: {
        chatRef?: RustTavernChatRef;
        stableChatId?: string;
        generationType?: string;
        generateOptions?: Record<string, any>;
        profileId?: string | null;
        generationIntent?: any;
        presentation?: RustTavernAgentRunPresentation;
        options?: { presentation?: RustTavernAgentRunPresentation; stream?: false };
    }) => Promise<RustTavernAgentRunHandle>;
    cancel: (runId: string) => Promise<RustTavernAgentRunHandle>;
    submitGuidance: (input: {
        runId: string;
        text: string;
        clientGuidanceId?: string;
    }) => Promise<RustTavernAgentGuidanceResult>;
    readEvents: (input: {
        runId: string;
        afterSeq?: number;
        beforeSeq?: number;
        limit?: number;
        invocationId?: string;
        includeTimelineProjection?: boolean;
    }) => Promise<{
        events: RustTavernAgentRunEvent[];
        timelineProjection?: RustTavernAgentRunTimelineProjection;
    }>;
    readWorkspaceFile: (input: {
        runId: string;
        path: string;
    }) => Promise<{ path: string; text: string; chars: number; words: number; sha256: string }>;
    readModelTurn: (input: {
        runId: string;
        invocationId?: string;
        round: number;
        maxChars?: number;
    }) => Promise<RustTavernAgentModelTurn>;
    subscribe: (
        runId: string,
        handler: (event: RustTavernAgentRunEvent) => void,
        options?: { afterSeq?: number; limit?: number; intervalMs?: number; onError?: (error: unknown) => void },
    ) => RustTavernHostUnsubscribe;
    profiles: RustTavernAgentProfilesApi;
    tools: RustTavernAgentToolsApi;
    promptAssembly: RustTavernAgentPromptAssemblyApi;
    retention: RustTavernAgentRetentionApi;
    approveToolCall: () => never;
    listRuns: (input?: {
        chatRef?: RustTavernChatRef;
        stableChatId?: string;
        statuses?: RustTavernAgentRunStatus[];
        before?: RustTavernAgentRunListCursor;
        limit?: number;
    }) => Promise<{
        runs: RustTavernAgentRunSummary[];
        nextCursor?: RustTavernAgentRunListCursor;
    }>;
    readDiff: () => never;
    rollback: () => never;
};

type RustTavernLlmConnectionSummary = {
    id: string;
    displayName: string;
    description?: string;
    chatCompletionSource: string;
    customApiFormat?: string;
};

type RustTavernLlmConnectionDefinition = {
    schemaVersion: number;
    kind: 'tauritavern.llmConnection';
    id: string;
    displayName: string;
    description?: string;
    provider: {
        chatCompletionSource: string;
        customApiFormat?: string;
    };
    endpoint?: {
        baseUrl?: string;
        sourceSpecific?: Record<string, any>;
    };
    auth: {
        secretRef: {
            key: string;
            id: string;
            labelSnapshot?: string;
        };
    };
    routing?: {
        reverseProxy?: {
            url: string;
        };
    };
    adapterHints?: {
        promptPostProcessing?: string;
        customIncludeHeaders?: string;
        customIncludeBody?: string;
        customExcludeBody?: string;
    };
    capabilities?: {
        streaming?: string;
        toolCalling?: string;
    };
};

type RustTavernLlmConnectionsApi = {
    list: () => Promise<{ connections: RustTavernLlmConnectionSummary[] }>;
    load: (input: string | { connectionId: string } | { connection_id: string }) => Promise<{
        connection: RustTavernLlmConnectionDefinition | null;
    }>;
    save: (input: RustTavernLlmConnectionDefinition | { connection: RustTavernLlmConnectionDefinition }) => Promise<void>;
    delete: (input: string | { connectionId: string } | { connection_id: string }) => Promise<void>;
};

type RustTavernSkillFileKind = 'text' | 'binary';

type RustTavernSkillImportConflictKind = 'new' | 'same' | 'different';

type RustTavernSkillInstallConflictStrategy = 'skip' | 'replace';

type RustTavernSkillInstallAction = 'installed' | 'replaced' | 'already_installed' | 'skipped';

type RustTavernSkillScope =
    | { kind: 'global' }
    | { kind: 'preset'; apiId: string; name: string }
    | { kind: 'profile'; profileId: string }
    | { kind: 'character'; characterId: string };

type RustTavernSkillScopeFilter =
    | { kind: 'all' }
    | RustTavernSkillScope;

type RustTavernSkillIndexEntry = {
    scope: RustTavernSkillScope;
    name: string;
    description: string;
    displayName?: string;
    sourceKind?: string;
    license?: string;
    author?: string;
    version?: string;
    tags: string[];
    installedHash: string;
    fileCount: number;
    totalBytes: number;
    hasScripts: boolean;
    hasBinary: boolean;
    installedAt: string;
    sourceRefs?: RustTavernSkillSourceRef[];
};

type RustTavernSkillSourceRef = {
    kind: string;
    id: string;
    label: string;
    installedHash: string;
};

type RustTavernSkillInlineFile = {
    path: string;
    encoding?: 'utf8' | 'utf-8' | 'base64';
    content: string;
    mediaType?: string;
    sizeBytes?: number;
    sha256?: string;
};

type RustTavernSkillImportInput =
    | {
        kind: 'inlineFiles';
        files: RustTavernSkillInlineFile[];
        source?: any;
    }
    | {
        kind: 'directory';
        path: string;
        source?: any;
    }
    | {
        kind: 'archiveFile';
        path: string;
        source?: any;
    }
    | {
        kind: 'archiveBase64';
        fileName: string;
        contentBase64: string;
        sha256?: string;
        source?: any;
    };

type RustTavernSkillFileRef = {
    path: string;
    kind: RustTavernSkillFileKind;
    mediaType: string;
    sizeBytes: number;
    sha256: string;
};

type RustTavernSkillImportPreview = {
    skill: RustTavernSkillIndexEntry;
    files: RustTavernSkillFileRef[];
    conflict: {
        kind: RustTavernSkillImportConflictKind;
        installedHash?: string;
    };
    warnings: string[];
    source: any;
};

type RustTavernSkillInstallResult = {
    scope: RustTavernSkillScope;
    name: string;
    action: RustTavernSkillInstallAction;
    skill?: RustTavernSkillIndexEntry;
};

type RustTavernSkillReadResult = {
    name: string;
    path: string;
    content: string;
    chars: number;
    words: number;
    totalChars: number;
    totalWords: number;
    startChar: number;
    endChar: number;
    totalLines: number;
    startLine: number;
    endLine: number;
    bytes: number;
    sha256: string;
    truncated: boolean;
    resourceRef: string;
};

type RustTavernSkillExportPayload = {
    fileName: string;
    contentBase64: string;
    sha256: string;
};

type RustTavernSkillApi = {
    list: (options?: { scope?: RustTavernSkillScopeFilter; filter?: RustTavernSkillScopeFilter }) => Promise<RustTavernSkillIndexEntry[]>;
    listFiles: (options: { scope?: RustTavernSkillScope; name: string }) => Promise<RustTavernSkillFileRef[]>;
    pickImportArchive: () => Promise<RustTavernSkillImportInput | null>;
    discardPickedImport: (input?: RustTavernSkillImportInput | null) => Promise<void>;
    downloadImport: (options: { url: string }) => Promise<RustTavernSkillImportInput>;
    previewImport: (options: {
        input: RustTavernSkillImportInput;
        targetScope?: RustTavernSkillScope;
    }) => Promise<RustTavernSkillImportPreview>;
    installImport: (request: {
        input: RustTavernSkillImportInput;
        targetScope?: RustTavernSkillScope;
        conflictStrategy?: RustTavernSkillInstallConflictStrategy;
    }) => Promise<RustTavernSkillInstallResult>;
    readFile: (options: {
        scope?: RustTavernSkillScope;
        name: string;
        path: string;
        maxChars?: number;
        startLine?: number;
        lineCount?: number;
        startChar?: number;
    }) => Promise<RustTavernSkillReadResult>;
    writeFile: (options: {
        scope?: RustTavernSkillScope;
        name: string;
        path: string;
        content: string;
        expectedSha256?: string;
    }) => Promise<RustTavernSkillReadResult>;
    export: (options: { scope?: RustTavernSkillScope; name: string }) => Promise<RustTavernSkillExportPayload>;
    delete: (options: { scope?: RustTavernSkillScope; name: string }) => Promise<void>;
    move: (request: {
        name: string;
        fromScope: RustTavernSkillScope;
        toScope: RustTavernSkillScope;
        conflictStrategy?: RustTavernSkillInstallConflictStrategy;
    }) => Promise<RustTavernSkillInstallResult>;
    retargetScope: (request: {
        fromScope: RustTavernSkillScope;
        toScope: RustTavernSkillScope;
    }) => Promise<any>;
};

type RustTavernFrontendLogsApi = {
    list: (options?: { limit?: number }) => Promise<RustTavernFrontendLogEntry[]>;
    subscribe: (
        handler: (entry: RustTavernFrontendLogEntry) => void,
    ) => Promise<RustTavernHostUnsubscribe>;
    getConsoleCaptureEnabled: () => Promise<boolean>;
    setConsoleCaptureEnabled: (enabled: boolean) => Promise<void>;
};

type RustTavernBackendLogsApi = {
    tail: (options?: { limit?: number }) => Promise<RustTavernBackendLogEntry[]>;
    subscribe: (
        handler: (entry: RustTavernBackendLogEntry) => void,
    ) => Promise<RustTavernHostUnsubscribe>;
};

type RustTavernLlmApiLogsApi = {
    index: (options?: { limit?: number }) => Promise<RustTavernLlmApiLogIndexEntry[]>;
    getPreview: (id: number) => Promise<RustTavernLlmApiLogPreview>;
    getRaw: (id: number) => Promise<RustTavernLlmApiLogRaw>;
    subscribeIndex: (
        handler: (entry: RustTavernLlmApiLogIndexEntry) => void,
    ) => Promise<RustTavernHostUnsubscribe>;
    getKeep: () => Promise<number>;
    setKeep: (value: number) => Promise<void>;
};

type RustTavernDevApi = {
    frontendLogs: RustTavernFrontendLogsApi;
    backendLogs: RustTavernBackendLogsApi;
    llmApiLogs: RustTavernLlmApiLogsApi;
};

type RustTavernWorldInfoApi = {
    getLastActivation: () => Promise<RustTavernWorldInfoActivationBatch | null>;
    subscribeActivations: (
        handler: (batch: RustTavernWorldInfoActivationBatch) => void,
    ) => Promise<RustTavernHostUnsubscribe>;
    openEntry: (ref: RustTavernWorldInfoEntryRef) => Promise<{ opened: boolean }>;
};

type RustTavernExtensionStoreApi = {
    getJson: (options: { namespace: string; key: string; table?: string }) => Promise<any>;
    tryGetJson: (options: { namespace: string; key: string; table?: string }) => Promise<{ found: boolean; value?: any }>;
    setJson: (options: { namespace: string; key: string; value: any; table?: string }) => Promise<void>;
    updateJson: (options: { namespace: string; key: string; value: any; table?: string }) => Promise<void>;
    updateJSON: (options: { namespace: string; key: string; value: any; table?: string }) => Promise<void>;
    renameKey: (options: { namespace: string; key: string; newKey: string; table?: string }) => Promise<void>;
    updateKey: (options: { namespace: string; key: string; newKey: string; table?: string }) => Promise<void>;
    deleteJson: (options: { namespace: string; key: string; table?: string }) => Promise<void>;
    listKeys: (options: { namespace: string; table?: string }) => Promise<string[]>;
    listTables: (options: { namespace: string }) => Promise<string[]>;
    deleteTable: (options: { namespace: string; table: string }) => Promise<void>;
    getBlob: (options: { namespace: string; key: string; table?: string }) => Promise<Blob>;
    setBlob: (options: {
        namespace: string;
        key: string;
        table?: string;
        data: Blob | ArrayBuffer | Uint8Array | string;
    }) => Promise<void>;
    deleteBlob: (options: { namespace: string; key: string; table?: string }) => Promise<void>;
    listBlobKeys: (options: { namespace: string; table?: string }) => Promise<string[]>;
};

type RustTavernExtensionApi = {
    store: RustTavernExtensionStoreApi;
};

type RustTavernLayoutInsets = {
    top: number;
    right: number;
    bottom: number;
    left: number;
};

type RustTavernLayoutFrame = {
    left: number;
    top: number;
    width: number;
    height: number;
    right: number;
    bottom: number;
};

type RustTavernLayoutImeKind = 'composer' | 'fixed-shell' | 'dialog';

type RustTavernLayoutImeSnapshot = {
    activeSurface: Element | null;
    kind: RustTavernLayoutImeKind;
    bottom: number;
    viewportBottomInset: number;
    keyboardOffset: number;
};

type RustTavernLayoutSnapshot = {
    version: number;
    timestampMs: number;
    viewport: RustTavernLayoutFrame;
    safeInsets: RustTavernLayoutInsets;
    safeFrame: RustTavernLayoutFrame;
    ime: RustTavernLayoutImeSnapshot;
};

type RustTavernLayoutApi = {
    snapshot: () => RustTavernLayoutSnapshot;
    subscribe: (
        handler: (snapshot: RustTavernLayoutSnapshot) => void,
    ) => Promise<RustTavernHostUnsubscribe>;
};

type RustTavernCharacterCardsPickOptions = {
    multiple?: boolean;
    title?: string;
};

type RustTavernCharacterCardsApi = {
    isNativePickerAvailable: () => boolean;
    pickFiles: (options?: RustTavernCharacterCardsPickOptions) => Promise<File[] | null>;
};

type RustTavernChatSurfaceDisposable = (() => void) | { dispose: () => void };

type RustTavernChatSurfaceDetachedContext = {
    readonly mesid: number;
    readonly content: HTMLElement;
};

type RustTavernChatSurfaceMountedContext = RustTavernChatSurfaceDetachedContext & {
    readonly element: HTMLElement;
    readonly signal: AbortSignal;
};

type RustTavernChatSurfaceRuntimeContext = {
    readonly mesid: number;
    readonly source: Element;
    readonly element: HTMLElement;
    readonly content: HTMLElement;
    readonly signal: AbortSignal;
};

type RustTavernChatSurfaceRuntimeClaims = {
    claim: (
        source: Element,
        activate: (context: RustTavernChatSurfaceRuntimeContext) => RustTavernChatSurfaceDisposable,
    ) => void;
};

type RustTavernChatSurfaceParticipant = {
    id: string;
    protocolVersion: 1;
    prepareContent?: (
        context: RustTavernChatSurfaceDetachedContext,
        claims: RustTavernChatSurfaceRuntimeClaims,
    ) => void;
    didMount?: (
        context: RustTavernChatSurfaceMountedContext,
    ) => void | RustTavernChatSurfaceDisposable;
    didCommitContent?: (
        context: RustTavernChatSurfaceMountedContext,
    ) => void | RustTavernChatSurfaceDisposable;
};

type RustTavernChatSurfaceRegistration = {
    fault: (error: unknown) => void;
};

type RustTavernChatSurfaceApi = {
    readonly protocolVersion: 1;
    isManagedOwnershipRequired: () => boolean;
    registerParticipant: (
        participant: RustTavernChatSurfaceParticipant,
    ) => RustTavernChatSurfaceRegistration;
};

type RustTavernHostApi = {
    chat?: RustTavernChatApi;
    chatSurface?: RustTavernChatSurfaceApi;
    characterCards?: RustTavernCharacterCardsApi;
    agent?: RustTavernAgentApi;
    llmConnections?: RustTavernLlmConnectionsApi;
    skill?: RustTavernSkillApi;
    layout?: RustTavernLayoutApi;
    dev?: RustTavernDevApi;
    worldInfo?: RustTavernWorldInfoApi;
    extension?: RustTavernExtensionApi;
};

type RustTavernHostAbi = {
    abiVersion: 1;
    traceHeader: string;
    ready: Promise<void> | null;
    invoke: RustTavernHostInvokeApi;
    assets: RustTavernHostAssetsApi;
    api?: RustTavernHostApi;
};

type RustTavernHostUnsubscribe = () => void | Promise<void>;

type RustTavernFrontendLogEntry = {
    id: number;
    timestampMs: number;
    level: 'debug' | 'info' | 'warn' | 'error';
    message: string;
    target?: string;
};

type RustTavernBackendLogEntry = {
    id: number;
    timestampMs: number;
    level: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
    target: string;
    message: string;
};

type RustTavernLlmApiRawKind = 'json' | 'sse';

type RustTavernLlmApiLogIndexEntry = {
    id: number;
    timestampMs: number;
    level: 'INFO' | 'ERROR';
    ok: boolean;
    source: string;
    model: string | null;
    endpoint: string;
    durationMs: number;
    stream: boolean;
};

type RustTavernLlmApiLogPreview = {
    id: number;
    timestampMs: number;
    level: 'INFO' | 'ERROR';
    ok: boolean;
    source: string;
    model: string | null;
    endpoint: string;
    durationMs: number;
    stream: boolean;
    errorMessage: string | null;
    requestReadable: string;
    responseReadable: string;
    responseRawKind: RustTavernLlmApiRawKind | null;
};

type RustTavernLlmApiLogRaw = {
    id: number;
    requestRaw: string;
    responseRaw: string;
    responseRawKind: RustTavernLlmApiRawKind | null;
};

type RustTavernWorldInfoEntryRef = {
    world: string;
    uid: string | number;
};

type RustTavernWorldInfoActivationPosition =
    | 'before'
    | 'after'
    | 'an_top'
    | 'an_bottom'
    | 'depth'
    | 'em_top'
    | 'em_bottom'
    | 'outlet';

type RustTavernWorldInfoActivationEntry = {
    world: string;
    uid: string | number;
    displayName: string;
    constant: boolean;
    position?: RustTavernWorldInfoActivationPosition;
};

type RustTavernWorldInfoActivationBatch = {
    timestampMs: number;
    trigger: string;
    entries: RustTavernWorldInfoActivationEntry[];
};

type RustTavernChatRef =
    | { kind: 'character'; characterId: string; fileName: string }
    | { kind: 'group'; chatId: string };

type RustTavernChatSummary = {
    character_name: string;
    file_name: string;
    file_size: number;
    message_count: number;
    preview: string;
    date: number;
    chat_id: string | null;
    chat_metadata?: unknown | null;
};

type RustTavernChatHistoryPage = {
    startIndex: number;
    totalCount: number;
    messages: ChatMessage[];
    cursor: any;
    hasMoreBefore: boolean;
};

type RustTavernChatWindowInfo = {
    mode: 'off';
    chatKind: RustTavernChatRef['kind'];
    chatRef: RustTavernChatRef;
    totalCount: number;
    windowStartIndex: number;
    windowLength: number;
};

type RustTavernChatMessageSearchFilters = {
    role?: 'user' | 'assistant' | 'system' | 'tool';
    startIndex?: number;
    endIndex?: number;
    scanLimit?: number;
};

type RustTavernChatMessageSearchHit = {
    index: number;
    score: number;
    snippet: string;
    role: 'user' | 'assistant' | 'system' | 'tool';
    text: string;
};

type RustTavernChatHandle = {
    ref: RustTavernChatRef;
    summary: (options?: { includeMetadata?: boolean }) => Promise<RustTavernChatSummary>;
    stableId: () => Promise<string>;
    searchMessages: (options: {
        query: string;
        limit?: number;
        filters?: RustTavernChatMessageSearchFilters;
    }) => Promise<RustTavernChatMessageSearchHit[]>;
    metadata: {
        get: () => Promise<ChatMetadata>;
        setExtension: (options: { namespace: string; value: unknown }) => Promise<void>;
    };
    store: {
        getJson: (options: { namespace: string; key: string }) => Promise<unknown>;
        setJson: (options: { namespace: string; key: string; value: unknown }) => Promise<void>;
        updateJson: (options: { namespace: string; key: string; value: unknown }) => Promise<void>;
        updateJSON: (options: { namespace: string; key: string; value: unknown }) => Promise<void>;
        renameKey: (options: { namespace: string; key: string; newKey: string }) => Promise<void>;
        deleteJson: (options: { namespace: string; key: string }) => Promise<void>;
        listKeys: (options: { namespace: string }) => Promise<string[]>;
    };
    locate: {
        findLastMessage: (query?: unknown) => Promise<{ index: number; message: ChatMessage } | null>;
    };
    history: {
        tail: (options: { limit: number }) => Promise<RustTavernChatHistoryPage>;
        before: (
            page: RustTavernChatHistoryPage,
            options: { limit: number },
        ) => Promise<RustTavernChatHistoryPage>;
        beforePages: (
            page: RustTavernChatHistoryPage,
            options: { limit: number; pages: number },
        ) => Promise<RustTavernChatHistoryPage[]>;
    };
};
