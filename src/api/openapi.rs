use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
};
use serde_json::Value;

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;

mod spec;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/openapi.json", get(get_openapi_json))
        .route("/openapi/docs", get(get_openapi_docs))
        .with_state(state)
}

async fn get_openapi_json(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    Ok(Json(openapi_spec()))
}

async fn get_openapi_docs() -> impl IntoResponse {
    Html(OPENAPI_DOCS_HTML)
}

fn openapi_spec() -> Value {
    spec::openapi_spec()
}

const OPENAPI_DOCS_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>AgentHub OpenAPI</title>
    <style>
      body {
        margin: 0;
        font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
        background: #0f1115;
        color: #e6ebf2;
      }
      header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        padding: 12px 16px;
        border-bottom: 1px solid #2a3140;
        background: #141925;
      }
      h1 {
        margin: 0;
        font-size: 16px;
      }
      .actions {
        display: flex;
        gap: 8px;
      }
      button {
        background: #2d3a52;
        color: #fff;
        border: 1px solid #42557a;
        border-radius: 8px;
        padding: 6px 10px;
        cursor: pointer;
      }
      .hint {
        padding: 10px 16px;
        border-bottom: 1px solid #2a3140;
        color: #c4cfdf;
        font-size: 13px;
      }
      pre {
        margin: 0;
        padding: 16px;
        overflow: auto;
        font-size: 12px;
        line-height: 1.5;
      }
      code {
        font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      }
      .error {
        color: #ff8b8b;
      }
    </style>
  </head>
  <body>
    <header>
      <h1>AgentHub OpenAPI</h1>
      <div class="actions">
        <button id="reload">Reload</button>
        <button id="copy">Copy JSON</button>
      </div>
    </header>
    <div class="hint">
      Reads token from <code>localStorage.agenthub_auth</code> and requests <code>/api/openapi.json</code>.
    </div>
    <pre id="output">Loading...</pre>
    <script>
      let currentJson = "";

      function getToken() {
        try {
          const raw = localStorage.getItem("agenthub_auth");
          if (!raw) return null;
          const parsed = JSON.parse(raw);
          if (parsed && typeof parsed.token === "string" && parsed.token) {
            return parsed.token;
          }
        } catch (_) {}
        return null;
      }

      async function loadSpec() {
        const output = document.getElementById("output");
        output.textContent = "Loading...";
        const token = getToken();
        const headers = token ? { Authorization: `Bearer ${token}` } : {};
        try {
          const resp = await fetch("/api/openapi.json", { headers });
          if (!resp.ok) {
            const text = await resp.text();
            output.textContent = `HTTP ${resp.status}\n${text}`;
            currentJson = "";
            return;
          }
          const body = await resp.json();
          currentJson = JSON.stringify(body, null, 2);
          output.textContent = currentJson;
        } catch (err) {
          output.textContent = String(err);
          currentJson = "";
        }
      }

      document.getElementById("reload").addEventListener("click", loadSpec);
      document.getElementById("copy").addEventListener("click", async () => {
        if (!currentJson) return;
        try {
          await navigator.clipboard.writeText(currentJson);
        } catch (_) {}
      });

      loadSpec();
    </script>
  </body>
</html>
"#;

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header},
    };
    use serde_json::Value;
    use tower::util::ServiceExt;

    use crate::api::teams::tests::build_test_state;

    #[tokio::test]
    async fn openapi_json_requires_authorization() {
        let state = build_test_state().await;
        let app = super::router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/openapi.json")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("request openapi without auth");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn openapi_json_contains_team_runs_list_path() {
        let state = build_test_state().await;
        let token = crate::api::teams::tests::create_auth_token(&state).await;
        let app = super::router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/openapi.json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("build authorized request"),
            )
            .await
            .expect("request openapi with auth");
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let value: Value = serde_json::from_slice(&bytes).expect("decode openapi json");
        assert_eq!(value["openapi"], Value::from("3.0.3"));
        assert!(value["paths"]["/api/teams/{id}"]["delete"].is_object());
        assert!(value["paths"]["/api/teams/{id}/runs"].is_object());
        assert!(value["paths"]["/api/teams/runs/{run_id}/resume"].is_object());
        assert!(value["paths"]["/api/teams/runs/{run_id}/restart"].is_object());
        assert!(value["paths"]["/api/teams/runs/{run_id}/snapshot"].is_object());
    }
}
