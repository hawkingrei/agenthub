use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::{fs::OpenOptions, io::Write as IoWrite};

use axum::{Router, routing::get};
use chrono::{DateTime, TimeZone, Utc};
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

type LogSpec = (std::path::PathBuf, String);

struct LogGuards {
    _writer: tracing_appender::non_blocking::WorkerGuard,
}

struct ActiveHourlyLogWriter {
    directory: PathBuf,
    file_name: String,
    current_path: PathBuf,
    current_hour_slot: i64,
    file: std::fs::File,
    now_utc: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

impl ActiveHourlyLogWriter {
    const ROTATION_CANDIDATE_MAX_ATTEMPTS: usize = 1_000;

    fn new(directory: PathBuf, file_name: String) -> std::io::Result<Self> {
        Self::new_with_clock(directory, file_name, Box::new(Utc::now))
    }

    fn new_with_clock(
        directory: PathBuf,
        file_name: String,
        now_utc: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    ) -> std::io::Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let now = now_utc();
        let current_hour_slot = Self::hour_slot(now);
        let current_path = directory.join(&file_name);
        let file = Self::open_append_file(&current_path)?;
        Ok(Self {
            directory,
            file_name,
            current_path,
            current_hour_slot,
            file,
            now_utc,
        })
    }

    fn open_append_file(path: &Path) -> std::io::Result<std::fs::File> {
        OpenOptions::new().create(true).append(true).open(path)
    }

    fn hour_slot(now: DateTime<Utc>) -> i64 {
        now.timestamp().div_euclid(3600)
    }

    fn slot_to_suffix(hour_slot: i64) -> String {
        let slot_epoch = hour_slot.saturating_mul(3600);
        let ts = Utc
            .timestamp_opt(slot_epoch, 0)
            .single()
            .unwrap_or_else(Utc::now);
        ts.format("%Y-%m-%d-%H").to_string()
    }

    fn rotate_destination_with_limit(
        &self,
        hour_slot: i64,
        max_attempts: usize,
    ) -> std::io::Result<PathBuf> {
        let suffix = Self::slot_to_suffix(hour_slot);
        let base = self
            .directory
            .join(format!("{}.{}", self.file_name, suffix));
        if !base.exists() {
            return Ok(base);
        }
        for idx in 1..=max_attempts {
            let candidate = self
                .directory
                .join(format!("{}.{}.{}", self.file_name, suffix, idx));
            if !candidate.exists() {
                return Ok(candidate);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "failed to allocate rotated log path under {} after {} attempts",
                self.directory.display(),
                max_attempts
            ),
        ))
    }

    fn rotate_destination(&self, hour_slot: i64) -> std::io::Result<PathBuf> {
        self.rotate_destination_with_limit(hour_slot, Self::ROTATION_CANDIDATE_MAX_ATTEMPTS)
    }

    fn rotate_if_needed(&mut self) -> std::io::Result<()> {
        let now_slot = Self::hour_slot((self.now_utc)());
        if now_slot == self.current_hour_slot {
            return Ok(());
        }

        self.file.flush()?;
        let replacement = Self::open_append_file(&self.current_path)?;
        let previous_file = std::mem::replace(&mut self.file, replacement);
        drop(previous_file);

        if self.current_path.exists() {
            let rotated_path = self.rotate_destination(self.current_hour_slot)?;
            std::fs::rename(&self.current_path, rotated_path)?;
        }

        self.file = Self::open_append_file(&self.current_path)?;
        self.current_hour_slot = now_slot;
        Ok(())
    }
}

impl std::io::Write for ActiveHourlyLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.rotate_if_needed()?;
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn split_log_path(path: &str) -> LogSpec {
    let path_buf = std::path::Path::new(path);
    if path_buf.extension().is_some() {
        let file_name = path_buf
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("agenthub.log")
            .to_string();
        let dir = path_buf
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        return (dir.to_path_buf(), file_name);
    }
    (path_buf.to_path_buf(), "agenthub.log".to_string())
}

fn init_tracing(
    filter: EnvFilter,
    log_spec: Option<&LogSpec>,
) -> anyhow::Result<Option<LogGuards>> {
    if let Some((dir, file_name)) = log_spec {
        let appender = ActiveHourlyLogWriter::new(dir.clone(), file_name.clone())?;
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(writer)
            .with_ansi(false)
            .with_target(true)
            .try_init();
        return Ok(Some(LogGuards { _writer: guard }));
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
    Ok(None)
}

fn log_config_details(
    config: &crate::config::AppConfig,
    info: &crate::config::ConfigLoadInfo,
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
    tracing::info!("config rp_id: {}", config.rp_id());
    tracing::info!("config rp_origin: {}", config.rp_origin());
    tracing::info!("config rp_name: {}", config.rp_name());
    let configured_web_dir = config
        .web_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !cfg!(debug_assertions) && config.web_dir.is_some() {
        tracing::info!("config web_dir ignored in release build");
    }
    tracing::info!(
        "config web_dir: {}",
        configured_web_dir.unwrap_or("<unset>")
    );
    tracing::info!("effective web_dir: {}", web_dir.unwrap_or("embedded"));
    tracing::info!("config codex_acp_binary: {}", config.codex_acp_binary());
    tracing::info!(
        "config codex_acp_default_mode: {}",
        config
            .codex_acp_default_mode()
            .as_deref()
            .unwrap_or("<unset>")
    );
    tracing::info!("config vapid_subject: {}", config.vapid_subject());
    tracing::info!(
        "config vapid_keys_path: {}",
        config.vapid_keys_path().display()
    );
    tracing::info!("config safe_paths: {}", config.safe_paths().len());
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
        let web_service = ServeDir::new(dir)
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(format!("{}/index.html", dir)));
        app = app.fallback_service(web_service);
    } else {
        tracing::info!("serving web from embedded assets");
        app = app.fallback(crate::web::embedded_handler);
    }
    app
}

pub async fn run() -> anyhow::Result<()> {
    if let Some(result) = crate::actor_mcp::maybe_run_from_args().await {
        return result;
    }
    if let Some(result) = crate::actor_cli::maybe_run_from_args().await {
        return result;
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (config, info) = crate::config::AppConfig::load_with_info()?;
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
    let state = crate::state::AppState::init(config.clone()).await?;
    let _internal_grpc = crate::internal::maybe_spawn_internal_grpc(state.clone(), &config).await?;

    let api_router = crate::api::router(state.clone());
    let app = build_app_router(state.clone(), api_router, web_dir.as_deref());

    let addr: SocketAddr = config.listen_addr().parse()?;

    tracing::info!("listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use chrono::{TimeZone, Utc};
    use tower::util::ServiceExt;
    use tracing_subscriber::EnvFilter;

    #[test]
    fn split_log_path_detects_file_and_directory_inputs() {
        let (dir, file) = super::split_log_path("/tmp/agenthub/service.log");
        assert!(dir.ends_with("/tmp/agenthub"));
        assert_eq!(file, "service.log");

        let (dir, file) = super::split_log_path("/tmp/agenthub/logs");
        assert!(dir.ends_with("/tmp/agenthub/logs"));
        assert_eq!(file, "agenthub.log");
    }

    #[test]
    fn log_config_details_handles_all_branches() {
        let config = crate::config::AppConfig::default();
        let file_info = crate::config::ConfigLoadInfo {
            path: std::path::PathBuf::from("/tmp/agenthub/config.toml"),
            file_exists: true,
            env_overrides: vec!["AGENTHUB_LISTEN".to_string()],
        };
        let default_info = crate::config::ConfigLoadInfo {
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
    fn init_tracing_supports_stdout_and_file_targets() {
        let stdout_guard =
            super::init_tracing(EnvFilter::new("info"), None).expect("init tracing for stdout");
        assert!(stdout_guard.is_none());

        let unique = format!(
            "agenthub-app-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        let spec = (dir.clone(), "agenthub.log".to_string());
        let file_guard = super::init_tracing(EnvFilter::new("info"), Some(&spec))
            .expect("init tracing for file");
        assert!(file_guard.is_some());
        assert!(dir.exists());
        assert!(dir.join("agenthub.log").exists());
    }

    #[test]
    fn active_log_writer_keeps_latest_plain_and_rotates_with_suffix() {
        let unique = format!(
            "agenthub-log-rotate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create temp log dir");

        let hour_slot = Arc::new(AtomicI64::new(48_000));
        let hour_slot_for_clock = Arc::clone(&hour_slot);
        let clock = Box::new(move || {
            let slot = hour_slot_for_clock.load(Ordering::Relaxed);
            Utc.timestamp_opt(slot.saturating_mul(3600), 0)
                .single()
                .expect("valid slot timestamp")
        });
        let mut writer = super::ActiveHourlyLogWriter::new_with_clock(
            dir.clone(),
            "agenthub.log".to_string(),
            clock,
        )
        .expect("create writer");

        writer.write_all(b"line-1\n").expect("write first log line");

        let current_log = dir.join("agenthub.log");
        assert!(current_log.exists());
        let first_content = std::fs::read_to_string(&current_log).expect("read current log");
        assert_eq!(first_content, "line-1\n");

        let old_slot = hour_slot.load(Ordering::Relaxed);
        hour_slot.store(old_slot + 1, Ordering::Relaxed);
        writer
            .write_all(b"line-2\n")
            .expect("write second log line with rotation");

        let rotated = dir.join(format!(
            "agenthub.log.{}",
            super::ActiveHourlyLogWriter::slot_to_suffix(old_slot)
        ));
        assert!(rotated.exists());
        let rotated_content = std::fs::read_to_string(rotated).expect("read rotated log");
        assert_eq!(rotated_content, "line-1\n");

        let current_content = std::fs::read_to_string(current_log).expect("read current log");
        assert_eq!(current_content, "line-2\n");
    }

    #[test]
    fn rotate_destination_with_limit_returns_error_after_exhaustion() {
        let unique = format!(
            "agenthub-log-rotate-limit-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create temp log dir");

        let hour_slot = 48_001;
        let suffix = super::ActiveHourlyLogWriter::slot_to_suffix(hour_slot);
        let base = dir.join(format!("agenthub.log.{}", suffix));
        std::fs::write(&base, "x").expect("write base collision file");
        std::fs::write(dir.join(format!("agenthub.log.{}.1", suffix)), "x")
            .expect("write first collision file");
        std::fs::write(dir.join(format!("agenthub.log.{}.2", suffix)), "x")
            .expect("write second collision file");

        let writer = super::ActiveHourlyLogWriter::new(dir.clone(), "agenthub.log".to_string())
            .expect("create writer");
        let err = writer
            .rotate_destination_with_limit(hour_slot, 2)
            .expect_err("rotate destination should fail after hitting max attempts");
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
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

        let state = crate::api::team_tests::build_test_state().await;
        let api_router = crate::api::router(state.clone());
        let app = super::build_app_router(state, api_router, Some(dir.to_string_lossy().as_ref()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/not-found-path")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request fallback path");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body_text = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert!(body_text.contains("agenthub-test"));
    }
}
