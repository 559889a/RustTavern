//! HTTP server host: config -> runtime paths -> observability -> AppState -> axum router -> serve.
//!
//! This module replaces the Tauri shell (`app::host`) as the composition
//! entry point. The frontend is served statically; commands are dispatched
//! through `POST /__tt/invoke/{command}`; events stream through
//! `GET /__tt/events` (SSE).

pub mod config;
pub mod dispatch;
pub mod events;
pub mod fs_resources;
pub mod host_resources;
pub mod router;
pub mod security;
pub mod stream;

use std::sync::Arc;

use clap::Parser;

use crate::app::context::AppContext;
use crate::app::events::EventBus;
use crate::app::host_handles::HostHandles;
use crate::app::{BackendReadiness, StartupProfile, spawn_initialization};
use crate::infrastructure::logging::{devtools, llm_api_logs, tracing_runtime};
use crate::presentation::web_resources::WebResourceServices;
use crate::server::config::{CliOverrides, ServerConfig};
use crate::server::router::{AppStateSlot, ServerState};
use tt_adapter_http::HttpClientPool;
use tt_application::services::runtime_paths_service::RuntimePathsService;

/// CLI entry point.
#[derive(Debug, Parser)]
#[command(name = "rusttavern", about = "SillyTavern-compatible HTTP server with Rust backend")]
struct Cli {
    /// Listen host (default: 127.0.0.1)
    #[arg(long)]
    host: Option<String>,
    /// Listen port (default: 8000)
    #[arg(long)]
    port: Option<u16>,
    /// Data directory override (wins over config.yaml)
    #[arg(long)]
    data_root: Option<std::path::PathBuf>,
    /// Config file path (default: <exe dir>/config.yaml)
    #[arg(long)]
    config: Option<std::path::PathBuf>,
    /// Resources root override (contains default/ and src/scripts/templates/)
    #[arg(long)]
    resources: Option<std::path::PathBuf>,
    /// Do not open the browser on startup
    #[arg(long)]
    no_open_browser: bool,
}

/// Run the HTTP server (blocking; installs Ctrl-C shutdown).
pub fn run() {
    let cli = Cli::parse();
    let cli_overrides = CliOverrides {
        host: cli.host,
        port: cli.port,
        data_root: cli.data_root,
        config_file: cli.config,
        resources_root: cli.resources,
        no_open_browser: cli.no_open_browser,
    };

    let config = match config::resolve_server_config(&cli_overrides) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("RustTavern: {error}");
            std::process::exit(1);
        }
    };

    // Fail-fast: an open (unauthenticated) bind on a non-loopback address is
    // almost always a misconfiguration. Resolve the security policy now so
    // config errors surface before the listener starts.
    if let Err(error) = crate::server::security::SecurityPolicy::from_config(&config.security) {
        eprintln!("RustTavern: {error}");
        std::process::exit(1);
    }
    // A whitelist-only bind is allowed but has no password; the warning has to
    // reach the operator before tracing exists, so print it here and log it
    // again once the subscriber is installed.
    let bind_warning = match crate::server::security::validate_bind_security(&config) {
        Ok(warning) => warning,
        Err(error) => {
            eprintln!("RustTavern: {error}");
            std::process::exit(1);
        }
    };
    if let Some(warning) = &bind_warning {
        eprintln!("RustTavern: warning: {warning}");
    }

    // Cap the runtime worker threads: the target machine has 1 GB of RAM,
    // and one worker per core would multiply stack reservations without
    // helping an IO-bound server. Two workers still let blocking HTTP client
    // work run on a second thread.
    let worker_threads = std::thread::available_parallelism()
        .map(|count| count.get().min(2))
        .unwrap_or(2);
    // Cap the blocking pool as well. Its default ceiling is 512 threads, which
    // a burst of host-resource image work or character-index scans can actually
    // reach; each thread carries its own stack and allocator heap. Sixteen is
    // above the internal parallelism caps (8) so the gated scans cannot starve,
    // and far below what would multiply a phone's memory.
    let max_blocking_threads = 16;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(max_blocking_threads)
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");
    if let Err(error) = runtime.block_on(run_server(config, bind_warning)) {
        eprintln!("RustTavern: {error}");
        std::process::exit(1);
    }
}

async fn run_server(
    config: ServerConfig,
    bind_warning: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Runtime paths.
    let runtime_paths = config::resolve_runtime_paths(&config)?;

    // 2. Observability (log store, error hub, tracing). These are published
    //    before AppState so startup diagnostics are captured.
    let events = Arc::new(EventBus::new());
    let http_client_pool = Arc::new(HttpClientPool::new(crate::product::USER_AGENT));
    let backend_log_store = Arc::new(devtools::BackendLogStore::new(events.clone()));
    let backend_error_hub = Arc::new(crate::app::backend_errors::BackendErrorHub::new(
        events.clone(),
    ));
    let llm_api_log_store = Arc::new(llm_api_logs::LlmApiLogStore::new(
        events.clone(),
        runtime_paths.log_root.clone(),
    ));
    let tracing_guard = tracing_runtime::init_tracing(
        &runtime_paths.log_root,
        Some(backend_log_store.clone()),
        {
            let backend_error_hub = backend_error_hub.clone();
            Arc::new(move |message| backend_error_hub.emit_or_queue(message))
        },
    )
    .map_err(std::io::Error::other)?;

    // An unauthenticated LAN bind was already printed to stderr before the
    // subscriber existed; record it in the log file too so it survives in the
    // deployment's own diagnostics.
    if let Some(warning) = bind_warning {
        tracing::warn!("{warning}");
    }

    // Purge stale logs before tracing opens new appenders. Failure is
    // intentionally non-fatal.
    if let Err(error) = devtools::purge_old_log_files(
        &runtime_paths.log_root,
        std::time::Duration::from_secs(14 * 24 * 60 * 60),
    ) {
        tracing::warn!("Failed to purge old log files: {error}");
    }

    // 3. Startup profile snapshot.
    let startup_profile = StartupProfile::load(&runtime_paths.data_root)
        .map_err(|error| format!("Failed to load startup profile: {error}"))?;
    http_client_pool
        .apply_request_proxy_settings(&startup_profile.rusttavern_settings.request_proxy)
        .map_err(|error| format!("Failed to apply request proxy settings: {error}"))?;
    llm_api_log_store.apply_settings(startup_profile.rusttavern_settings.dev.effective_llm_api_keep());

    // 4. Host-bound resource services (host resources + user media + templates).
    let web_resources = WebResourceServices::install(&runtime_paths, &config, &startup_profile)?;

    // 5. Managed handles shared with commands and composition.
    let dev_observability = Arc::new(crate::app::dev_observability::DevObservabilityHub::new(
        runtime_paths.clone(),
        backend_log_store.clone(),
        llm_api_log_store.clone(),
    ));

    let runtime_paths_service = Arc::new(RuntimePathsService::new(
        tt_ports::runtime_paths::RuntimePathsSnapshot {
            mode: tt_ports::runtime_paths::RuntimeModeInfo::Standard,
            app_root: runtime_paths.app_root.clone(),
            data_root: runtime_paths.data_root.clone(),
        },
        Arc::new(crate::infrastructure::runtime_paths_config_store::FilesystemRuntimePathConfigStore),
    ));

    let backend_readiness = Arc::new(BackendReadiness::new());
    let streams = crate::server::stream::StreamRegistry::new();
    let canonical_data_root = canonical_data_root(&runtime_paths.data_root);

    let app_context = Arc::new(AppContext {
        events: events.clone(),
        http_client_pool,
        llm_api_log_store: llm_api_log_store.clone(),
        resources: web_resources.resources.clone(),
        runtime_paths: Arc::new(runtime_paths.clone()),
    });

    let host_handles = HostHandles {
        backend_errors: backend_error_hub.clone(),
        backend_readiness: backend_readiness.clone(),
        observability: dev_observability.clone(),
        host_resources: web_resources.host_resources.clone(),
        user_media: web_resources.user_media_service.clone(),
        runtime_paths: runtime_paths_service,
        templates: web_resources.bundled_templates.clone(),
        streams: streams.clone(),
        data_root: canonical_data_root.clone(),
        archive_imports_root: runtime_paths.archive_imports_root.clone(),
        fs_resources: crate::server::fs_resources::FsResourceRegistry::new(),
    };

    // 6. AppState initialization (async; commands wait on the slot).
    let app_state_slot = AppStateSlot::new();
    spawn_initialization(
        app_context,
        host_handles,
        app_state_slot.clone(),
        runtime_paths,
        startup_profile,
        backend_readiness,
    );

    // 7. Command registry.
    let registry = Arc::new(crate::presentation::commands::registry::build_registry());

    // 8. Security state (whitelist + auth + sessions).
    let security = Arc::new(crate::server::security::SecurityState {
        policy: crate::server::security::SecurityPolicy::from_config(&config.security)
            .expect("security policy validated before startup"),
        sessions: crate::server::security::SessionStore::new(),
    });

    // 9. Serve.
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);
    let server_state = Arc::new(ServerState {
        events: events.clone(),
        registry,
        app_state: app_state_slot,
        host_resources: web_resources.host_resources.clone(),
        streams,
        shutdown: shutdown_tx.clone(),
        web_root: config.web_root.clone(),
        data_root: canonical_data_root,
    });

    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port))
        .await
        .map_err(|error| {
            format!(
                "Failed to bind {}:{}: {error}",
                config.host, config.port
            )
        })?;

    let local_addr = listener.local_addr().map_err(|error| {
        format!("Failed to resolve local address: {error}")
    })?;
    println!(
        "RustTavern server listening on http://{}:{}{}",
        config.host,
        local_addr.port(),
        if config.host == "127.0.0.1" || config.host == "localhost" {
            ""
        } else {
            " (reachable from other devices on the network)"
        }
    );

    if config.auto_open_browser {
        open_browser(&config);
    }

    let app = crate::server::router::build_router(server_state, security);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(events, shutdown_tx))
    .await
    .map_err(|error| format!("Server error: {error}"))?;

    drop(tracing_guard);
    Ok(())
}

/// Grace window after announcing a graceful exit, so connected browsers
/// receive `rusttavern-graceful-exit-requested` over SSE and can flush
/// pending state before the process terminates.
const GRACEFUL_EXIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

async fn shutdown_signal(
    events: std::sync::Arc<EventBus>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("Shutdown signal received, stopping server");

    // Announce the graceful exit to connected browsers (the frontend flush
    // service listens for this event) and give them a window to flush before
    // the process terminates. The SSE connections are torn down after the
    // grace period via `shutdown_tx`, letting `axum::serve` drain cleanly.
    announce_graceful_exit(&events, &shutdown_tx, GRACEFUL_EXIT_GRACE).await;
}

/// Publish the graceful-exit event, wait `grace` for browsers to flush, then
/// signal SSE handlers to terminate their connections.
async fn announce_graceful_exit(
    events: &std::sync::Arc<EventBus>,
    shutdown_tx: &tokio::sync::watch::Sender<bool>,
    grace: std::time::Duration,
) {
    events.publish(crate::app::events::GRACEFUL_EXIT_EVENT, ());
    tokio::time::sleep(grace).await;
    let _ = shutdown_tx.send(true);
}

/// Canonicalize the data root for path validation; falls back to the raw path
/// when canonicalization fails (the directory is created later at startup).
fn canonical_data_root(data_root: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(data_root).unwrap_or_else(|_| data_root.to_path_buf())
}

fn open_browser(config: &ServerConfig) {
    let url = format!("http://{}:{}/", config.host, config.port);
    let result = open_url(&url);
    if let Err(error) = result {
        tracing::warn!(
            "Failed to open browser automatically (visit {url} manually): {error}",
        );
    }
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    // Termux/Android is a unix family member without xdg-open in PATH by
    // default; termux-open handles it when present, otherwise fall back to
    // the unsupported message instead of reporting a spawned xdg-open as
    // success.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::process::Command;
        let opener = if which_exists("termux-open") {
            "termux-open"
        } else if which_exists("xdg-open") {
            "xdg-open"
        } else {
            return Err("No browser opener found (tried termux-open, xdg-open)".to_string());
        };
        Command::new(opener)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        let _ = url;
        Err("Browser auto-open is not supported on this platform".to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn which_exists(command: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join(command).is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::app::events::{EventBus, GRACEFUL_EXIT_EVENT};

    use super::announce_graceful_exit;

    #[tokio::test]
    async fn announce_graceful_exit_publishes_event_then_signals_shutdown() {
        let events = Arc::new(EventBus::new());
        let mut event_rx = events.subscribe();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn({
            let events = events.clone();
            let shutdown_tx = shutdown_tx.clone();
            async move {
                announce_graceful_exit(
                    &events,
                    &shutdown_tx,
                    std::time::Duration::from_millis(50),
                )
                .await;
            }
        });

        // The graceful-exit event is published before the SSE teardown signal.
        let message = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .expect("graceful-exit event must be published")
        .expect("event channel open");
        assert_eq!(message.event, GRACEFUL_EXIT_EVENT);

        // The watch value flips to true so SSE connections terminate.
        tokio::time::timeout(std::time::Duration::from_secs(2), shutdown_rx.changed())
            .await
            .expect("shutdown watch must be signaled")
            .expect("shutdown watch channel must stay open");
        assert!(*shutdown_rx.borrow_and_update());
    }
}
