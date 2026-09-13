function getTypeTag(value) {
    return Object.prototype.toString.call(value);
}

function isRequestLike(value) {
    if (!value || typeof value !== 'object') {
        return false;
    }

    return typeof value.url === 'string'
        && typeof value.method === 'string'
        && typeof value.clone === 'function';
}

function isFormDataLike(value) {
    return getTypeTag(value) === '[object FormData]';
}

function isUrlSearchParamsLike(value) {
    return getTypeTag(value) === '[object URLSearchParams]';
}

function isBlobLike(value) {
    return getTypeTag(value) === '[object Blob]';
}

function isArrayBufferLike(value) {
    return getTypeTag(value) === '[object ArrayBuffer]';
}

function resolveBaseUrl(baseUrl) {
    return String(baseUrl || window.location.origin);
}

export function toUrl(input, baseUrl = window.location.origin) {
    const resolvedBaseUrl = resolveBaseUrl(baseUrl);
    try {
        if (input instanceof URL) {
            return input;
        }

        if (isRequestLike(input)) {
            return new URL(input.url, resolvedBaseUrl);
        }

        if (typeof input === 'string') {
            return new URL(input, resolvedBaseUrl);
        }
    } catch {
        return null;
    }

    return null;
}

export function getMethodHint(input, init) {
    if (init?.method) {
        return String(init.method).toUpperCase();
    }

    if (isRequestLike(input)) {
        return String(input.method || 'GET').toUpperCase();
    }

    return 'GET';
}

export async function getMethod(input, init) {
    return getMethodHint(input, init);
}

export async function readRequestBody(input, init) {
    let rawBody;

    if (init && Object.prototype.hasOwnProperty.call(init, 'body')) {
        rawBody = init.body;
    } else if (isRequestLike(input) && !['GET', 'HEAD'].includes(String(input.method).toUpperCase())) {
        rawBody = await input.clone().text();
    }

    if (rawBody === undefined || rawBody === null) {
        return null;
    }

    if (isFormDataLike(rawBody)) {
        return rawBody;
    }

    if (typeof rawBody === 'string') {
        return parseMaybeJson(rawBody);
    }

    if (isUrlSearchParamsLike(rawBody)) {
        return Object.fromEntries(rawBody.entries());
    }

    if (isBlobLike(rawBody)) {
        const text = await rawBody.text();
        return parseMaybeJson(text);
    }

    if (ArrayBuffer.isView(rawBody) || isArrayBufferLike(rawBody)) {
        const bytes = isArrayBufferLike(rawBody) ? new Uint8Array(rawBody) : new Uint8Array(rawBody.buffer);
        const text = new TextDecoder().decode(bytes);
        return parseMaybeJson(text);
    }

    return rawBody;
}

export function parseMaybeJson(value) {
    const text = String(value || '').trim();
    if (!text) {
        return {};
    }

    try {
        return JSON.parse(text);
    } catch {
        return text;
    }
}

export async function safeJson(response) {
    try {
        return await response.json();
    } catch {
        return {};
    }
}

export function jsonResponse(data, status = 200) {
    return new Response(JSON.stringify(data), {
        status,
        headers: {
            'Content-Type': 'application/json',
        },
    });
}

// `jsonResponse` serializes up front so that every caller gets its own parsed
// copy. That is two full passes over the payload (stringify here, parse in the
// caller) plus a duplicate object graph; on a 2.51MB chat the overhead measured
// ~14x the parse itself (handoff 8A.3).
//
// `jsonResponseNoCopy` skips both: `.json()` hands back the route's own object,
// and the body stream only serializes if something actually pulls it. Everything
// else (`.text()`, `.clone()`, `.body`) still goes through a normal Response.
//
// Only legal for payloads this layer owns outright -- built per request and
// never shared. A caller that mutates the result is mutating that object.
class JsonResponse extends Response {
    #data;

    constructor(data, status) {
        // highWaterMark 0 keeps the stream from pulling -- and so from
        // serializing -- until a reader actually shows up.
        super(singleChunkBody(data), {
            status,
            headers: {
                'Content-Type': 'application/json',
            },
        });
        this.#data = data;
    }

    async json() {
        return this.#data;
    }
}

function singleChunkBody(data) {
    let sent = false;

    return new ReadableStream({
        pull(controller) {
            if (sent) {
                controller.close();
                return;
            }

            sent = true;
            controller.enqueue(new TextEncoder().encode(JSON.stringify(data)));
        },
    }, { highWaterMark: 0 });
}

/** @param {any} data @param {number} [status] */
export function jsonResponseNoCopy(data, status = 200) {
    return new JsonResponse(data, status);
}

export function safeResponseStatusText(value) {
    const text = String(value || '').trim();
    return /^[\x20-\x7E]*$/.test(text) ? text : '';
}

export function textResponse(text, status = 200, statusText) {
    const init = {
        status,
        headers: {
            'Content-Type': 'text/plain; charset=utf-8',
        },
    };
    const safeStatusText = safeResponseStatusText(statusText);

    if (safeStatusText) {
        init.statusText = safeStatusText;
    }

    return new Response(String(text), {
        ...init,
    });
}
