use std::net::SocketAddr;

use axum::{Router, routing::get};
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

mod acp;
mod agent;
mod api;
mod auth;
mod config;
mod db;
mod push;
mod sse;
mod state;
mod web;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let (config, info) = config::AppConfig::load_with_info()?;
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
    tracing::info!("config rp_id: {}", config.rp_id());
    tracing::info!("config rp_origin: {}", config.rp_origin());
    tracing::info!("config rp_name: {}", config.rp_name());
    let configured_web_dir = config
        .web_dir
        .clone()
        .unwrap_or_else(|| "embedded".to_string());
    let web_dir = config.effective_web_dir();
    if !cfg!(debug_assertions) && config.web_dir.is_some() {
        tracing::info!("config web_dir ignored in release build");
    }
    tracing::info!("config web_dir: {}", configured_web_dir);
    tracing::info!(
        "effective web_dir: {}",
        web_dir.clone().unwrap_or_else(|| "embedded".to_string())
    );
    tracing::info!("config codex_acp_binary: {}", config.codex_acp_binary());
    tracing::info!("config vapid_subject: {}", config.vapid_subject());
    tracing::info!(
        "config vapid_keys_path: {}",
        config.vapid_keys_path().display()
    );
    tracing::info!("config safe_paths: {}", config.safe_paths().len());
    let state = state::AppState::init(config.clone()).await?;

    let api_router = api::router(state.clone());

    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        .on_failure(DefaultOnFailure::new().level(Level::ERROR));
    let compression = CompressionLayer::new().gzip(true);

    let mut app = Router::new()
        .route("/health", get(api::health))
        .nest("/api", api_router)
        .nest("/sse", sse::router(state.clone()))
        .layer(trace)
        .layer(compression);
    if let Some(dir) = web_dir {
        tracing::info!("serving web from dir: {}", dir);
        let web_service = ServeDir::new(&dir)
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(format!("{}/index.html", dir)));
        app = app.fallback_service(web_service);
    } else {
        tracing::info!("serving web from embedded assets");
        app = app.fallback(web::embedded_handler);
    }

    let addr: SocketAddr = config.listen_addr().parse()?;

    tracing::info!("listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
