#!/usr/bin/env node

// Guardrail: the preload hints in `src/index.html` must stay in step with the
// sheets reached through `@import`.
//
// `@import` is only discovered after the importing sheet has been fetched and
// parsed, so every import level is another round trip. The hints in index.html
// move those fetches into the first wave (docs/history/handoff/NextHarnessHandoff.md
// §8F.2), which makes the two lists duplicated data: adding an `@import` without
// a hint silently gives that sheet its own round trip back.
//
// Checks:
//   1. every sheet reached through `@import` has a `preload as="style"` hint
//   2. every hinted href (style and module) exists on disk
//   3. every `@import` target exists on disk
//
// Usage: node scripts/check-preload-hints.mjs

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const srcRoot = path.resolve(scriptDir, '..', 'src');
const indexPath = path.join(srcRoot, 'index.html');
const rootSheet = path.join(srcRoot, 'style.css');

const LINK_TAG = /<link\b[^>]*>/gi;
const ATTRIBUTE = /([a-zA-Z][\w-]*)\s*=\s*(["'])([\s\S]*?)\2/g;
// url(css/x.css) | url('css/x.css') | url("css/x.css") | "css/x.css"
const IMPORT = /@import\s+(?:url\(\s*(?:"([^"]+)"|'([^']+)'|([^)'"\s]+))\s*\)|"([^"]+)"|'([^']+)')/g;

/** Attributes of every `<link>` tag, lowercased by name. */
function linkTags(html) {
    const tags = [];
    for (const match of html.matchAll(LINK_TAG)) {
        const attributes = {};
        for (const attribute of match[0].matchAll(ATTRIBUTE)) {
            attributes[attribute[1].toLowerCase()] = attribute[3];
        }
        tags.push(attributes);
    }
    return tags;
}

/** @import specifiers of a sheet, resolved against the sheet's own directory. */
function importsOf(sheetPath) {
    const source = readFileSync(sheetPath, 'utf8');
    const resolved = [];
    for (const match of source.matchAll(IMPORT)) {
        const specifier = match[1] ?? match[2] ?? match[3] ?? match[4] ?? match[5];
        if (!specifier) {
            continue;
        }
        resolved.push({
            specifier,
            // A root-absolute `@import` is served from the web root (`src/`).
            path: specifier.startsWith('/')
                ? path.join(srcRoot, specifier.slice(1))
                : path.resolve(path.dirname(sheetPath), specifier),
        });
    }
    return resolved;
}

function relative(absolutePath) {
    return path.relative(srcRoot, absolutePath).split(path.sep).join('/');
}

/** Hrefs of `<link rel=... >` tags, resolved against the web root. */
function hintedHrefs(tags, rel, as = null) {
    const hrefs = [];
    for (const tag of tags) {
        if ((tag.rel ?? '').toLowerCase() !== rel) {
            continue;
        }
        if (as !== null && (tag.as ?? '').toLowerCase() !== as) {
            continue;
        }
        if (tag.href) {
            hrefs.push(tag.href);
        }
    }
    return hrefs;
}

function main() {
    if (!existsSync(indexPath) || !existsSync(rootSheet)) {
        console.error('[preload-hints] FAILED\n- src/index.html or src/style.css is missing');
        process.exitCode = 1;
        return;
    }

    const tags = linkTags(readFileSync(indexPath, 'utf8'));
    const stylePreloads = new Set(hintedHrefs(tags, 'preload', 'style'));
    const modulePreloads = hintedHrefs(tags, 'modulepreload');

    const problems = [];

    // Every hinted href must resolve to a real file, otherwise the browser spends
    // a request to learn that and logs a console error.
    for (const [kind, hrefs] of [['preload as=style', stylePreloads], ['modulepreload', modulePreloads]]) {
        for (const href of hrefs) {
            const resolved = path.join(srcRoot, href.replace(/^\//, ''));
            if (!existsSync(resolved)) {
                problems.push(`${kind} hint points at a missing file: ${href}`);
            }
        }
    }

    // Walk the @import graph from style.css, requiring a hint for every sheet.
    const seen = new Set();
    const queue = [rootSheet];
    let walked = 0;
    while (queue.length > 0) {
        const sheetPath = queue.pop();
        if (seen.has(sheetPath)) {
            continue;
        }
        seen.add(sheetPath);
        walked += 1;

        for (const imported of importsOf(sheetPath)) {
            const href = relative(imported.path);
            if (!existsSync(imported.path)) {
                problems.push(`${relative(sheetPath)} imports a missing sheet: ${imported.specifier}`);
                continue;
            }
            if (!stylePreloads.has(href)) {
                problems.push(
                    `${relative(sheetPath)} imports ${imported.specifier}, but index.html has no`
                    + ` <link rel="preload" as="style" href="${href}">`,
                );
            }
            queue.push(imported.path);
        }
    }

    if (problems.length > 0) {
        console.error(`[preload-hints] FAILED\n${problems.map(problem => `- ${problem}`).join('\n')}`);
        console.error('[preload-hints] Tip: add the missing <link rel="preload" as="style"> next to the others in src/index.html.');
        process.exitCode = 1;
        return;
    }

    console.log(
        `[preload-hints] OK (${walked} sheets walked, ${stylePreloads.size} style +`
        + ` ${modulePreloads.length} module hints)`,
    );
}

main();
