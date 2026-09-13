<#
RustTavern service control for Windows (PowerShell).

    .\start.ps1 start | stop | restart | status | health | url | logs [N]

A native PowerShell port of packaging/termux/start.sh: same commands, same
TT_HOST / TT_PORT / TT_DATA_ROOT overrides, same reporting and the same safety
behaviour. Same directory-resolution rule too -- the binary reads config.yaml,
default\ and src\ from its own directory, so the service moves by moving the
whole folder.

Windows specifics this script exists to handle:
  - A running exe cannot be overwritten, so an upgrade must stop first.
  - The PID file is cross-checked against the process image name, because a
    stale PID can be reused by an unrelated process after a reboot.
  - Port state is probed by connecting rather than by parsing netstat, whose
    column layout and wording differ across Windows versions and languages.
  - The server is launched detached, so closing this window does not kill it.
  - Start-Process cannot point stdout and stderr at one file, so they go to
    server.log and server.err.log. The server additionally keeps its own
    rotated log at logs\rusttavern.log; server.err.log is the only place
    startup failures land, because they are printed before logging exists.
  - The server logs in UTF-8, so the console is switched to UTF-8 for the
    duration of the run and restored on exit.
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)][string]$Command = 'help',
    [Parameter(Position = 1)][string]$Argument
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ServiceDir = $PSScriptRoot
$Binary     = Join-Path $ServiceDir 'rusttavern.exe'
$PidFile    = Join-Path $ServiceDir 'rusttavern.pid'
$LogFile    = Join-Path $ServiceDir 'server.log'
$ErrFile    = Join-Path $ServiceDir 'server.err.log'
$ConfigFile = Join-Path $ServiceDir 'config.yaml'
$DataRootDefault = Join-Path $ServiceDir 'data'

# Overridable without editing the script: $env:TT_HOST = '0.0.0.0'; .\start.ps1 start
$TtHost     = $env:TT_HOST
$TtPort     = $env:TT_PORT
$TtDataRoot = $env:TT_DATA_ROOT

$StopTimeoutSeconds  = 15
$StartTimeoutSeconds = 90

# ------------------------------------------------------------------ output ---

function Write-Log  { param([string]$Message) Write-Host "[tt] $Message" }
function Write-Warn { param([string]$Message) [Console]::Error.WriteLine("[tt] $Message") }
function Die        { param([string]$Message) Write-Warn $Message; exit 1 }

# The server logs UTF-8; keep the console in step for the duration of the run.
$script:OriginalEncoding = [Console]::OutputEncoding
try { [Console]::OutputEncoding = [Text.Encoding]::UTF8 } catch { }

# ------------------------------------------------------------- config.yaml ---

# Read the `listen:` / `security:` blocks once. Only the handful of scalars
# this script reports are parsed -- the binary does its own full resolution.
function Initialize-Config {
    $script:ConfigHost = ''
    $script:ConfigPort = ''
    $script:AuthMode  = ''
    $script:Whitelist = @()

    if (-not (Test-Path -LiteralPath $ConfigFile)) { return }

    $lines = Get-Content -LiteralPath $ConfigFile -ErrorAction SilentlyContinue
    $section = ''
    foreach ($line in $lines) {
        if ($line -match '^([A-Za-z_][A-Za-z0-9_]*):') { $section = $Matches[1]; continue }
        if ($line -match '^\s*#') { continue }

        if ($section -eq 'listen') {
            if ($line -match '^\s+host:\s*(.+?)\s*$') { $script:ConfigHost = $Matches[1].Trim('"', "'", ' ') }
            elseif ($line -match '^\s+port:\s*(\d+)')  { $script:ConfigPort = $Matches[1] }
        }
        elseif ($section -eq 'security') {
            if ($line -match '^\s+authMode:\s*(.+?)\s*$') { $script:AuthMode = $Matches[1].Trim('"', "'", ' ') }
            elseif ($line -match '^\s+-\s*(.+?)\s*$')     { $script:Whitelist += $Matches[1].Trim('"', "'", ' ') }
        }
    }
}

function Get-EffectiveHost {
    if ($TtHost) { return $TtHost }
    if ($script:ConfigHost) { return $script:ConfigHost }
    return '127.0.0.1'
}

function Get-EffectivePort {
    if ($TtPort) { return $TtPort }
    if ($script:ConfigPort) { return $script:ConfigPort }
    return '8000'
}

# Which access control the server will enforce, for the status report. Never
# prints credentials -- only which mode is configured.
function Get-SecuritySummary {
    if (-not (Test-Path -LiteralPath $ConfigFile)) { return 'none configured (no config.yaml)' }

    $mode = if ($script:AuthMode) { $script:AuthMode } else { 'none' }
    $summary = if ($mode -eq 'basic') { 'password required (authMode basic)' }
               else { 'no password (authMode none)' }

    if ($script:Whitelist.Count -gt 0) { "$summary; only $($script:Whitelist -join ', ') may connect" }
    else { "$summary; any address may connect" }
}

# -------------------------------------------------------------- process IO ---

# A wildcard bind is not a connectable address.
function Get-ProbeHost {
    switch (Get-EffectiveHost) {
        '0.0.0.0' { '127.0.0.1' }
        '::'      { '127.0.0.1' }
        '[::]'    { '127.0.0.1' }
        '*'       { '127.0.0.1' }
        default   { Get-EffectiveHost }
    }
}

function Test-PortInUse {
    param([string]$Port)

    $client = [System.Net.Sockets.TcpClient]::new()
    try {
        $task = $client.ConnectAsync((Get-ProbeHost), [int]$Port)
        if (-not $task.Wait(2000)) { return $false }
        return $client.Connected
    } catch {
        return $false
    } finally {
        $client.Dispose()
    }
}

# The PID file can outlive its process, or (after a reboot) point at an
# unrelated one. Only trust it when the image name is still our binary.
function Get-RunningPid {
    if (-not (Test-Path -LiteralPath $PidFile)) { return $null }

    $raw = (Get-Content -LiteralPath $PidFile -Raw -ErrorAction SilentlyContinue)
    if (-not $raw) { return $null }
    $parsed = 0
    if (-not [int]::TryParse(($raw -replace '[^0-9]', ''), [ref]$parsed)) { return $null }

    $process = Get-Process -Id $parsed -ErrorAction SilentlyContinue
    if (-not $process) { return $null }
    if ($process.ProcessName -ne 'rusttavern') { return $null }

    return $parsed
}

# HTTP status of /__tt/health, or 0 when nothing answered.
#
# HttpWebRequest rather than HttpClient: System.Net.Http is not loaded by
# default in Windows PowerShell 5.1, and a whitelist refusal must be reported as
# 403 rather than as a connection failure, which is exactly what the
# WebException.Response carries.
function Get-HealthCode {
    param([string]$TargetHost, [string]$TargetPort)

    $url = "http://${TargetHost}:${TargetPort}/__tt/health"
    $response = $null
    try {
        $request = [System.Net.HttpWebRequest]::Create($url)
        $request.Method = 'GET'
        $request.Timeout = 5000
        $request.AllowAutoRedirect = $false
        $response = $request.GetResponse()
        return [int]$response.StatusCode
    } catch [System.Net.WebException] {
        # A refused/erroring response still means the server answered.
        if ($_.Exception.Response) { return [int]$_.Exception.Response.StatusCode }
        return 0
    } catch {
        return 0
    } finally {
        if ($response) { $response.Close() }
    }
}

# True when the configured host actually accepts connections from other machines.
function Test-BindIsLanReachable {
    switch (Get-EffectiveHost) {
        '127.0.0.1' { return $false }
        'localhost' { return $false }
        '::1'       { return $false }
        default     { return $true }
    }
}

function Get-LanAddress {
    # Address other machines on the LAN can reach. Prefer the Wi-Fi adapter by
    # name; fall back to any non-loopback, non-APIPA IPv4 address.
    try {
        $addresses = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction Stop
    } catch {
        return ''
    }

    $usable = $addresses | Where-Object {
        $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*'
    }

    $wifi = $usable | Where-Object { $_.InterfaceAlias -match 'Wi-?Fi|WLAN|Wireless' } | Select-Object -First 1
    if ($wifi) { return $wifi.IPAddress }

    $first = $usable | Select-Object -First 1
    if ($first) { return $first.IPAddress }
    return ''
}

# ------------------------------------------------------------------ start ----

# Launch the server detached, with stdout/stderr redirected to files.
#
# Two hops on purpose. `Start-Process` *with* `-RedirectStandardOutput` goes
# through CreateProcess with bInheritHandles=TRUE, so the server inherits this
# script's stdout. If the caller piped our output (`.\start.ps1 start | more`,
# or a CI step capturing it), that pipe stays open for the server's entire
# lifetime and the caller hangs until it times out — measured, see
# packaging/windows/README.md. The outer Start-Process therefore carries no
# redirection at all (ShellExecute path, nothing inherited) and all it does is
# spawn a clean intermediate PowerShell, which is the one that redirects and
# records the server's PID for us.
function Start-ServerProcess {
    param([string[]]$Arguments)

    $quoted = ($Arguments | ForEach-Object { "'" + ($_ -replace "'", "''") + "'" }) -join ', '
    $inner = @"
`$process = Start-Process -FilePath '$Binary' -ArgumentList @($quoted) ``
    -WorkingDirectory '$ServiceDir' -WindowStyle Hidden -PassThru ``
    -RedirectStandardOutput '$LogFile' -RedirectStandardError '$ErrFile'
Set-Content -LiteralPath '$PidFile' -Value `$process.Id -NoNewline
"@

    # -EncodedCommand sidesteps every quoting layer between here and the inner
    # script, which contains paths and argument values.
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($inner))
    $outer = Start-Process -FilePath 'powershell' -WindowStyle Hidden -PassThru `
        -ArgumentList @('-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded)

    # Wait for the intermediate to finish writing the PID file; it exits
    # immediately after handing the server off.
    if (-not $outer.WaitForExit(15000)) {
        Write-Warn 'launcher did not finish within 15s'
    }
}

function Invoke-Start {
    $serverPid = Get-RunningPid
    if ($serverPid) {
        Write-Log "already running (pid $serverPid)"
        Invoke-Status
        return 0
    }

    if (-not (Test-Path -LiteralPath $Binary)) { Die "binary not found: $Binary" }

    $ListenHost = Get-EffectiveHost
    $Port  = Get-EffectivePort
    if (Test-PortInUse $Port) {
        Die "port $Port is already in use (another instance? run '$($MyInvocation.MyCommand.Name) status')"
    }

    $serverArgs = @()
    if ($TtHost)     { $serverArgs += @('--host', $TtHost) }
    if ($TtPort)     { $serverArgs += @('--port', $TtPort) }
    if ($TtDataRoot) { $serverArgs += @('--data-root', $TtDataRoot) }
    # No point opening a browser from a service launcher.
    $serverArgs += '--no-open-browser'

    Write-Log "starting: $Binary $($serverArgs -join ' ')"
    Start-ServerProcess -Arguments $serverArgs

    # A crash-on-startup (bad config, missing resources) must not look like a
    # success, so wait for the port instead of just for the process.
    $probe  = Get-ProbeHost
    $waited = 0
    while ($waited -lt $StartTimeoutSeconds) {
        $livePid = Get-RunningPid
        if (-not $livePid) {
            Write-Warn 'process exited during startup; last log lines:'
            Show-Tail $ErrFile 20
            Show-Tail $LogFile 20
            Remove-Item -LiteralPath $PidFile -Force -ErrorAction SilentlyContinue
            return 1
        }

        $code = Get-HealthCode $probe $Port
        # 403 means the server is up and answering, but this machine's address is
        # not in security.whitelist. That is a running server, not a failed start.
        if ($code -eq 200 -or $code -eq 403) {
            Write-Log "started (pid $livePid)"
            if ($code -eq 403) {
                Write-Warn "note: $probe is not in security.whitelist, so the browser on this"
                Write-Warn '      machine is refused (403). Add 127.0.0.1 to the whitelist to'
                Write-Warn '      use it here too.'
            } else {
                Write-Log "local:  http://${probe}:${Port}"
            }
            if (Test-BindIsLanReachable) {
                $lan = Get-LanAddress
                if ($lan) { Write-Log "lan:    http://${lan}:${Port}" }
            }
            return 0
        }

        Start-Sleep -Seconds 1
        $waited++
    }

    Write-Warn "started but /__tt/health did not answer within ${StartTimeoutSeconds}s; last log lines:"
    Show-Tail $ErrFile 20
    Show-Tail $LogFile 20
    return 1
}

# ------------------------------------------------------------------- stop ----

function Invoke-Stop {
    $Port = Get-EffectivePort
    $serverPid  = Get-RunningPid

    if (-not $serverPid) {
        Write-Log 'not running'
        Remove-Item -LiteralPath $PidFile -Force -ErrorAction SilentlyContinue
        if (Test-PortInUse $Port) {
            Write-Warn "port $Port is still bound by a process this script does not track"
            Write-Warn 'inspect with: Get-Process rusttavern'
            return 1
        }
        return 0
    }

    Write-Log "stopping pid $serverPid"
    Stop-Process -Id $serverPid -Force -ErrorAction SilentlyContinue

    $waited = 0
    while ($waited -lt $StopTimeoutSeconds) {
        $alive = Get-Process -Id $serverPid -ErrorAction SilentlyContinue
        if (-not $alive -and -not (Test-PortInUse $Port)) {
            Remove-Item -LiteralPath $PidFile -Force -ErrorAction SilentlyContinue
            Write-Log 'stopped'
            return 0
        }
        Start-Sleep -Seconds 1
        $waited++
    }

    Write-Warn "pid $serverPid or port $Port still present after ${StopTimeoutSeconds}s"
    Write-Warn "process alive: $([bool](Get-Process -Id $serverPid -ErrorAction SilentlyContinue))"
    Write-Warn "port $Port bound:   $(Test-PortInUse $Port)"
    return 1
}

function Invoke-Restart {
    if ((Invoke-Stop) -ne 0) { Write-Warn 'stop reported a problem; attempting start anyway' }
    Start-Sleep -Seconds 1
    Invoke-Start
}

# ---------------------------------------------------------------- status -----

function Format-Memory {
    param([int]$ProcessId)

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if (-not $process) { return @("memory:    unknown (process $ProcessId not readable)") }

    $resident = $process.WorkingSet64 / 1MB
    $peak     = $process.PeakWorkingSet64 / 1MB
    $private  = $process.PrivateMemorySize64 / 1MB

    return @(
        ('memory:    {0:N1} MiB resident (peak {1:N1} MiB)' -f $resident, $peak),
        ('           {0:N1} MiB private commit' -f $private)
    )
}

function Invoke-Status {
    $ListenHost = Get-EffectiveHost
    $Port  = Get-EffectivePort
    $probe = Get-ProbeHost
    $dataRoot = if ($TtDataRoot) { $TtDataRoot } else { $DataRootDefault }

    $serverPid = Get-RunningPid
    if ($serverPid) {
        Write-Host "state:     running (pid $serverPid)"
        Format-Memory $serverPid | ForEach-Object { Write-Host $_ }
        $process = Get-Process -Id $serverPid -ErrorAction SilentlyContinue
        if ($process) {
            $up = (Get-Date) - $process.StartTime
            Write-Host ('uptime:    {0:d\.hh\:mm\:ss}' -f $up)
        }
    } else {
        Write-Host 'state:     stopped'
    }

    Write-Host "listen:    ${ListenHost}:${Port}"
    Write-Host "port bound: $(if (Test-PortInUse $Port) { 'yes' } else { 'no' })"
    Write-Host "access:    $(Get-SecuritySummary)"

    switch (Get-HealthCode $probe $Port) {
        200     { Write-Host 'health:    ok (200)' }
        403     { Write-Host "health:    up, but $probe is blocked by security.whitelist (403)" }
        0       { Write-Host 'health:    no answer (000)' }
        default { Write-Host "health:    no answer ($_)" }
    }

    Write-Host "data root: $dataRoot$(if (Test-Path -LiteralPath $dataRoot) { '' } else { ' (missing)' })"
    Write-Host "log:       $LogFile"

    $lan = Get-LanAddress
    if (Test-BindIsLanReachable) {
        if ($lan) { Write-Host "lan url:   http://${lan}:${Port}" }
    } elseif ($lan) {
        Write-Host "lan url:   (unreachable: bound to $ListenHost -- see TT_HOST)"
    }
    return 0
}

function Invoke-Health {
    $code = Get-HealthCode (Get-ProbeHost) (Get-EffectivePort)
    Write-Host $code
    if ($code -eq 200) { return 0 } else { return 1 }
}

function Invoke-Url {
    $Port = Get-EffectivePort
    Write-Host "http://$(Get-ProbeHost):${Port}"
    if (Test-BindIsLanReachable) {
        $lan = Get-LanAddress
        if ($lan) { Write-Host "http://${lan}:${Port}" }
    }
    return 0
}

function Show-Tail {
    param([string]$Path, [int]$Lines)
    if (Test-Path -LiteralPath $Path) {
        Get-Content -LiteralPath $Path -Tail $Lines -ErrorAction SilentlyContinue |
            ForEach-Object { [Console]::Error.WriteLine($_) }
    }
}

function Invoke-Logs {
    param([string]$Lines = '40')
    if (-not (Test-Path -LiteralPath $LogFile)) { Die "no log file at $LogFile" }
    $count = 0
    if (-not [int]::TryParse($Lines, [ref]$count)) { $count = 40 }
    Get-Content -LiteralPath $LogFile -Tail $count
    return 0
}

function Show-Usage {
    Write-Host @"
RustTavern service control (Windows)

Usage: start.ps1 <command>

  start          Start the server and wait for /__tt/health
  stop           Stop it by PID and confirm the port is released
  restart        stop + start
  status         Process, memory, port, access control, health, URLs
  health         Print the health HTTP code; exit 0 only on 200
  url            Print local and LAN URLs
  logs [N]       Tail the last N lines of server.log (default 40)

Environment overrides (per invocation):
  TT_HOST        Listen host, e.g. `$env:TT_HOST = '0.0.0.0'. A non-loopback
                 bind needs one access control in config.yaml, otherwise the
                 server refuses to start:
                   security.authMode: basic  (username + password), or
                   security.whitelist: ["192.168.1.*"]  (allowed peers)
  TT_PORT        Listen port
  TT_DATA_ROOT   Data directory (default: <service dir>\data)
"@
}

# ---------------------------------------------------------------- dispatch ---

function Invoke-Dispatcher {
    Initialize-Config

    switch ($Command.ToLowerInvariant()) {
        'start'   { return (Invoke-Start) }
        'stop'    { return (Invoke-Stop) }
        'restart' { return (Invoke-Restart) }
        'status'  { return (Invoke-Status) }
        'health'  { return (Invoke-Health) }
        'url'     { return (Invoke-Url) }
        'logs'    { return (Invoke-Logs $Argument) }
        { $_ -in 'help', '-h', '--help', '/?' } { Show-Usage; return 0 }
        default {
            Write-Warn "unknown command: $Command"
            Show-Usage
            return 2
        }
    }
}

# The dispatcher returns the command's exit code; the console encoding is
# restored exactly once, on every path including a failure inside a command.
$exitCode = 0
try {
    $exitCode = Invoke-Dispatcher
} finally {
    if ($script:OriginalEncoding) { [Console]::OutputEncoding = $script:OriginalEncoding }
}
exit $exitCode
