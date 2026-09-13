#!/data/data/com.termux/files/usr/bin/bash
# Copy a SillyTavern data directory into a RustTavern data root.
#
#   ./import-sillytavern.sh [--from <ST data dir>] [--to <TT data root>]
#                           [--skip-backups] [--dry-run]
#
# RustTavern reuses SillyTavern's on-disk layout as a compatibility contract
# (default-user/characters, chats, worlds, User Avatars, settings.json, ...), so
# the user-data subtree transfers as-is. What is NOT copied is SillyTavern's own
# server-side scratch state, which RustTavern neither reads nor can use:
#
#   _cache/      ST thumbnail/asset cache, regenerated on demand
#   _webpack/    ST's frontend build cache
#   _uploads/    ST upload staging
#   _storage/    ST key-value store (node-persist)
#   _errors/     ST error dumps
#   access.log / content.log     ST server logs
#   cookie-secret.txt            ST session signing secret
#
# Existing files in the destination are left alone unless --overwrite is given,
# so re-running this cannot silently clobber data written by RustTavern.
set -uo pipefail

ST_DIR_DEFAULT="$HOME/SillyTavern/data"
SERVICE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TT_DIR_DEFAULT="$SERVICE_DIR/data"

ST_DIR="$ST_DIR_DEFAULT"
TT_DIR="$TT_DIR_DEFAULT"
SKIP_BACKUPS=0
DRY_RUN=0
OVERWRITE=0

log()  { printf '[import] %s\n' "$*"; }
warn() { printf '[import] %s\n' "$*" >&2; }
die()  { printf '[import] %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case "$1" in
        --from) ST_DIR="${2:-}"; shift 2 ;;
        --to) TT_DIR="${2:-}"; shift 2 ;;
        --skip-backups) SKIP_BACKUPS=1; shift ;;
        --overwrite) OVERWRITE=1; shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help)
            sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

[ -n "$ST_DIR" ] && [ -n "$TT_DIR" ] || die "--from and --to need values"
[ -d "$ST_DIR" ] || die "SillyTavern data directory not found: $ST_DIR"
[ -d "$ST_DIR/default-user" ] || die "$ST_DIR does not look like an ST data dir (no default-user/)"

log "source:      $ST_DIR"
log "destination: $TT_DIR"

# Subtrees carried over. `_css` holds the user stylesheet, which RustTavern
# serves at /css/user.css.
COPY_ENTRIES=(default-user _css)

# `cp -n` (no-clobber) unless --overwrite; -a preserves times so the chat
# summary cache and RustTavern's incremental scans see stable signatures.
cp_flags='-a'
[ "$OVERWRITE" -eq 1 ] || cp_flags='-a -n'

total_bytes=0
for entry in "${COPY_ENTRIES[@]}"; do
    src="$ST_DIR/$entry"
    if [ ! -e "$src" ]; then
        log "skip $entry (not present in source)"
        continue
    fi
    size="$(du -sk "$src" 2>/dev/null | awk '{print $1}')"
    total_bytes=$((total_bytes + size))
    log "copy $entry ($((size / 1024)) MB)"
done

if [ "$SKIP_BACKUPS" -eq 1 ] && [ -d "$ST_DIR/default-user/backups" ]; then
    log "excluding default-user/backups ($(du -sh "$ST_DIR/default-user/backups" 2>/dev/null | awk '{print $1}')) per --skip-backups"
fi

avail_kb="$(df -k "$(dirname "$TT_DIR")" 2>/dev/null | awk 'NR==2{print $4}')"
if [ -n "$avail_kb" ] && [ "$avail_kb" -lt "$total_bytes" ]; then
    die "not enough free space: need ~$((total_bytes / 1024)) MB, have $((avail_kb / 1024)) MB"
fi

if [ "$DRY_RUN" -eq 1 ]; then
    log "dry run: nothing written"
    exit 0
fi

mkdir -p "$TT_DIR" || die "cannot create $TT_DIR"

for entry in "${COPY_ENTRIES[@]}"; do
    src="$ST_DIR/$entry"
    [ -e "$src" ] || continue
    dest="$TT_DIR/$entry"
    mkdir -p "$dest"
    log "copying $entry ..."
    if [ "$entry" = "default-user" ] && [ "$SKIP_BACKUPS" -eq 1 ]; then
        # Copy children individually so backups/ can be left behind.
        for child in "$src"/*; do
            [ -e "$child" ] || continue
            [ "$(basename "$child")" = "backups" ] && continue
            cp $cp_flags "$child" "$dest/" 2>/dev/null
        done
    else
        for child in "$src"/*; do
            [ -e "$child" ] || continue
            cp $cp_flags "$child" "$dest/" 2>/dev/null
        done
    fi
done

log "--- result ---"
for entry in "${COPY_ENTRIES[@]}"; do
    [ -d "$TT_DIR/$entry" ] || continue
    printf '[import] %-14s %s\n' "$entry" "$(du -sh "$TT_DIR/$entry" 2>/dev/null | awk '{print $1}')"
done

# Spot-check the pieces the app needs to boot with real content.
chars=$(find "$TT_DIR/default-user/characters" -maxdepth 1 -name '*.png' 2>/dev/null | wc -l)
chats=$(find "$TT_DIR/default-user/chats" -maxdepth 2 -name '*.jsonl' 2>/dev/null | wc -l)
worlds=$(find "$TT_DIR/default-user/worlds" -maxdepth 1 -name '*.json' 2>/dev/null | wc -l)
printf '[import] characters: %s   chats: %s   world info: %s\n' "$chars" "$chats" "$worlds"
[ -f "$TT_DIR/default-user/settings.json" ] && log "settings.json present" || warn "settings.json missing"
[ -f "$TT_DIR/default-user/secrets.json" ] && log "secrets.json present (API credentials carried over)"

log "done. start the server with: $SERVICE_DIR/start.sh start"
