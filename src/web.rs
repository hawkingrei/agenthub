use axum::body::Body;
use axum::http::header;
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;

const WEB_NO_CACHE: &str = "no-cache";
const WEB_IMMUTABLE_ASSET_CACHE: &str = "public, max-age=31536000, immutable";

#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "web"]
struct EmbeddedWeb;

fn web_cache_control_for_request_path(path: &str) -> &'static str {
    if path.trim_start_matches('/').starts_with("assets/") {
        WEB_IMMUTABLE_ASSET_CACHE
    } else {
        WEB_NO_CACHE
    }
}

fn sanitize_relative_web_path(path: &str) -> Option<PathBuf> {
    let mut sanitized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    Some(sanitized)
}

fn candidate_web_paths(path: &str) -> Vec<String> {
    let requested = if path.is_empty() { "index.html" } else { path };
    let mut candidates = vec![requested.to_string()];
    if !requested.starts_with("dist/") {
        candidates.push(format!("dist/{requested}"));
    }
    candidates
}

fn fallback_web_paths() -> [&'static str; 2] {
    ["dist/index.html", "index.html"]
}

fn web_mime_for_path(path: &str) -> &'static str {
    if path.ends_with(".webmanifest") {
        "application/manifest+json"
    } else {
        mime_guess::from_path(path)
            .first_raw()
            .unwrap_or("application/octet-stream")
    }
}

fn apply_web_response_headers(response: &mut Response, request_path: &str, mime_path: &str) {
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(web_mime_for_path(mime_path))
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(web_cache_control_for_request_path(request_path)),
    );
}

pub async fn dir_handler(base_dir: Arc<PathBuf>, uri: Uri) -> Response {
    serve_dir_fallback(base_dir.as_path(), uri).await
}

async fn serve_dir_fallback(base_dir: &Path, uri: Uri) -> Response {
    let request_path = uri.path().trim_start_matches('/');
    for candidate in candidate_web_paths(request_path) {
        let Some(sanitized_candidate) = sanitize_relative_web_path(&candidate) else {
            continue;
        };
        let candidate_path = base_dir.join(&sanitized_candidate);
        match tokio::fs::read(&candidate_path).await {
            Ok(content) => {
                let mut response = Response::new(Body::from(content));
                apply_web_response_headers(&mut response, request_path, &candidate);
                return response;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to read web asset",
                )
                    .into_response();
            }
        }
    }

    for fallback in fallback_web_paths() {
        let Some(sanitized_fallback) = sanitize_relative_web_path(fallback) else {
            continue;
        };
        let fallback_path = base_dir.join(sanitized_fallback);
        match tokio::fs::read(&fallback_path).await {
            Ok(content) => {
                let mut response = Response::new(Body::from(content));
                apply_web_response_headers(&mut response, request_path, fallback);
                return response;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to read web asset",
                )
                    .into_response();
            }
        }
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

#[cfg(not(debug_assertions))]
pub async fn embedded_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidates = candidate_web_paths(path);

    let hit = candidates
        .iter()
        .find_map(|candidate| EmbeddedWeb::get(candidate).map(|content| (content, candidate)));
    let resolved = if let Some((content, mime_path)) = hit {
        Some((content, mime_path.as_str()))
    } else {
        fallback_web_paths()
            .into_iter()
            .find_map(|fallback| EmbeddedWeb::get(fallback).map(|content| (content, fallback)))
    };
    let Some((content, mime_path)) = resolved else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    let mut response = Response::new(Body::from(content.data.into_owned()));
    apply_web_response_headers(&mut response, path, mime_path);
    response
}

#[cfg(debug_assertions)]
pub async fn embedded_handler(_uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "embedded assets are disabled in debug builds",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{WEB_IMMUTABLE_ASSET_CACHE, WEB_NO_CACHE, web_cache_control_for_request_path};

    #[test]
    fn web_cache_control_uses_immutable_for_hashed_assets_only() {
        assert_eq!(
            web_cache_control_for_request_path("/assets/index-abc123.js"),
            WEB_IMMUTABLE_ASSET_CACHE
        );
        assert_eq!(
            web_cache_control_for_request_path("/teams/example"),
            WEB_NO_CACHE
        );
        assert_eq!(
            web_cache_control_for_request_path("/manifest.webmanifest"),
            WEB_NO_CACHE
        );
        assert_eq!(web_cache_control_for_request_path("/sw.js"), WEB_NO_CACHE);
    }
}
