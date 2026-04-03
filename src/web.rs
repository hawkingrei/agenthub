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
    if is_hashed_asset_request_path(path) {
        WEB_IMMUTABLE_ASSET_CACHE
    } else {
        WEB_NO_CACHE
    }
}

fn is_hashed_asset_request_path(path: &str) -> bool {
    let normalized = path.trim_start_matches('/');
    let normalized = normalized.strip_prefix("dist/").unwrap_or(normalized);
    if !normalized.starts_with("assets/") {
        return false;
    }
    let Some(file_name) = Path::new(normalized)
        .file_name()
        .and_then(|value| value.to_str())
    else {
        return false;
    };
    let Some((stem, _extension)) = file_name.rsplit_once('.') else {
        return false;
    };
    let Some((_prefix, hash_suffix)) = stem.rsplit_once('-') else {
        return false;
    };
    hash_suffix.len() >= 6
        && hash_suffix
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '_' || value == '-')
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
    if !requested.starts_with("dist/") {
        vec![format!("dist/{requested}"), requested.to_string()]
    } else {
        vec![requested.to_string()]
    }
}

fn fallback_web_paths() -> [&'static str; 2] {
    ["dist/index.html", "index.html"]
}

fn should_fallback_to_shell(path: &str) -> bool {
    let normalized = path.trim_start_matches('/');
    if normalized.is_empty() {
        return true;
    }
    let normalized = normalized.strip_prefix("dist/").unwrap_or(normalized);
    if normalized.starts_with("assets/") {
        return false;
    }
    Path::new(normalized).extension().is_none()
}

fn web_paths_to_try(path: &str) -> Vec<String> {
    let normalized = path.trim_start_matches('/');
    let mut paths = candidate_web_paths(normalized);
    if should_fallback_to_shell(normalized) {
        for fallback in fallback_web_paths() {
            if paths.iter().any(|candidate| candidate == fallback) {
                continue;
            }
            paths.push(fallback.to_string());
        }
    }
    paths
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

fn no_cache_text_response(status: StatusCode, body: &'static str) -> Response {
    let mut response = (status, body).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(WEB_NO_CACHE),
    );
    response
}

pub async fn dir_handler(base_dir: Arc<PathBuf>, uri: Uri) -> Response {
    serve_dir_fallback(base_dir.as_path(), uri).await
}

async fn serve_dir_fallback(base_dir: &Path, uri: Uri) -> Response {
    let request_path = uri.path().trim_start_matches('/');
    for candidate in web_paths_to_try(request_path) {
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
                return no_cache_text_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to read web asset",
                );
            }
        }
    }

    no_cache_text_response(StatusCode::NOT_FOUND, "not found")
}

#[cfg(not(debug_assertions))]
pub async fn embedded_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidates = web_paths_to_try(path);

    let resolved = candidates
        .iter()
        .find_map(|candidate| EmbeddedWeb::get(candidate).map(|content| (content, candidate)));
    let Some((content, mime_path)) = resolved else {
        return no_cache_text_response(StatusCode::NOT_FOUND, "not found");
    };

    let mut response = Response::new(Body::from(content.data.into_owned()));
    apply_web_response_headers(&mut response, path, mime_path);
    response
}

#[cfg(debug_assertions)]
pub async fn embedded_handler(_uri: Uri) -> impl IntoResponse {
    no_cache_text_response(
        StatusCode::NOT_FOUND,
        "embedded assets are disabled in debug builds",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        WEB_IMMUTABLE_ASSET_CACHE, WEB_NO_CACHE, web_cache_control_for_request_path,
        web_paths_to_try,
    };

    #[test]
    fn web_cache_control_uses_immutable_for_hashed_asset_requests_only() {
        assert_eq!(
            web_cache_control_for_request_path("/assets/index-abc123.js"),
            WEB_IMMUTABLE_ASSET_CACHE
        );
        assert_eq!(
            web_cache_control_for_request_path("/dist/assets/vendor-C8LL_u4z.css"),
            WEB_IMMUTABLE_ASSET_CACHE
        );
        assert_eq!(
            web_cache_control_for_request_path("/assets/runtime.js"),
            WEB_NO_CACHE
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

    #[test]
    fn web_paths_to_try_only_adds_shell_fallback_for_navigation_requests() {
        assert_eq!(web_paths_to_try(""), vec!["dist/index.html", "index.html"]);
        assert_eq!(
            web_paths_to_try("teams/example"),
            vec![
                "dist/teams/example".to_string(),
                "teams/example".to_string(),
                "dist/index.html".to_string(),
                "index.html".to_string(),
            ]
        );
        assert_eq!(
            web_paths_to_try("assets/app-OLDHASH.js"),
            vec![
                "dist/assets/app-OLDHASH.js".to_string(),
                "assets/app-OLDHASH.js".to_string(),
            ]
        );
    }
}
