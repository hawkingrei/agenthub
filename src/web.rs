use axum::body::Body;
use axum::http::{StatusCode, Uri, header};
use axum::response::IntoResponse;

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct EmbeddedWeb;

pub async fn embedded_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let file = EmbeddedWeb::get(path);
    let (content, mime_path) = if let Some(content) = file {
        (content, path)
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
