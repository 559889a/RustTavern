@echo off
rem RustTavern service control for Windows.
rem
rem   start.cmd start ^| stop ^| restart ^| status ^| health ^| url ^| logs [N]
rem
rem Everything resolves relative to this script's directory: the binary reads
rem config.yaml, default\ and src\ from its own directory, so the service can be
rem moved by moving the whole folder.
rem
rem Windows specifics this script exists to handle:
rem   - A running exe cannot be overwritten, so an upgrade must stop first.
rem   - The PID file is cross-checked against the image name, because a stale
rem     PID can be reused by an unrelated process after a reboot.
rem   - Port state is probed by connecting rather than by parsing netstat, whose
rem     column layout and wording differ across Windows versions and languages.
rem   - The server is launched with Start-Process so it has no console parent;
rem     `start /b` would tie it to this window and closing the window would kill
rem     the server.
rem   - Start-Process cannot point stdout and stderr at one file, so they go to
rem     server.log and server.err.log. The server additionally keeps its own
rem     rotated log at logs\rusttavern.log; server.err.log is the only place
rem     startup failures land, because they are printed before logging exists.

setlocal EnableDelayedExpansion

set "SERVICE_DIR=%~dp0"
if "%SERVICE_DIR:~-1%"=="\" set "SERVICE_DIR=%SERVICE_DIR:~0,-1%"
set "BINARY=%SERVICE_DIR%\rusttavern.exe"
set "PID_FILE=%SERVICE_DIR%\rusttavern.pid"
set "LOG_FILE=%SERVICE_DIR%\server.log"
set "ERR_FILE=%SERVICE_DIR%\server.err.log"
set "CONFIG_FILE=%SERVICE_DIR%\config.yaml"
set "DATA_ROOT_DEFAULT=%SERVICE_DIR%\data"

set "STOP_TIMEOUT_SECONDS=15"
set "START_TIMEOUT_SECONDS=90"
set "PS=powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command"

rem The server logs in UTF-8. Switch the console to UTF-8 so its messages (which
rem include non-ASCII text) display correctly; without this a CP936/CP1252
rem console renders them as mojibake. The original code page is restored on exit.
for /f "tokens=2 delims=:" %%C in ('chcp') do set "ORIGINAL_CODEPAGE=%%C"
set "ORIGINAL_CODEPAGE=%ORIGINAL_CODEPAGE: =%"
chcp 65001 >nul 2>&1

set "COMMAND=%~1"
if "%COMMAND%"=="" set "COMMAND=help"

rem Dispatch through `call` so every command returns here and the console code
rem page is restored exactly once, on every path including failures.
call :dispatch "%COMMAND%" "%~2"
set "RESULT=%errorlevel%"
if defined ORIGINAL_CODEPAGE chcp %ORIGINAL_CODEPAGE% >nul 2>&1
exit /b %RESULT%

:dispatch
if /i "%~1"=="help"    goto print_usage
if /i "%~1"=="-h"      goto print_usage
if /i "%~1"=="--help"  goto print_usage
if /i "%~1"=="/?"      goto print_usage

call :resolve_config
if errorlevel 1 exit /b 1

if /i "%~1"=="start"   goto cmd_start
if /i "%~1"=="stop"    goto cmd_stop
if /i "%~1"=="restart" goto cmd_restart
if /i "%~1"=="status"  goto cmd_status
if /i "%~1"=="health"  goto cmd_health
if /i "%~1"=="url"     goto cmd_url
if /i "%~1"=="logs"    goto cmd_logs

call :warn "unknown command: %~1"
call :print_usage
exit /b 2

rem =========================================================== helpers ====

:log
echo [tt] %~1
exit /b 0

:warn
echo [tt] %~1 1>&2
exit /b 0

rem Read listen host/port and the security section from config.yaml in a single
rem PowerShell pass. TT_HOST / TT_PORT / TT_DATA_ROOT override it for this
rem invocation; the binary does its own resolution and is passed the same flags.
rem
rem Two cmd constraints shape how this is written:
rem   - The `for /f` sits at top level, not inside an `if exist (...)` block: cmd
rem     scans a block for its matching paren before running anything, so a `)`
rem     in the PowerShell text would close the block early.
rem   - The PowerShell text contains no `"` at all (values are captured by a
rem     pattern that excludes surrounding quotes, written as \x22/\x27), because
rem     a quote there would terminate cmd's argument.
:resolve_config
set "CFG_HOST="
set "CFG_PORT="
set "ACCESS_SUMMARY=none configured (no config.yaml)"
if not exist "%CONFIG_FILE%" goto resolve_effective

for /f "usebackq tokens=1,2,* delims=|" %%A in (`%PS% "$ErrorActionPreference='SilentlyContinue'; $listenHost=''; $listenPort=''; $mode=''; $entries=@(); $section=''; foreach ($line in Get-Content -LiteralPath '%CONFIG_FILE%') { if ($line -match '^[^\s#]') { if ($line -match '^listen:') { $section='listen' } elseif ($line -match '^security:') { $section='security' } else { $section='' }; continue }; if ($line -match '^\s*#') { continue }; if ($section -eq 'listen') { if ($line -match '^\s+host:\s*[\x22\x27]?([^\x22\x27\s]+)') { $listenHost=$matches[1] } elseif ($line -match '^\s+port:\s*[\x22\x27]?([^\x22\x27\s]+)') { $listenPort=$matches[1] } } elseif ($section -eq 'security') { if ($line -match '^\s+authMode:\s*[\x22\x27]?([^\x22\x27\s]+)') { $mode=$matches[1] } elseif ($line -match '^\s+-\s*[\x22\x27]?([^\x22\x27\s]+)') { $entries+=$matches[1] } } }; if ($mode -eq 'basic') { $access='password required (authMode basic)' } else { $access='no password (authMode none)' }; if ($entries.Count -gt 0) { $access+='; only ' + ($entries -join ', ') + ' may connect' } else { $access+='; any address may connect' }; $listenHost + '|' + $listenPort + '|' + $access"`) do set "CFG_HOST=%%A" & set "CFG_PORT=%%B" & set "ACCESS_SUMMARY=%%C"

:resolve_effective
set "LISTEN_HOST=127.0.0.1"
if defined CFG_HOST set "LISTEN_HOST=%CFG_HOST%"
if defined TT_HOST set "LISTEN_HOST=%TT_HOST%"

set "LISTEN_PORT=8000"
if defined CFG_PORT set "LISTEN_PORT=%CFG_PORT%"
if defined TT_PORT set "LISTEN_PORT=%TT_PORT%"

set "DATA_ROOT=%DATA_ROOT_DEFAULT%"
if defined TT_DATA_ROOT set "DATA_ROOT=%TT_DATA_ROOT%"

rem Host to use when talking to the service from this machine: a wildcard bind
rem is not a connectable address.
set "PROBE_HOST=%LISTEN_HOST%"
if "%LISTEN_HOST%"=="0.0.0.0" set "PROBE_HOST=127.0.0.1"
if "%LISTEN_HOST%"=="::"      set "PROBE_HOST=127.0.0.1"
if "%LISTEN_HOST%"=="*"       set "PROBE_HOST=127.0.0.1"

rem True when the configured host accepts connections from other machines.
set "LAN_REACHABLE=1"
if /i not "%LISTEN_HOST%"=="127.0.0.1" if /i not "%LISTEN_HOST%"=="localhost" if not "%LISTEN_HOST%"=="::1" set "LAN_REACHABLE=0"
exit /b 0

rem Sets PORT_STATE to "used" or "free". Probed by connecting: netstat's output
rem columns and wording differ across Windows versions and display languages.
rem
rem The result is reported as a printed token rather than an exit code: an exit
rem code has to survive PowerShell's try/finally plus cmd's errorlevel handling,
rem and a token cannot be misread.
:port_in_use
set "PORT_STATE=free"
for /f "usebackq delims=" %%R in (`%PS% "$c=New-Object Net.Sockets.TcpClient; try { $t=$c.ConnectAsync('%PROBE_HOST%',%LISTEN_PORT%); if ($t.Wait(2000) -and $c.Connected) { 'used' } else { 'free' } } catch { 'free' } finally { $c.Dispose() }"`) do set "PORT_STATE=%%R"
exit /b 0

rem The PID file can outlive its process, or (after a reboot) point at an
rem unrelated one. Only trust it when the running image is still ours.
rem
rem Sets RUNNING_PID when the service is alive, and leaves it empty otherwise.
rem Done in PowerShell rather than `tasklist | find`, because when Git Bash, MSYS
rem or Cygwin is on PATH, `find` resolves to their Unix find and the check always
rem fails -- which silently orphans a running server.
:running_pid
set "RUNNING_PID="
if not exist "%PID_FILE%" exit /b 0
set /p _pid=<"%PID_FILE%"
for /f "tokens=* delims= " %%P in ("!_pid!") do set "_pid=%%P"
if not defined _pid exit /b 0
for /f "usebackq delims=" %%R in (`%PS% "$ErrorActionPreference='SilentlyContinue'; $p=Get-Process -Id %_pid%; if ($p -and $p.ProcessName -eq 'rusttavern') { 'alive' }"`) do if "%%R"=="alive" set "RUNNING_PID=!_pid!"
exit /b 0

rem HTTP status of /__tt/health, or 000 when nothing answered.
:health_code
set "HEALTH_CODE=000"
for /f "usebackq delims=" %%C in (`%PS% "try { (Invoke-WebRequest -Uri 'http://%PROBE_HOST%:%LISTEN_PORT%/__tt/health' -UseBasicParsing -TimeoutSec 5).StatusCode } catch { if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { '000' } }"`) do set "HEALTH_CODE=%%C"
if not defined HEALTH_CODE set "HEALTH_CODE=000"
exit /b 0

rem Address other machines on the LAN can reach. Virtual adapters (Hyper-V, WSL,
rem VPN tunnels) also have IPv4 addresses, so pick the interface that carries the
rem default route rather than the first non-loopback address.
:lan_address
set "LAN_ADDRESS="
for /f "usebackq delims=" %%A in (`%PS% "$ErrorActionPreference='SilentlyContinue'; $r=Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric,ifMetric | Select-Object -First 1; if ($r) { $a=Get-NetIPAddress -InterfaceIndex $r.ifIndex -AddressFamily IPv4 | Where-Object { $_.IPAddress -ne '127.0.0.1' } | Select-Object -First 1; if ($a) { $a.IPAddress } }"`) do set "LAN_ADDRESS=%%A"
exit /b 0

rem Memory the service occupies. Working set is what Windows counts against
rem physical RAM; private bytes is this process's own committed memory. Virtual
rem size is deliberately not shown -- it is mostly untouched reserved address
rem space and says nothing about real usage.
:process_facts
set "MEM_LINE_1=memory:    unknown"
set "MEM_LINE_2="
set "UPTIME_TEXT=unknown"
for /f "usebackq tokens=1,2,3,4 delims=|" %%A in (`%PS% "$ErrorActionPreference='SilentlyContinue'; $p=Get-Process -Id %~1; if ($p) { $s=(Get-Date)-$p.StartTime; '{0:N1}|{1:N1}|{2:N1}|{3:00}:{4:00}:{5:00}' -f ($p.WorkingSet64/1MB),($p.PrivateMemorySize64/1MB),($p.PeakWorkingSet64/1MB),[int]$s.TotalHours,$s.Minutes,$s.Seconds }"`) do (
    set "MEM_LINE_1=memory:    %%A MiB working set (%%B MiB private to this process)"
    set "MEM_LINE_2=           peak %%C MiB working set"
    set "UPTIME_TEXT=%%D"
)
exit /b 0

:sleep_one
%PS% "Start-Sleep -Seconds 1" >nul 2>&1
exit /b 0

rem ============================================================= start ====

:cmd_start
call :running_pid
if defined RUNNING_PID (
    call :log "already running (pid !RUNNING_PID!)"
    goto cmd_status
)

if not exist "%BINARY%" (
    call :warn "binary not found: %BINARY%"
    exit /b 1
)

call :port_in_use
if "!PORT_STATE!"=="used" (
    call :warn "port %LISTEN_PORT% is already in use (another instance? run '%~nx0 status')"
    exit /b 1
)

set "TT_LAUNCH_ARGS=--no-open-browser"
if defined TT_HOST set "TT_LAUNCH_ARGS=!TT_LAUNCH_ARGS!,--host,%TT_HOST%"
if defined TT_PORT set "TT_LAUNCH_ARGS=!TT_LAUNCH_ARGS!,--port,%TT_PORT%"
if defined TT_DATA_ROOT set "TT_LAUNCH_ARGS=!TT_LAUNCH_ARGS!,--data-root,%TT_DATA_ROOT%"
set "TT_LAUNCH_BIN=%BINARY%"
set "TT_LAUNCH_DIR=%SERVICE_DIR%"
set "TT_LAUNCH_LOG=%LOG_FILE%"
set "TT_LAUNCH_ERR=%ERR_FILE%"
set "TT_LAUNCH_PID=%PID_FILE%"

call :log "starting %BINARY% on %LISTEN_HOST%:%LISTEN_PORT%"
del /q "%PID_FILE%" 2>nul
rem The server is launched through an intermediate PowerShell, which is itself
rem started *without* redirection. Both hops are needed:
rem
rem   - Redirection is what puts the server's output in server.log, but
rem     -RedirectStandard* makes Start-Process use CreateProcess with handle
rem     inheritance. Called directly, the server would inherit a duplicate of
rem     this script's own stdout handle; if the caller piped the script's output
rem     (`start.cmd start | more`), that pipe would stay open for the server's
rem     whole lifetime and the caller would hang instead of returning.
rem   - The outer Start-Process has no redirection, so it goes through
rem     ShellExecute and inherits nothing. The intermediate PowerShell therefore
rem     starts with a clean handle table, and the server inherits only from it.
rem
rem Every value travels in an environment variable so the inner command needs no
rem nested quoting at all (the comma-joined argument list is split back apart).
%PS% "Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-Command','$ErrorActionPreference=[char]83+[char]116+[char]111+[char]112; $p=Start-Process -FilePath $env:TT_LAUNCH_BIN -ArgumentList $env:TT_LAUNCH_ARGS.Split([char]44) -WorkingDirectory $env:TT_LAUNCH_DIR -WindowStyle Hidden -RedirectStandardOutput $env:TT_LAUNCH_LOG -RedirectStandardError $env:TT_LAUNCH_ERR -PassThru; Set-Content -LiteralPath $env:TT_LAUNCH_PID -Value $p.Id'" >nul 2>&1

rem The launcher runs asynchronously, so give it a moment to publish the PID.
set /a _waited=0
:await_pid_file
if exist "%PID_FILE%" goto pid_file_ready
call :sleep_one
set /a _waited+=1
if !_waited! LSS 10 goto await_pid_file
call :warn "failed to launch %BINARY% (no PID was published)"
%PS% "if (Test-Path -LiteralPath '%ERR_FILE%') { Get-Content -LiteralPath '%ERR_FILE%' -Encoding UTF8 -Tail 20 }" 1>&2
exit /b 1
:pid_file_ready

rem A crash-on-startup (bad config, missing resources) must not look like a
rem success, so wait for the health endpoint rather than just for the process.
set /a _waited=0
:start_wait
call :running_pid
if not defined RUNNING_PID (
    call :warn "process exited during startup; last error output:"
    %PS% "if (Test-Path -LiteralPath '%ERR_FILE%') { Get-Content -LiteralPath '%ERR_FILE%' -Encoding UTF8 -Tail 20 }" 1>&2
    %PS% "if (Test-Path -LiteralPath '%LOG_FILE%') { Get-Content -LiteralPath '%LOG_FILE%' -Encoding UTF8 -Tail 10 }" 1>&2
    del /q "%PID_FILE%" 2>nul
    exit /b 1
)
call :health_code
rem 403 means the server is up and answering but this machine's own address is
rem not in security.whitelist. That is a running server, not a failed start.
if "!HEALTH_CODE!"=="200" goto start_ok
if "!HEALTH_CODE!"=="403" goto start_ok
call :sleep_one
set /a _waited+=1
if !_waited! LSS %START_TIMEOUT_SECONDS% goto start_wait

call :warn "started but /__tt/health did not answer within %START_TIMEOUT_SECONDS%s; last log lines:"
%PS% "if (Test-Path -LiteralPath '%LOG_FILE%') { Get-Content -LiteralPath '%LOG_FILE%' -Encoding UTF8 -Tail 20 }" 1>&2
exit /b 1

:start_ok
call :log "started (pid !RUNNING_PID!)"
if "!HEALTH_CODE!"=="403" (
    call :warn "note: %PROBE_HOST% is not in security.whitelist, so a browser on this"
    call :warn "      machine is refused (403). Add 127.0.0.1 to the whitelist to open"
    call :warn "      it here too."
) else (
    call :log "local:  http://%PROBE_HOST%:%LISTEN_PORT%"
)
if "%LAN_REACHABLE%"=="0" (
    call :lan_address
    if defined LAN_ADDRESS call :log "lan:    http://!LAN_ADDRESS!:%LISTEN_PORT%   (allow the port through Windows Firewall)"
)
exit /b 0

rem ============================================================== stop ====

:cmd_stop
call :running_pid
if not defined RUNNING_PID goto stop_not_running

call :log "stopping pid !RUNNING_PID!"
taskkill /PID !RUNNING_PID! /F >nul 2>&1

set /a _waited=0
:stop_wait
call :running_pid
call :port_in_use
if not defined RUNNING_PID if "!PORT_STATE!"=="free" (
    del /q "%PID_FILE%" 2>nul
    call :log "stopped"
    exit /b 0
)
call :sleep_one
set /a _waited+=1
if !_waited! LSS %STOP_TIMEOUT_SECONDS% goto stop_wait

call :warn "process or port %LISTEN_PORT% still present after %STOP_TIMEOUT_SECONDS%s"
exit /b 1

:stop_not_running
call :log "not running"
del /q "%PID_FILE%" 2>nul
call :port_in_use
if "!PORT_STATE!"=="used" (
    call :warn "port %LISTEN_PORT% is still bound by a process this script does not track"
    call :warn "inspect it with:  tasklist /FI ^"IMAGENAME eq rusttavern.exe^""
    exit /b 1
)
exit /b 0

:cmd_restart
call :cmd_stop
if errorlevel 1 call :warn "stop reported a problem; attempting start anyway"
call :sleep_one
goto cmd_start

rem ============================================================ status ====

:cmd_status
call :running_pid
if not defined RUNNING_PID (
    echo state:      stopped
) else (
    echo state:      running ^(pid !RUNNING_PID!^)
    call :process_facts !RUNNING_PID!
    echo !MEM_LINE_1!
    if defined MEM_LINE_2 echo !MEM_LINE_2!
    echo uptime:     !UPTIME_TEXT!
)

echo listen:     %LISTEN_HOST%:%LISTEN_PORT%
call :port_in_use
if "!PORT_STATE!"=="used" (echo port bound: yes) else (echo port bound: no)
echo access:     %ACCESS_SUMMARY%
call :health_code
if "!HEALTH_CODE!"=="200" (
    echo health:     ok ^(200^)
) else if "!HEALTH_CODE!"=="403" (
    echo health:     up, but %PROBE_HOST% is blocked by security.whitelist ^(403^)
) else (
    echo health:     no answer ^(!HEALTH_CODE!^)
)
if exist "%DATA_ROOT%\" (echo data root:  %DATA_ROOT%) else (echo data root:  %DATA_ROOT% ^(missing^))
echo log:        %LOG_FILE%
call :lan_address
if "%LAN_REACHABLE%"=="0" (
    if defined LAN_ADDRESS echo lan url:    http://!LAN_ADDRESS!:%LISTEN_PORT%
) else (
    if defined LAN_ADDRESS echo lan url:    ^(unreachable: bound to %LISTEN_HOST% -- see TT_HOST in --help^)
)
exit /b 0

:cmd_health
call :health_code
echo !HEALTH_CODE!
if "!HEALTH_CODE!"=="200" exit /b 0
exit /b 1

:cmd_url
echo http://%PROBE_HOST%:%LISTEN_PORT%
if "%LAN_REACHABLE%"=="0" (
    call :lan_address
    if defined LAN_ADDRESS echo http://!LAN_ADDRESS!:%LISTEN_PORT%
)
exit /b 0

:cmd_logs
set "LINES=%~2"
if "%LINES%"=="" set "LINES=40"
if not exist "%LOG_FILE%" (
    call :warn "no log file at %LOG_FILE%"
    exit /b 1
)
%PS% "Get-Content -LiteralPath '%LOG_FILE%' -Encoding UTF8 -Tail %LINES%"
rem Startup failures are printed before logging is initialized, so they land only
rem in server.err.log. Show it when it has content, since that is where the
rem explanation for a refused start lives.
if not exist "%ERR_FILE%" exit /b 0
set "ERR_SIZE=0"
for /f "usebackq delims=" %%S in (`%PS% "(Get-Item -LiteralPath '%ERR_FILE%').Length"`) do set "ERR_SIZE=%%S"
if "!ERR_SIZE!"=="0" exit /b 0
echo.
echo --- server.err.log ^(startup errors^) ---
%PS% "Get-Content -LiteralPath '%ERR_FILE%' -Encoding UTF8 -Tail 20"
exit /b 0

:print_usage
echo RustTavern service control ^(Windows^)
echo.
echo Usage: %~nx0 ^<command^>
echo.
echo   start          Start the server and wait for /__tt/health
echo   stop           Stop it by PID and confirm the port is released
echo   restart        stop + start
echo   status         Process, memory, port, access control, health, URLs
echo   health         Print the health HTTP code; exit 0 only on 200
echo   url            Print local and LAN URLs
echo   logs [N]       Show the last N lines of server.log ^(default 40^)
echo.
echo Environment overrides ^(per invocation^):
echo   TT_HOST        Listen host, e.g.  set TT_HOST=0.0.0.0
echo                  A non-loopback bind needs one access control in
echo                  config.yaml, otherwise the server refuses to start:
echo                    security.authMode: basic    ^(username + password^), or
echo                    security.whitelist:         ^(allowed peers, e.g.
echo                      - 192.168.1.*              a single IP, a CIDR range,
echo                      - 127.0.0.1                or a trailing wildcard^)
echo   TT_PORT        Listen port
echo   TT_DATA_ROOT   Data directory ^(default: the data folder next to this script^)
echo.
echo A non-loopback bind also needs the port allowed through Windows Firewall,
echo or other machines cannot reach it even with the whitelist configured.
exit /b 0
