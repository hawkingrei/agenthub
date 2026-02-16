#[cfg(not(debug_assertions))]
use axum::body::Body;
#[cfg(not(debug_assertions))]
use axum::http::header;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;

#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;

#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "web"]
struct EmbeddedWeb;

#[cfg(not(debug_assertions))]
pub async fn embedded_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    let mut candidates = vec![path.to_string()];
    if !path.starts_with("dist/") {
        candidates.push(format!("dist/{path}"));
    }

    let hit = candidates
        .iter()
        .find_map(|candidate| EmbeddedWeb::get(candidate).map(|content| (content, candidate)));
    let (content, mime_path) = if let Some((content, mime_path)) = hit {
        (content, mime_path.as_str())
    } else if let Some(index) = EmbeddedWeb::get("dist/index.html") {
        (index, "dist/index.html")
    } else if let Some(index) = EmbeddedWeb::get("index.html") {
        (index, "index.html")
    } else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    let mime = mime_guess::from_path(mime_path).first_or_octet_stream();
    let mut resp = axum::response::Response::new(Body::from(content.data.into_owned()));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_str(mime.as_ref())
            .unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
    );
    resp
}

#[cfg(debug_assertions)]
pub async fn embedded_handler(_uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "embedded assets are disabled in debug builds",
    )
        .into_response()
}
