use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse},
    routing::get,
};
use serde_json::{Value, json};

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;

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
    json!({
      "openapi": "3.0.3",
      "info": {
        "title": "AgentHub HTTP API",
        "version": "0.1.0",
        "description": "AgentHub API contract. Current OpenAPI focus covers Team workbench endpoints and can be incrementally extended."
      },
      "servers": [
        { "url": "/" }
      ],
      "tags": [
        { "name": "openapi", "description": "OpenAPI discovery endpoints" },
        { "name": "teams", "description": "Team definitions, runs, steps, and actor mailbox" }
      ],
      "components": {
        "securitySchemes": {
          "bearerAuth": {
            "type": "http",
            "scheme": "bearer",
            "bearerFormat": "JWT"
          }
        },
        "schemas": {
          "ApiError": {
            "type": "object",
            "required": ["error"],
            "properties": {
              "error": { "type": "string" }
            }
          },
          "TeamDefinitionRecord": {
            "type": "object",
            "required": ["id", "name", "spec", "created_at", "updated_at"],
            "properties": {
              "id": { "type": "string" },
              "name": { "type": "string" },
              "description": { "type": ["string", "null"] },
              "spec": { "type": "object", "additionalProperties": true },
              "created_at": { "type": "integer", "format": "int64" },
              "updated_at": { "type": "integer", "format": "int64" }
            }
          },
          "TeamRunRecord": {
            "type": "object",
            "required": ["id", "team_id", "context_id", "status", "input", "created_at"],
            "properties": {
              "id": { "type": "string" },
              "team_id": { "type": "string" },
              "context_id": { "type": "string" },
              "status": {
                "type": "string",
                "enum": ["submitted", "working", "input_required", "completed", "failed", "canceled"]
              },
              "input": { "type": "object", "additionalProperties": true },
              "created_at": { "type": "integer", "format": "int64" },
              "started_at": { "type": ["integer", "null"], "format": "int64" },
              "ended_at": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "TeamRunEventRecord": {
            "type": "object",
            "required": ["event_id", "run_id", "event_type", "ts", "payload"],
            "properties": {
              "event_id": { "type": "integer", "format": "int64" },
              "run_id": { "type": "string" },
              "step_id": { "type": ["string", "null"] },
              "event_type": { "type": "string" },
              "ts": { "type": "integer", "format": "int64" },
              "payload": { "type": "object", "additionalProperties": true }
            }
          },
          "TeamStepRecord": {
            "type": "object",
            "required": [
              "id", "run_id", "step_key", "member_id", "status", "attempt", "depends_on"
            ],
            "properties": {
              "id": { "type": "string" },
              "run_id": { "type": "string" },
              "step_key": { "type": "string" },
              "member_id": { "type": "string" },
              "remote_task_id": { "type": ["string", "null"] },
              "status": {
                "type": "string",
                "enum": ["submitted", "working", "input_required", "completed", "failed", "canceled"]
              },
              "attempt": { "type": "integer", "format": "int64" },
              "depends_on": { "type": "array", "items": { "type": "string" } },
              "input": { "type": ["object", "null"], "additionalProperties": true },
              "output": { "type": ["object", "null"], "additionalProperties": true },
              "error_text": { "type": ["string", "null"] },
              "started_at": { "type": ["integer", "null"], "format": "int64" },
              "ended_at": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "TeamActorMessageRecord": {
            "type": "object",
            "required": [
              "message_id", "run_id", "from_actor_id", "to_actor_id", "channel", "transport", "payload", "status", "created_at"
            ],
            "properties": {
              "message_id": { "type": "integer", "format": "int64" },
              "run_id": { "type": "string" },
              "from_actor_id": { "type": "string" },
              "to_actor_id": { "type": "string" },
              "channel": { "type": "string" },
              "transport": { "type": "string", "enum": ["local", "remote"] },
              "route": { "type": ["object", "null"], "additionalProperties": true },
              "payload": { "type": "object", "additionalProperties": true },
              "status": { "type": "string", "enum": ["pending", "delivered", "dead_letter"] },
              "created_at": { "type": "integer", "format": "int64" },
              "delivered_at": { "type": ["integer", "null"], "format": "int64" }
            }
          },
          "CreateTeamRequest": {
            "type": "object",
            "required": ["name", "spec"],
            "properties": {
              "name": { "type": "string" },
              "description": { "type": ["string", "null"] },
              "spec": { "type": "object", "additionalProperties": true }
            }
          },
          "CreateTeamRunRequest": {
            "type": "object",
            "properties": {
              "context_id": { "type": ["string", "null"] },
              "input": { "type": ["object", "null"], "additionalProperties": true }
            }
          },
          "SubmitTeamRunStepRequest": {
            "type": "object",
            "required": ["step_key", "member_id"],
            "properties": {
              "step_key": { "type": "string" },
              "member_id": { "type": "string" },
              "depends_on": { "type": "array", "items": { "type": "string" } },
              "input": { "type": ["object", "null"], "additionalProperties": true }
            }
          },
          "SendTeamRunMessageRequest": {
            "type": "object",
            "required": ["from_actor_id", "to_actor_id", "payload"],
            "properties": {
              "from_actor_id": { "type": "string" },
              "to_actor_id": { "type": "string" },
              "channel": { "type": ["string", "null"] },
              "transport": { "type": ["string", "null"], "enum": ["local", "remote", null] },
              "route": { "type": ["object", "null"], "additionalProperties": true },
              "payload": { "type": "object", "additionalProperties": true },
              "idempotency_key": { "type": ["string", "null"] }
            }
          }
        }
      },
      "security": [
        { "bearerAuth": [] }
      ],
      "paths": {
        "/api/openapi.json": {
          "get": {
            "tags": ["openapi"],
            "summary": "Get OpenAPI JSON",
            "responses": {
              "200": {
                "description": "OpenAPI document",
                "content": {
                  "application/json": {
                    "schema": { "type": "object", "additionalProperties": true }
                  }
                }
              },
              "401": {
                "description": "Unauthorized",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiError" }
                  }
                }
              }
            }
          }
        },
        "/api/openapi/docs": {
          "get": {
            "tags": ["openapi"],
            "summary": "Get OpenAPI docs page",
            "security": [],
            "responses": {
              "200": {
                "description": "HTML docs page",
                "content": {
                  "text/html": {
                    "schema": { "type": "string" }
                  }
                }
              }
            }
          }
        },
        "/api/teams": {
          "get": {
            "tags": ["teams"],
            "summary": "List teams",
            "responses": {
              "200": {
                "description": "Team definitions",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Create team",
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateTeamRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Created team",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}": {
          "get": {
            "tags": ["teams"],
            "summary": "Get team",
            "parameters": [
              {
                "name": "id",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
              }
            ],
            "responses": {
              "200": {
                "description": "Team definition",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamDefinitionRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/{id}/runs": {
          "get": {
            "tags": ["teams"],
            "summary": "List runs for a team",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 500 } },
              {
                "name": "status",
                "in": "query",
                "schema": {
                  "type": "string",
                  "enum": ["submitted", "working", "input_required", "completed", "failed", "canceled"]
                }
              },
              { "name": "before_created_at", "in": "query", "schema": { "type": "integer", "format": "int64" } }
            ],
            "responses": {
              "200": {
                "description": "Team runs",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamRunRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Create run for a team",
            "parameters": [
              { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateTeamRunRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Created run",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}": {
          "get": {
            "tags": ["teams"],
            "summary": "Get run",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Run record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/cancel": {
          "post": {
            "tags": ["teams"],
            "summary": "Cancel run",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Canceled run",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamRunRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/events": {
          "get": {
            "tags": ["teams"],
            "summary": "List run events",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 1000 } },
              { "name": "before_id", "in": "query", "schema": { "type": "integer", "format": "int64" } }
            ],
            "responses": {
              "200": {
                "description": "Run events",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamRunEventRecord" }
                    }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps": {
          "get": {
            "tags": ["teams"],
            "summary": "List run steps",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Run steps",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamStepRecord" }
                    }
                  }
                }
              }
            }
          },
          "post": {
            "tags": ["teams"],
            "summary": "Submit run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/SubmitTeamRunStepRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/start": {
          "post": {
            "tags": ["teams"],
            "summary": "Start run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/complete": {
          "post": {
            "tags": ["teams"],
            "summary": "Complete run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/fail": {
          "post": {
            "tags": ["teams"],
            "summary": "Fail run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/input_required": {
          "post": {
            "tags": ["teams"],
            "summary": "Set run step input required",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/steps/{step_id}/resume": {
          "post": {
            "tags": ["teams"],
            "summary": "Resume run step",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "step_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": {
                "description": "Step record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamStepRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/messages/send": {
          "post": {
            "tags": ["teams"],
            "summary": "Send actor message",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/SendTeamRunMessageRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Actor message record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/messages/inbox": {
          "get": {
            "tags": ["teams"],
            "summary": "List actor inbox",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "actor_id", "in": "query", "required": true, "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 1000 } },
              { "name": "after_id", "in": "query", "schema": { "type": "integer", "format": "int64" } },
              { "name": "include_delivered", "in": "query", "schema": { "type": "boolean" } }
            ],
            "responses": {
              "200": {
                "description": "Actor inbox",
                "content": {
                  "application/json": {
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                    }
                  }
                }
              }
            }
          }
        },
        "/api/teams/runs/{run_id}/messages/{message_id}/ack": {
          "post": {
            "tags": ["teams"],
            "summary": "Acknowledge actor message",
            "parameters": [
              { "name": "run_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "message_id", "in": "path", "required": true, "schema": { "type": "integer", "format": "int64" } }
            ],
            "responses": {
              "200": {
                "description": "Actor message record",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/TeamActorMessageRecord" }
                  }
                }
              }
            }
          }
        }
      }
    })
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
            output.innerHTML = `<span class="error">HTTP ${resp.status}</span>\n${text}`;
            currentJson = "";
            return;
          }
          const body = await resp.json();
          currentJson = JSON.stringify(body, null, 2);
          output.textContent = currentJson;
        } catch (err) {
          output.innerHTML = `<span class="error">${String(err)}</span>`;
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
        assert!(value["paths"]["/api/teams/{id}/runs"].is_object());
    }
}
