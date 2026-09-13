#!/usr/bin/env bash

# Flatpak launch wrapper: the app tree under /app is read-only, and the
# server's default data root is <exe dir>/data, which would be /app/bin/data
# (immutable). Route persistent data to the per-user XDG data directory,
# which the manifest's finish-args must allow (--filesystem=xdg-data/rusttavern).
#
# `--resources /app/bin` points the server at the packaged default/ and src/
# trees; the static web root resolves next to them.

set -euo pipefail

readonly DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/rusttavern"

mkdir -p "$DATA_DIR"

exec /app/bin/rusttavern \
    --data-root "$DATA_DIR" \
    --resources /app/bin \
    "$@"
