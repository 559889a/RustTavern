#!/data/data/com.termux/files/usr/bin/bash
# RustTavern service control for Termux (Android / aarch64).
#
#   ./start.sh start | stop | restart | status | logs [N] | health | url
#
# Everything is resolved relative to this script's directory: the binary reads
# config.yaml, default/ and src/ from its own directory, so the service can be
# moved by moving the whole folder.
#
# Termux specifics this script exists to handle:
#   - Android aggressively freezes background processes, so a wake lock is taken
#     while the service runs and released when it stops.
#   - pgrep/pkill are unreliable here (they report a process gone while it still
#     holds the port), so the PID is tracked in a file, killed by PID, and both
#     /proc/<pid> and the port are polled until actually free.
#   - Overwriting a running binary yields ETXTBSY, so an upgrade must stop first.
#   - Port state is probed by connecting, not by listing sockets: this device has
#     no `ss`, its `netstat -tln` reports "no support for AF INET (tcp)", and
#     /proc/net/tcp is not readable under Termux.
set -uo pipefail

SERVICE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SERVICE_DIR/rusttavern"
PID_FILE="$SERVICE_DIR/rusttavern.pid"
LOG_FILE="$SERVICE_DIR/server.log"
CONFIG_FILE="$SERVICE_DIR/config.yaml"
DATA_ROOT_DEFAULT="$SERVICE_DIR/data"

# Overridable without editing the script: TT_HOST=0.0.0.0 ./start.sh start
TT_HOST="${TT_HOST:-}"
TT_PORT="${TT_PORT:-}"
TT_DATA_ROOT="${TT_DATA_ROOT:-}"

STOP_TIMEOUT_SECONDS=15
START_TIMEOUT_SECONDS=60

log()  { printf '[tt] %s\n' "$*"; }
warn() { printf '[tt] %s\n' "$*" >&2; }
die()  { printf '[tt] %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------- helpers ----

# Effective listen host/port: CLI env wins, then config.yaml, then the server's
# own defaults. Only used for reporting and health checks; the binary does its
# own resolution.
config_value() {
    local key="$1" fallback="$2" value=''
    if [ -f "$CONFIG_FILE" ]; then
        # `listen:` block only; good enough for the two scalars we report.
        value="$(sed -n '/^listen:/,/^[^[:space:]]/p' "$CONFIG_FILE" \
            | sed -n "s/^[[:space:]]\+${key}:[[:space:]]*//p" \
            | head -1 | tr -d '"'"'"'\r')"
    fi
    printf '%s' "${value:-$fallback}"
}

effective_host() { printf '%s' "${TT_HOST:-$(config_value host 127.0.0.1)}"; }
effective_port() { printf '%s' "${TT_PORT:-$(config_value port 8000)}"; }

# Which access control the server will enforce, for the status report. Never
# prints credentials — only which mode is configured.
security_summary() {
    [ -f "$CONFIG_FILE" ] || { printf 'none configured (no config.yaml)'; return; }
    local block mode entries
    block="$(sed -n '/^security:/,/^[^[:space:]#]/p' "$CONFIG_FILE")"
    mode="$(printf '%s' "$block" | sed -n 's/^[[:space:]]\+authMode:[[:space:]]*//p' \
        | head -1 | tr -d '"'"'"'\r')"
    # Uncommented `- entry` lines inside the block are whitelist entries; the
    # inline `whitelist: [...]` form is only ever empty in the shipped template.
    entries="$(printf '%s' "$block" | sed -n 's/^[[:space:]]\+-[[:space:]]*//p' | tr -d '\r' \
        | paste -sd, - | sed 's/,/, /g')"

    case "${mode:-none}" in
        basic) printf 'password required (authMode basic)' ;;
        *)     printf 'no password (authMode none)' ;;
    esac
    if [ -n "$entries" ]; then
        printf '; only %s may connect' "$entries"
    else
        printf '; any address may connect'
    fi
}

# Host to use when talking to the service from this device. A wildcard bind is
# not a connectable address.
probe_host() {
    local host
    host="$(effective_host)"
    case "$host" in
        0.0.0.0|'::'|'[::]'|'*') printf '127.0.0.1' ;;
        *) printf '%s' "$host" ;;
    esac
}

port_in_use() {
    local port="$1" host
    host="$(probe_host)"
    # No socket listing tool works on this platform (see the header), so probe by
    # connecting. `curl --connect-to` would need a URL scheme anyway, and a plain
    # TCP connect is enough to tell "someone is listening" from "nothing there".
    if command -v nc >/dev/null 2>&1; then
        nc -z -w 2 "$host" "$port" >/dev/null 2>&1 && return 0
        return 1
    fi
    # curl exits 7 (couldn't connect) when the port is closed; any other exit
    # means something accepted the connection, even if it spoke no HTTP.
    curl -m 3 -s -o /dev/null "http://${host}:${port}/__tt/health" >/dev/null 2>&1
    [ "$?" != "7" ]
}

pid_alive() {
    local pid="$1"
    [ -n "$pid" ] && [ -d "/proc/$pid" ]
}

# The PID file can outlive its process, or (after a reboot) point at an
# unrelated one. Only trust it when /proc says the command is our binary.
running_pid() {
    local pid
    [ -f "$PID_FILE" ] || return 1
    pid="$(tr -cd '0-9' < "$PID_FILE")"
    [ -n "$pid" ] || return 1
    pid_alive "$pid" || return 1
    if ! tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -q 'rusttavern'; then
        return 1
    fi
    printf '%s' "$pid"
}

wake_lock() {
    command -v termux-wake-lock >/dev/null 2>&1 && termux-wake-lock 2>/dev/null || true
}

wake_unlock() {
    command -v termux-wake-unlock >/dev/null 2>&1 && termux-wake-unlock 2>/dev/null || true
}

# ------------------------------------------------------------------ start ----

cmd_start() {
    local pid host port probe url waited code
    if pid="$(running_pid)"; then
        log "already running (pid $pid)"
        cmd_status
        return 0
    fi

    [ -f "$BINARY" ] || die "binary not found: $BINARY"
    [ -x "$BINARY" ] || chmod +x "$BINARY" || die "cannot make $BINARY executable"

    host="$(effective_host)"
    port="$(effective_port)"
    if port_in_use "$port"; then
        die "port $port is already in use (another instance? run '$0 status')"
    fi

    local -a args=()
    [ -n "$TT_HOST" ] && args+=(--host "$TT_HOST")
    [ -n "$TT_PORT" ] && args+=(--port "$TT_PORT")
    if [ -n "$TT_DATA_ROOT" ]; then
        args+=(--data-root "$TT_DATA_ROOT")
    fi
    # Termux has no browser to open and the attempt just logs a failure.
    args+=(--no-open-browser)

    wake_lock
    log "starting: $BINARY ${args[*]}"
    (
        cd "$SERVICE_DIR" || exit 1
        nohup "$BINARY" "${args[@]}" >> "$LOG_FILE" 2>&1 &
        printf '%s' "$!" > "$PID_FILE"
    )

    # A crash-on-startup (bad config, missing resources) must not look like a
    # success, so wait for the port instead of just for the process.
    probe="$(probe_host)"
    waited=0
    while [ "$waited" -lt "$START_TIMEOUT_SECONDS" ]; do
        if ! pid="$(running_pid)"; then
            warn "process exited during startup; last log lines:"
            tail -n 20 "$LOG_FILE" >&2
            wake_unlock
            rm -f "$PID_FILE"
            return 1
        fi
        code="$(health_code "$probe" "$port")"
        # 403 means the server is up and answering, but this device's own
        # address is not in security.whitelist. That is a running server, not a
        # failed start — only the local probe is blocked.
        if [ "$code" = "200" ] || [ "$code" = "403" ]; then
            url="http://${probe}:${port}"
            log "started (pid $pid)"
            if [ "$code" = "403" ]; then
                warn "note: $probe is not in security.whitelist, so the browser on this"
                warn "      device is refused (403). Add 127.0.0.1 to the whitelist to"
                warn "      use it here too."
            else
                log "local:  $url"
            fi
            local lan
            if bind_is_lan_reachable; then
                lan="$(lan_address)"
                [ -n "$lan" ] && log "lan:    http://${lan}:${port}"
            fi
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    warn "started but /__tt/health did not answer within ${START_TIMEOUT_SECONDS}s; last log lines:"
    tail -n 20 "$LOG_FILE" >&2
    return 1
}

# ------------------------------------------------------------------- stop ----

cmd_stop() {
    local pid port waited
    port="$(effective_port)"
    if ! pid="$(running_pid)"; then
        log "not running"
        rm -f "$PID_FILE"
        # A stale wake lock keeps draining the battery, so release it anyway.
        wake_unlock
        if port_in_use "$port"; then
            warn "port $port is still bound by a process this script does not track"
            warn "inspect with: ps aux | grep rusttavern"
            return 1
        fi
        return 0
    fi

    log "stopping pid $pid"
    kill -9 "$pid" 2>/dev/null

    waited=0
    while [ "$waited" -lt "$STOP_TIMEOUT_SECONDS" ]; do
        if ! pid_alive "$pid" && ! port_in_use "$port"; then
            rm -f "$PID_FILE"
            wake_unlock
            log "stopped"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done

    warn "pid $pid or port $port still present after ${STOP_TIMEOUT_SECONDS}s"
    warn "/proc/$pid exists: $([ -d "/proc/$pid" ] && echo yes || echo no)"
    warn "port $port bound:   $(port_in_use "$port" && echo yes || echo no)"
    return 1
}

cmd_restart() {
    cmd_stop || warn "stop reported a problem; attempting start anyway"
    sleep 1
    cmd_start
}

# ----------------------------------------------------------------- status ----

health_code() {
    local host="$1" port="$2" code
    # curl prints the code from -w *and* exits non-zero when it cannot connect,
    # so `|| printf 000` would concatenate into "000000". Take the printed value
    # and only substitute when nothing came back at all.
    code="$(curl -m 5 -s -o /dev/null -w '%{http_code}' "http://${host}:${port}/__tt/health" 2>/dev/null)"
    printf '%s' "${code:-000}"
}

# True when the configured host actually accepts connections from other devices.
bind_is_lan_reachable() {
    case "$(effective_host)" in
        127.0.0.1|localhost|'::1') return 1 ;;
        *) return 0 ;;
    esac
}

lan_address() {
    # Address other devices on the LAN can reach. This device also has a VPN
    # tun0 and a virtual 172.19.0.1, so pick the Wi-Fi interface's address by
    # name rather than taking the first non-loopback one.
    #
    # `ifconfig wlan0` fails here ("cannot open /proc/net/dev") because a named
    # lookup needs that file; the unfiltered listing works, so parse its blocks.
    if command -v ifconfig >/dev/null 2>&1; then
        ifconfig 2>/dev/null | awk '
            /^[a-z0-9]+:/ { interface = substr($1, 1, length($1) - 1) }
            interface == "wlan0" && /inet / {
                for (i = 1; i <= NF; i++) {
                    if ($i == "inet") { print $(i + 1); exit }
                }
            }
        '
        return
    fi
    if command -v ip >/dev/null 2>&1; then
        ip -4 addr show wlan0 2>/dev/null \
            | sed -n 's/.*inet \([0-9.]\+\).*/\1/p' | head -1
    fi
}

# Memory the service currently occupies, read from /proc/<pid>/status.
#
# RSS on its own is misleading on Android: the kernel swaps idle anonymous pages
# out to zram, so an mostly-idle server reports a few MB resident while holding
# several more in swap. Report resident + swapped together, plus VmHWM (the
# high-water mark of resident memory), which is the number that matters when
# arguing about memory pressure. VmSize is deliberately not shown — it is
# reserved address space (2 GB here, mostly untouched thread-stack guards) and
# says nothing about real usage.
memory_lines() {
    local status="/proc/$1/status"
    [ -r "$status" ] || return 1
    awk '
        /^VmRSS:/    { rss   = $2 }
        /^VmSwap:/   { swap  = $2 }
        /^VmHWM:/    { peak  = $2 }
        /^RssAnon:/  { anon  = $2 }
        /^RssFile:/  { file  = $2 }
        /^RssShmem:/ { shmem = $2 }
        END {
            if (rss == "") { exit 1 }
            printf "memory:    %.1f MiB in use (%.1f resident + %.1f swapped out)\n",
                (rss + swap) / 1024, rss / 1024, swap / 1024
            printf "           peak %.1f MiB resident; of the resident part %.1f anon (heap/stack), %.1f file-backed, %.1f shared\n",
                peak / 1024, anon / 1024, file / 1024, shmem / 1024
        }
    ' "$status"
}

cmd_status() {
    local pid host port probe code data_root
    host="$(effective_host)"
    port="$(effective_port)"
    probe="$(probe_host)"
    data_root="${TT_DATA_ROOT:-$DATA_ROOT_DEFAULT}"

    if pid="$(running_pid)"; then
        printf 'state:     running (pid %s)\n' "$pid"
        memory_lines "$pid" || printf 'memory:    unknown (/proc/%s/status unreadable)\n' "$pid"
        printf 'uptime:    %s\n' "$(ps -o etime= -p "$pid" 2>/dev/null | tr -d ' ' || echo unknown)"
    else
        printf 'state:     stopped\n'
    fi

    printf 'listen:    %s:%s\n' "$host" "$port"
    printf 'port bound: %s\n' "$(port_in_use "$port" && echo yes || echo no)"
    printf 'access:    %s\n' "$(security_summary)"
    code="$(health_code "$probe" "$port")"
    case "$code" in
        200) printf 'health:    ok (200)\n' ;;
        # The server answered but refused this device's address, which is a
        # whitelist decision rather than a health problem.
        403) printf 'health:    up, but %s is blocked by security.whitelist (403)\n' "$probe" ;;
        *)   printf 'health:    no answer (%s)\n' "$code" ;;
    esac
    printf 'data root: %s%s\n' "$data_root" "$([ -d "$data_root" ] && echo '' || echo ' (missing)')"
    printf 'log:       %s\n' "$LOG_FILE"
    local lan
    lan="$(lan_address)"
    if bind_is_lan_reachable; then
        [ -n "$lan" ] && printf 'lan url:   http://%s:%s\n' "$lan" "$port"
    elif [ -n "$lan" ]; then
        printf 'lan url:   (unreachable: bound to %s — see TT_HOST in --help)\n' "$host"
    fi
    return 0
}

cmd_health() {
    local code
    code="$(health_code "$(probe_host)" "$(effective_port)")"
    printf '%s\n' "$code"
    [ "$code" = "200" ]
}

cmd_url() {
    local lan port
    port="$(effective_port)"
    printf 'http://%s:%s\n' "$(probe_host)" "$port"
    if bind_is_lan_reachable; then
        lan="$(lan_address)"
        [ -n "$lan" ] && printf 'http://%s:%s\n' "$lan" "$port"
    fi
    return 0
}

cmd_logs() {
    local lines="${1:-40}"
    [ -f "$LOG_FILE" ] || die "no log file at $LOG_FILE"
    tail -n "$lines" "$LOG_FILE"
}

usage() {
    cat <<EOF
RustTavern service control (Termux)

Usage: $0 <command>

  start          Start the server and wait for /__tt/health
  stop           Stop it by PID and confirm the port is released
  restart        stop + start
  status         Process, memory, port, access control, health, URLs
  health         Print the health HTTP code; exit 0 only on 200
  url            Print local and LAN URLs
  logs [N]       Tail the last N lines of server.log (default 40)

Environment overrides (per invocation):
  TT_HOST        Listen host, e.g. TT_HOST=0.0.0.0. A non-loopback bind
                 needs one access control in config.yaml, otherwise the
                 server refuses to start:
                   security.authMode: basic  (username + password), or
                   security.whitelist: ["192.168.1.*"]  (allowed peers)
  TT_PORT        Listen port
  TT_DATA_ROOT   Data directory (default: <service dir>/data)
EOF
}

case "${1:-}" in
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    restart) cmd_restart ;;
    status)  cmd_status ;;
    health)  cmd_health ;;
    url)     cmd_url ;;
    logs)    cmd_logs "${2:-40}" ;;
    ''|-h|--help|help) usage ;;
    *) warn "unknown command: $1"; usage; exit 2 ;;
esac
