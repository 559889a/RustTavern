#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const repositoryRoot = resolve(import.meta.dirname, '../..');
const requestedTag = process.argv[2] ?? '';
const version = requestedTag.startsWith('v') ? requestedTag.slice(1) : requestedTag;

if (!/^[0-9]+\.[0-9]+\.[0-9]+$/.test(version)) {
    throw new Error(`Stable release tag must be vMAJOR.MINOR.PATCH, received: ${requestedTag || '(empty)'}`);
}

async function read(path) {
    return readFile(resolve(repositoryRoot, path), 'utf8');
}

function expectMatch(path, source, pattern) {
    const match = source.match(pattern);
    if (!match) {
        throw new Error(`Unable to locate the release version in ${path}`);
    }
    if (match[1] !== version) {
        throw new Error(`${path} declares ${match[1]}, but the release tag declares ${version}`);
    }
}

const packageJsonPath = 'package.json';
const cargoManifestPath = 'crates/rusttavern/Cargo.toml';
const cargoLockPath = 'Cargo.lock';
const nixPackagePath = 'nix/package.nix';

const [
    packageJsonSource,
    cargoManifestSource,
    cargoLockSource,
    nixPackageSource,
] = await Promise.all([
    read(packageJsonPath),
    read(cargoManifestPath),
    read(cargoLockPath),
    read(nixPackagePath),
]);

const packageJson = JSON.parse(packageJsonSource);

if (packageJson.version !== version) {
    throw new Error(`${packageJsonPath} declares ${packageJson.version}, but the release tag declares ${version}`);
}

expectMatch(cargoManifestPath, cargoManifestSource, /^\[package\]\s+name = "rusttavern"\s+version = "([^"]+)"/m);
expectMatch(cargoLockPath, cargoLockSource, /\[\[package\]\]\s+name = "rusttavern"\s+version = "([^"]+)"/m);
expectMatch(nixPackagePath, nixPackageSource, /^\s*version = "([^"]+)";$/m);

process.stdout.write(`${version}\n`);
