use std::{
    future::{Future, IntoFuture},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use axum::{Router, routing::get};
use tower_http::compression::CompressionLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use agenthub_config::ServerRole;
use agenthub_logging::{LogSpec, init_tracing, split_log_path};

const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

fn log_config_details(
    config: &agenthub_config::AppConfig,
    info: &agenthub_config::ConfigLoadInfo,
    log_path: Option<&str>,
    log_spec: Option<&LogSpec>,
    web_dir: Option<&str>,
) {
    if info.file_exists {
        tracing::info!("config source: file");
        tracing::info!("config path: {}", info.path.display());
    } else {
        tracing::info!("config source: defaults");
        tracing::info!("config path: {} (missing)", info.path.display());
    }
    if !info.env_overrides.is_empty() {
        tracing::warn!("env overrides ignored: {}", info.env_overrides.join(", "));
    }
    tracing::info!("config listen: {}", config.listen_addr());
    tracing::info!("config server.role: {}", config.server_role().as_str());
    if let Some(configured_node_id) = config.configured_server_node_id() {
        tracing::info!("config server.node_id: {}", configured_node_id);
    } else {
        tracing::info!("config server.node_id: <unset>");
    }
    tracing::info!(
        "effective server.node_id: {}",
        config
            .server_node_id()
            .unwrap_or_else(|_| "<invalid>".to_string())
    );
    if let Some((dir, file_name)) = log_spec {
        tracing::info!(
            "config log_path: {} (dir {}, file {})",
            log_path.unwrap_or("<stdout>"),
            dir.display(),
            file_name
        );
    } else {
        tracing::info!("config log_path: <stdout>");
    }
    if config.server_role() == ServerRole::Node {
        tracing::info!("config web auth settings: ignored in node mode");
    } else {
        tracing::info!("config rp_id: {}", config.rp_id());
        tracing::info!("config rp_origin: {}", config.rp_origin());
        tracing::info!("config rp_name: {}", config.rp_name());
    }
    let configured_web_dir = config
        .web_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !cfg!(debug_assertions) && config.web_dir.is_some() {
        tracing::info!("config web_dir ignored in release build");
    }
    if config.server_role() == ServerRole::Node {
        tracing::info!(
            "config web_dir: {} (ignored in node mode)",
            configured_web_dir.unwrap_or("<unset>")
        );
        tracing::info!("effective web_dir: <disabled>");
    } else {
        tracing::info!(
            "config web_dir: {}",
            configured_web_dir.unwrap_or("<unset>")
        );
        tracing::info!("effective web_dir: {}", web_dir.unwrap_or("embedded"));
    }
    tracing::info!("config codex_acp_binary: {}", config.codex_acp_binary());
    tracing::info!(
        "config codex_acp_default_mode: {}",
        config
            .codex_acp_default_mode()
            .as_deref()
            .unwrap_or("<unset>")
    );
    tracing::info!(
        "config codex_acp_multi_agent_enabled: {}",
        config.codex_acp_multi_agent_enabled()
    );
    if config.server_role() == ServerRole::Node {
        tracing::info!("config push settings: disabled in node mode");
    } else {
        tracing::info!("config vapid_subject: {}", config.vapid_subject());
        tracing::info!(
            "config vapid_keys_path: {}",
            config.vapid_keys_path().display()
        );
    }
    tracing::info!(
        "config worktree.default_root: {}",
        config.default_worktree_root()
    );
    tracing::info!(
        "config internal_grpc.enabled: {}",
        config.internal_grpc_enabled()
    );
    tracing::info!(
        "config internal_grpc.listen: {}",
        config.internal_grpc_listen_addr()
    );
    tracing::info!(
        "config internal_grpc.security.mode: {}",
        config.internal_grpc_security_mode()
    );
}

fn validate_startup_config(config: &agenthub_config::AppConfig) -> anyhow::Result<()> {
    let _ = config.server_node_id()?;
    if config.server_role() == ServerRole::Node && !config.internal_grpc_enabled() {
        anyhow::bail!("server.role = \"node\" requires internal_grpc.enabled = true");
    }
    Ok(())
}

fn build_app_router(
    state: crate::state::AppState,
    api_router: Router,
    web_dir: Option<&str>,
) -> Router {
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        .on_failure(DefaultOnFailure::new().level(Level::ERROR));
    let compression = CompressionLayer::new().gzip(true);

    let mut app = Router::new()
        .route("/health", get(crate::api::health))
        .nest("/api", api_router)
        .nest("/sse", crate::sse::router(state.clone()))
        .layer(trace)
        .layer(compression);
    if let Some(dir) = web_dir {
        tracing::info!("serving web from dir: {}", dir);
        let web_root = Arc::new(std::path::PathBuf::from(dir));
        app = app.fallback(move |uri| crate::web::dir_handler(web_root.clone(), uri));
    } else {
        tracing::info!("serving web from embedded assets");
        app = app.fallback(crate::web::embedded_handler);
    }
    app
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "failed to listen for Ctrl+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                let _ = stream.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to listen for SIGTERM");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn run_shutdown_cleanup(state: crate::state::AppState) {
    tracing::warn!("shutdown signal received; marking running agents exited");
    if let Err(err) = state.agents.mark_exited_on_shutdown().await {
        tracing::error!(
            error = %err,
            "shutdown cleanup failed to mark running agents exited"
        );
    }
}

async fn await_server_shutdown<F>(server: Pin<&mut F>) -> std::io::Result<()>
where
    F: Future<Output = std::io::Result<()>>,
{
    match tokio::time::timeout(SERVER_SHUTDOWN_TIMEOUT, server).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(
                timeout_seconds = SERVER_SHUTDOWN_TIMEOUT.as_secs(),
                "graceful shutdown timed out; forcing exit"
            );
            Ok(())
        }
    }
}

async fn run_main_server(
    state: crate::state::AppState,
    config: &agenthub_config::AppConfig,
    web_dir: Option<&str>,
) -> anyhow::Result<()> {
    let _internal_grpc = crate::internal::maybe_spawn_internal_grpc(state.clone(), config).await?;
    let api_router = crate::api::router(state.clone());
    let app = build_app_router(state.clone(), api_router, web_dir);

    let addr: SocketAddr = config.listen_addr().parse()?;
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    tracing::info!("listening on {}", addr);
    let server = axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .with_graceful_shutdown(async move {
            if shutdown_rx.changed().await.is_err() {
                // Receiver dropped before shutdown signal; let graceful shutdown future end.
            }
        })
        .into_future();
    tokio::pin!(server);

    tokio::select! {
        result = &mut server => {
            result?;
        }
        _ = wait_for_shutdown_signal() => {
            run_shutdown_cleanup(state).await;
            let _ = shutdown_tx.send(true);
            await_server_shutdown(server.as_mut()).await?;
        }
    }
    Ok(())
}

async fn run_node_server(
    state: crate::state::AppState,
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<()> {
    tracing::info!(
        node_id = %config.server_node_id()?,
        "node mode active; public HTTP server disabled"
    );
    let _internal_grpc = crate::internal::maybe_spawn_internal_grpc(state.clone(), config).await?;
    wait_for_shutdown_signal().await;
    run_shutdown_cleanup(state).await;
    Ok(())
}

fn pyroscope_bootstrap_options(
    role: ServerRole,
) -> agenthub_pyroscope::PyroscopeBootstrapOptions<'static> {
    agenthub_pyroscope::PyroscopeBootstrapOptions {
        application_name: match role {
            ServerRole::Main => "agenthub.server",
            ServerRole::Node => "agenthub.node",
        },
        application_version: env!("CARGO_PKG_VERSION"),
    }
}

pub async fn run() -> anyhow::Result<()> {
    match crate::cli::parse_root_cli_from_env() {
        Ok(crate::cli::RootCliCommand::Serve) => {}
        Ok(crate::cli::RootCliCommand::Init { args }) => {
            return crate::init_cli::run_from_args(&args).await;
        }
        Ok(crate::cli::RootCliCommand::Doctor { args }) => {
            return crate::doctor_cli::run_from_args(&args).await;
        }
        Ok(crate::cli::RootCliCommand::Actor { args }) => {
            return crate::actor_cli::run_from_args(&args).await;
        }
        Ok(crate::cli::RootCliCommand::Migrate { args }) => {
            return crate::migrate_cli::run_from_args(&args).await;
        }
        Ok(crate::cli::RootCliCommand::LegacyActorMcp) => {
            return Err(anyhow::anyhow!(
                "`agenthub actor-mcp` has been removed. Use `agenthub actor ...` instead."
            ));
        }
        Err(err) if crate::cli::is_non_error_clap_exit(&err) => {
            err.print()?;
            return Ok(());
        }
        Err(err) => {
            return Err(err.into());
        }
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (config, info) = agenthub_config::AppConfig::load_with_info()?;
    let log_path = config.log_path();
    let log_spec = log_path.as_deref().map(split_log_path);
    let web_dir = config.effective_web_dir();
    let _log_guard = init_tracing(filter, log_spec.as_ref())?;
    log_config_details(
        &config,
        &info,
        log_path.as_deref(),
        log_spec.as_ref(),
        web_dir.as_deref(),
    );
    validate_startup_config(&config)?;
    let _pyroscope =
        agenthub_pyroscope::maybe_start_from_env(pyroscope_bootstrap_options(config.server_role()));
    let state = crate::state::AppState::init(config.clone()).await?;
    match config.server_role() {
        ServerRole::Main => run_main_server(state, &config, web_dir.as_deref()).await,
        ServerRole::Node => run_node_server(state, &config).await,
    }
}

#[cfg(test)]
mod tests {
    use std::future;

    use agenthub_config::{
        AppConfig, ConfigLoadInfo, InternalGrpcConfig, PushConfig, ServerConfig, ServerRole,
    };
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };

    use tower::util::ServiceExt;

    #[test]
    fn log_config_details_handles_all_branches() {
        let config = agenthub_config::AppConfig::default();
        let file_info = agenthub_config::ConfigLoadInfo {
            path: std::path::PathBuf::from("/tmp/agenthub/config.toml"),
            file_exists: true,
            env_overrides: vec!["AGENTHUB_LISTEN".to_string()],
        };
        let default_info = agenthub_config::ConfigLoadInfo {
            path: std::path::PathBuf::from("/tmp/agenthub/config.toml"),
            file_exists: false,
            env_overrides: vec![],
        };
        let spec = (
            std::path::PathBuf::from("/tmp/agenthub/logs"),
            "service.log".to_string(),
        );
        super::log_config_details(
            &config,
            &file_info,
            Some("/tmp/agenthub/logs/service.log"),
            Some(&spec),
            Some("web/dist"),
        );
        super::log_config_details(&config, &default_info, None, None, None);
    }

    #[test]
    fn log_config_details_handles_node_mode_branches() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: Some("127.0.0.1:18080".to_string()),
                role: Some(ServerRole::Node),
                node_id: Some("node-east".to_string()),
            }),
            push: Some(PushConfig {
                subject: Some("mailto:test@example.com".to_string()),
                keys_path: Some("/tmp/agenthub/vapid.json".to_string()),
            }),
            web_dir: Some("web/custom".to_string()),
            ..Default::default()
        };
        let info = ConfigLoadInfo {
            path: std::path::PathBuf::from("/tmp/agenthub/config.toml"),
            file_exists: true,
            env_overrides: vec![],
        };
        let spec = (
            std::path::PathBuf::from("/tmp/agenthub/logs"),
            "node.log".to_string(),
        );

        super::log_config_details(
            &config,
            &info,
            Some("/tmp/agenthub/logs/node.log"),
            Some(&spec),
            None,
        );
    }

    #[test]
    fn validate_startup_config_rejects_node_mode_without_internal_grpc() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: Some("node-east".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(
            super::validate_startup_config(&config)
                .expect_err("node role should require internal grpc")
                .to_string(),
            "server.role = \"node\" requires internal_grpc.enabled = true"
        );
    }

    #[test]
    fn validate_startup_config_rejects_node_mode_without_node_id() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: None,
            }),
            internal_grpc: Some(InternalGrpcConfig {
                enabled: Some(true),
                listen: Some("127.0.0.1:50051".to_string()),
                security: None,
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            super::validate_startup_config(&config)
                .expect_err("node mode should require explicit node id")
                .to_string(),
            "server.node_id is required when server.role = \"node\""
        );
    }

    #[test]
    fn validate_startup_config_rejects_reserved_main_node_id() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: Some("main".to_string()),
            }),
            internal_grpc: Some(InternalGrpcConfig {
                enabled: Some(true),
                listen: Some("127.0.0.1:50051".to_string()),
                security: None,
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            super::validate_startup_config(&config)
                .expect_err("reserved node id should fail validation")
                .to_string(),
            "server.node_id must not be \"main\" when server.role = \"node\""
        );
    }

    #[test]
    fn validate_startup_config_accepts_node_mode_with_internal_grpc() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: Some("node-east".to_string()),
            }),
            internal_grpc: Some(InternalGrpcConfig {
                enabled: Some(true),
                listen: Some("127.0.0.1:50051".to_string()),
                security: None,
                auth: None,
                bootstrap: None,
            }),
            ..Default::default()
        };
        super::validate_startup_config(&config).expect("node role should validate");
    }

    #[test]
    fn validate_startup_config_accepts_main_role_without_internal_grpc() {
        super::validate_startup_config(&AppConfig::default())
            .expect("main role should not require internal grpc");
    }

    #[test]
    fn pyroscope_bootstrap_options_use_role_scoped_application_names() {
        let main_options = super::pyroscope_bootstrap_options(ServerRole::Main);
        assert_eq!(main_options.application_name, "agenthub.server");
        assert_eq!(main_options.application_version, env!("CARGO_PKG_VERSION"));

        let node_options = super::pyroscope_bootstrap_options(ServerRole::Node);
        assert_eq!(node_options.application_name, "agenthub.node");
        assert_eq!(node_options.application_version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn build_app_router_serves_embedded_health() {
        let state = crate::api::team_tests::build_test_state().await;
        let api_router = crate::api::router(state.clone());
        let app = super::build_app_router(state, api_router, None);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request health");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn build_app_router_serves_file_system_fallback() {
        let unique = format!(
            "agenthub-web-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create web dir");
        std::fs::write(
            dir.join("index.html"),
            "<html><body>agenthub-test</body></html>",
        )
        .expect("write index html");
        std::fs::create_dir_all(dir.join("assets")).expect("create assets dir");
        std::fs::write(dir.join("assets/app-abc123.js"), "console.log('agenthub');")
            .expect("write asset");
        std::fs::write(
            dir.join("manifest.webmanifest"),
            "{\"name\":\"AgentHub\",\"display\":\"standalone\"}",
        )
        .expect("write manifest");
        std::fs::write(
            dir.join("sw.js"),
            "self.addEventListener('install', () => self.skipWaiting());",
        )
        .expect("write service worker");

        let state = crate::api::team_tests::build_test_state().await;
        let api_router = crate::api::router(state.clone());
        let app = super::build_app_router(state, api_router, Some(dir.to_string_lossy().as_ref()));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/not-found-path")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request fallback path");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body_text = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert!(body_text.contains("agenthub-test"));

        let asset_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/assets/app-abc123.js")
                    .body(Body::empty())
                    .expect("build asset request"),
            )
            .await
            .expect("request asset path");
        assert_eq!(asset_response.status(), StatusCode::OK);
        assert_eq!(
            asset_response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("public, max-age=31536000, immutable")
        );

        let manifest_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/manifest.webmanifest")
                    .body(Body::empty())
                    .expect("build manifest request"),
            )
            .await
            .expect("request manifest path");
        assert_eq!(manifest_response.status(), StatusCode::OK);
        assert_eq!(
            manifest_response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );

        let service_worker_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/sw.js")
                    .body(Body::empty())
                    .expect("build service worker request"),
            )
            .await
            .expect("request service worker path");
        assert_eq!(service_worker_response.status(), StatusCode::OK);
        assert_eq!(
            service_worker_response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );

        let missing_asset_response = app
            .oneshot(
                Request::builder()
                    .uri("/assets/missing-abc123.js")
                    .body(Body::empty())
                    .expect("build missing asset request"),
            )
            .await
            .expect("request missing asset path");
        assert_eq!(missing_asset_response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            missing_asset_response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
    }

    #[tokio::test]
    async fn await_server_shutdown_returns_when_server_finishes_before_timeout() {
        let server = async { Ok::<(), std::io::Error>(()) };
        tokio::pin!(server);
        super::await_server_shutdown(server.as_mut())
            .await
            .expect("server result");
    }

    #[tokio::test]
    async fn await_server_shutdown_forces_exit_after_timeout() {
        let server = future::pending::<std::io::Result<()>>();
        tokio::pin!(server);
        super::await_server_shutdown(server.as_mut())
            .await
            .expect("forced shutdown result");
    }
}
