use std::collections::{HashMap, HashSet};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::Error as SqlxError;

use crate::api::authz::require_user;
use crate::api::error::ApiError;
use crate::state::AppState;
use crate::team::{TeamDefinitionConfig, TeamDefinitionRecord, TeamRunEventRecord, TeamRunRecord};

const TEAM_SPEC_VERSION_V1: i64 = 1;

#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub description: Option<String>,
    pub spec: Value,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamRunRequest {
    pub context_id: Option<String>,
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListTeamRunEventsQuery {
    pub limit: Option<i64>,
    pub before_id: Option<i64>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", post(create_team).get(list_teams))
        .route("/:id", get(get_team))
        .route("/:id/runs", post(create_team_run))
        .route("/runs/:run_id", get(get_team_run))
        .route("/runs/:run_id/cancel", post(cancel_team_run))
        .route("/runs/:run_id/events", get(list_team_run_events))
        .with_state(state)
}

async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("team name is required"));
    }
    let mut spec = payload.spec;
    normalize_team_spec(&mut spec)?;
    validate_team_spec(&spec)?;
    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name,
            description: payload.description,
            spec,
        })
        .await
        .map_err(map_create_team_error)?;
    Ok(Json(team))
}

async fn list_teams(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TeamDefinitionRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let teams = state.teams.list_teams().await?;
    Ok(Json(teams))
}

async fn get_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
) -> Result<Json<TeamDefinitionRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let team = state
        .teams
        .get_team(&team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    Ok(Json(team))
}

async fn create_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(team_id): Path<String>,
    Json(payload): Json<CreateTeamRunRequest>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let team = state
        .teams
        .get_team(&team_id)
        .await
        .map_err(|err| map_not_found_error(err, "team not found"))?;
    validate_team_spec(&team.spec)?;
    let run = state
        .teams
        .create_run(
            &team.id,
            payload.context_id.as_deref(),
            payload.input.unwrap_or_else(|| serde_json::json!({})),
        )
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(run))
}

async fn get_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let run = state
        .teams
        .get_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    Ok(Json(run))
}

async fn cancel_team_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<TeamRunRecord>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    let run = state
        .teams
        .cancel_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    Ok(Json(run))
}

async fn list_team_run_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<ListTeamRunEventsQuery>,
) -> Result<Json<Vec<TeamRunEventRecord>>, ApiError> {
    let _user = require_user(&headers, &state).await?;
    state
        .teams
        .get_run(&run_id)
        .await
        .map_err(|err| map_not_found_error(err, "run not found"))?;
    let limit = query.limit.unwrap_or(500).clamp(1, 1000);
    let events = state
        .teams
        .list_run_events(&run_id, limit, query.before_id)
        .await
        .map_err(map_team_internal_error)?;
    Ok(Json(events))
}

fn map_create_team_error(err: anyhow::Error) -> ApiError {
    if is_unique_team_name_violation(&err) {
        return ApiError::conflict("team name already exists");
    }
    map_team_internal_error(err)
}

fn map_not_found_error(err: anyhow::Error, msg: &str) -> ApiError {
    if is_row_not_found(&err) {
        return ApiError::not_found(msg);
    }
    map_team_internal_error(err)
}

fn map_team_internal_error(err: anyhow::Error) -> ApiError {
    ApiError::from(err)
}

fn is_row_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<SqlxError>(),
        Some(SqlxError::RowNotFound)
    )
}

fn is_unique_team_name_violation(err: &anyhow::Error) -> bool {
    err.to_string()
        .contains("UNIQUE constraint failed: team_definitions.name")
}

#[derive(Debug)]
struct TeamStepSpec {
    step_key: String,
    member_id: String,
    depends_on: Vec<String>,
}

fn normalize_team_spec(spec: &mut Value) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let version = parse_team_spec_version(spec_obj.get("spec_version"))?;
    spec_obj.insert("spec_version".to_string(), Value::from(version));
    Ok(())
}

fn validate_team_spec(spec: &Value) -> Result<(), ApiError> {
    let spec_obj = spec
        .as_object()
        .ok_or_else(|| ApiError::bad_request("spec must be an object"))?;
    let _ = parse_team_spec_version(spec_obj.get("spec_version"))?;
    let entrypoint = spec_obj
        .get("entrypoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::bad_request("spec.entrypoint is required"))?;
    let member_ids = parse_member_ids(spec_obj.get("members"))?;

    if let Some(steps_value) = spec_obj.get("steps") {
        validate_steps(entrypoint, steps_value, &member_ids)?;
    } else if !member_ids.contains(entrypoint) {
        return Err(ApiError::bad_request(
            "spec.entrypoint must reference spec.members[].member_id when spec.steps is omitted",
        ));
    }

    Ok(())
}

fn parse_team_spec_version(version_value: Option<&Value>) -> Result<i64, ApiError> {
    let version = match version_value {
        None => TEAM_SPEC_VERSION_V1,
        Some(value) => value
            .as_i64()
            .ok_or_else(|| ApiError::bad_request("spec.spec_version must be an integer"))?,
    };
    if version != TEAM_SPEC_VERSION_V1 {
        return Err(ApiError::bad_request(
            "unsupported spec.spec_version; expected 1",
        ));
    }
    Ok(version)
}

fn parse_member_ids(members_value: Option<&Value>) -> Result<HashSet<String>, ApiError> {
    let members = members_value
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::bad_request("spec.members must be an array"))?;
    if members.is_empty() {
        return Err(ApiError::bad_request("spec.members must not be empty"));
    }

    let mut member_ids = HashSet::with_capacity(members.len());
    for member in members {
        let member = member
            .as_object()
            .ok_or_else(|| ApiError::bad_request("spec.members entries must be objects"))?;
        let member_id = member
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.members[].member_id is required"))?;
        if !member_ids.insert(member_id.to_string()) {
            return Err(ApiError::bad_request(
                "spec.members[].member_id must be unique",
            ));
        }
    }
    Ok(member_ids)
}

fn validate_steps(
    entrypoint: &str,
    steps_value: &Value,
    member_ids: &HashSet<String>,
) -> Result<(), ApiError> {
    let steps = steps_value
        .as_array()
        .ok_or_else(|| ApiError::bad_request("spec.steps must be an array"))?;
    if steps.is_empty() {
        return Err(ApiError::bad_request("spec.steps must not be empty"));
    }

    let mut step_specs = Vec::with_capacity(steps.len());
    let mut step_keys = HashSet::with_capacity(steps.len());
    for step in steps {
        let step = step
            .as_object()
            .ok_or_else(|| ApiError::bad_request("spec.steps entries must be objects"))?;
        let step_key = step
            .get("step_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.steps[].step_key is required"))?
            .to_string();
        if !step_keys.insert(step_key.clone()) {
            return Err(ApiError::bad_request(
                "spec.steps[].step_key must be unique",
            ));
        }

        let member_id = step
            .get("member_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::bad_request("spec.steps[].member_id is required"))?
            .to_string();

        let depends_on = match step.get("depends_on") {
            Some(depends_on_value) => {
                let depends_on = depends_on_value.as_array().ok_or_else(|| {
                    ApiError::bad_request("spec.steps[].depends_on must be an array")
                })?;
                let mut seen_depends = HashSet::with_capacity(depends_on.len());
                let mut keys = Vec::with_capacity(depends_on.len());
                for dep in depends_on {
                    let dep = dep
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            ApiError::bad_request(
                                "spec.steps[].depends_on entries must be non-empty strings",
                            )
                        })?;
                    if !seen_depends.insert(dep.to_string()) {
                        return Err(ApiError::bad_request(
                            "spec.steps[].depends_on must not contain duplicates",
                        ));
                    }
                    keys.push(dep.to_string());
                }
                keys
            }
            None => Vec::new(),
        };

        step_specs.push(TeamStepSpec {
            step_key,
            member_id,
            depends_on,
        });
    }

    if !step_keys.contains(entrypoint) {
        return Err(ApiError::bad_request(
            "spec.entrypoint must reference spec.steps[].step_key when spec.steps is provided",
        ));
    }

    for step in &step_specs {
        if !member_ids.contains(&step.member_id) {
            return Err(ApiError::bad_request(
                "spec.steps[].member_id must reference spec.members[].member_id",
            ));
        }
        for dep in &step.depends_on {
            if dep == &step.step_key {
                return Err(ApiError::bad_request(
                    "spec.steps[].depends_on must not include the step itself",
                ));
            }
            if !step_keys.contains(dep) {
                return Err(ApiError::bad_request(
                    "spec.steps[].depends_on must reference existing spec.steps[].step_key",
                ));
            }
        }
    }

    ensure_acyclic_steps(&step_specs)?;
    Ok(())
}

fn ensure_acyclic_steps(steps: &[TeamStepSpec]) -> Result<(), ApiError> {
    let mut graph: HashMap<&str, &[String]> = HashMap::with_capacity(steps.len());
    for step in steps {
        graph.insert(step.step_key.as_str(), &step.depends_on);
    }

    let mut marks: HashMap<&str, u8> = HashMap::with_capacity(steps.len());
    for key in graph.keys().copied() {
        if has_cycle(key, &graph, &mut marks) {
            return Err(ApiError::bad_request(
                "spec.steps must form an acyclic dependency graph",
            ));
        }
    }
    Ok(())
}

fn has_cycle<'a>(
    key: &'a str,
    graph: &HashMap<&'a str, &'a [String]>,
    marks: &mut HashMap<&'a str, u8>,
) -> bool {
    match marks.get(key).copied().unwrap_or(0) {
        1 => return true,
        2 => return false,
        _ => {}
    }

    marks.insert(key, 1);
    if let Some(depends_on) = graph.get(key) {
        for dep in *depends_on {
            if has_cycle(dep.as_str(), graph, marks) {
                return true;
            }
        }
    }
    marks.insert(key, 2);
    false
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Json;
    use axum::body::{Body, to_bytes};
    use axum::extract::{Path, Query, State};
    use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header};
    use axum::response::IntoResponse;
    use chrono::Utc;
    use serde_json::{Value, json};
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::acp::AcpPermissionService;
    use crate::agent::AgentManager;
    use crate::auth::AuthService;
    use crate::config::{AppConfig, PushConfig, WebConfig};
    use crate::push::PushService;
    use crate::state::AppState;
    use crate::team::TeamManager;

    use super::{
        CreateTeamRequest, CreateTeamRunRequest, ListTeamRunEventsQuery, cancel_team_run,
        create_team, create_team_run, get_team, get_team_run, list_team_run_events, list_teams,
    };

    async fn build_test_state() -> AppState {
        let db = create_test_db().await;
        init_test_schema(&db).await;
        let keys_dir = std::env::temp_dir().join(format!("agenthub-a2a-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&keys_dir).expect("create keys dir");
        let keys_path = keys_dir.join("vapid.json");
        let config = AppConfig {
            web: Some(WebConfig {
                rp_id: Some("localhost".to_string()),
                rp_origin: Some("http://localhost:8080".to_string()),
                rp_name: Some("AgentHub Test".to_string()),
            }),
            push: Some(PushConfig {
                subject: Some("mailto:test@example.com".to_string()),
                keys_path: Some(keys_path.to_string_lossy().to_string()),
            }),
            ..Default::default()
        };
        let push = Arc::new(PushService::new(db.clone(), &config).expect("create push service"));
        let auth = Arc::new(
            AuthService::new(db.clone(), &config)
                .await
                .expect("create auth"),
        );
        let permissions = Arc::new(AcpPermissionService::new(db.clone()));
        let agents = Arc::new(AgentManager::new(
            db.clone(),
            push.clone(),
            Vec::new(),
            "agenthub-codex-acp".to_string(),
            None,
            permissions.clone(),
            auth.clone(),
        ));
        let teams = Arc::new(TeamManager::new(db.clone()));
        AppState {
            db,
            agents,
            teams,
            push,
            auth,
            acp_permissions: permissions,
        }
    }

    async fn create_test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite")
    }

    async fn init_test_schema(db: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL,
                password_hash TEXT,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create users");

        sqlx::query(
            r#"
            CREATE TABLE auth_sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                revoked_at INTEGER,
                FOREIGN KEY(user_id) REFERENCES users(id)
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create auth_sessions");

        sqlx::query(
            r#"
            CREATE TABLE devices (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                user_agent TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_login_at INTEGER,
                FOREIGN KEY(user_id) REFERENCES users(id)
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create devices");

        sqlx::query(
            r#"
            CREATE TABLE team_definitions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                spec_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create team_definitions");

        sqlx::query(
            r#"
            CREATE TABLE team_runs (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                context_id TEXT NOT NULL,
                status TEXT NOT NULL,
                input_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                ended_at INTEGER,
                FOREIGN KEY(team_id) REFERENCES team_definitions(id)
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create team_runs");

        sqlx::query(
            r#"
            CREATE TABLE team_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_key TEXT NOT NULL,
                member_id TEXT NOT NULL,
                remote_task_id TEXT,
                status TEXT NOT NULL,
                attempt INTEGER NOT NULL DEFAULT 0,
                depends_on_json TEXT NOT NULL DEFAULT '[]',
                input_json TEXT,
                output_json TEXT,
                error_text TEXT,
                started_at INTEGER,
                ended_at INTEGER,
                UNIQUE(run_id, step_key, attempt),
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create team_steps");

        sqlx::query(
            r#"
            CREATE TABLE team_run_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                step_id TEXT,
                event_type TEXT NOT NULL,
                ts INTEGER NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(run_id) REFERENCES team_runs(id),
                FOREIGN KEY(step_id) REFERENCES team_steps(id)
            );
            "#,
        )
        .execute(db)
        .await
        .expect("create team_run_events");
    }

    async fn auth_headers(state: &AppState) -> HeaderMap {
        let token = create_auth_token(state).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("auth header"),
        );
        headers
    }

    async fn create_auth_token(state: &AppState) -> String {
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO users (id, username, display_name, role, password_hash, created_at)
            VALUES (?1, ?2, ?3, 'root', NULL, ?4)
            "#,
        )
        .bind(&user_id)
        .bind(format!("root-{}", Uuid::new_v4()))
        .bind("Root")
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert user");

        let token = state
            .auth
            .create_session(&user_id)
            .await
            .expect("create token");
        token
    }

    fn build_json_request(
        method: Method,
        path: &str,
        token: Option<&str>,
        payload: Option<Value>,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        match payload {
            Some(value) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
                .expect("build json request"),
            None => builder.body(Body::empty()).expect("build request"),
        }
    }

    async fn decode_json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&bytes).expect("decode json response")
    }

    #[tokio::test]
    async fn teams_api_requires_authorization() {
        let state = build_test_state().await;
        let err = list_teams(State(state), HeaderMap::new())
            .await
            .expect_err("should reject without auth");
        assert_eq!(err.into_response().status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn teams_api_create_list_get_and_reject_duplicate_name() {
        let state = build_test_state().await;
        let headers = auth_headers(&state).await;

        let Json(created) = create_team(
            State(state.clone()),
            headers.clone(),
            Json(CreateTeamRequest {
                name: "review-team".to_string(),
                description: Some("team for review".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            }),
        )
        .await
        .expect("create team");
        assert_eq!(created.spec["spec_version"], Value::from(1));

        let Json(listed) = list_teams(State(state.clone()), headers.clone())
            .await
            .expect("list teams");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let Json(found) = get_team(
            State(state.clone()),
            headers.clone(),
            Path(created.id.clone()),
        )
        .await
        .expect("get team");
        assert_eq!(found.name, "review-team");

        let err = create_team(
            State(state),
            headers,
            Json(CreateTeamRequest {
                name: "review-team".to_string(),
                description: None,
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            }),
        )
        .await
        .expect_err("duplicate team name should fail");
        assert_eq!(err.into_response().status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn teams_api_rejects_invalid_spec() {
        let state = build_test_state().await;
        let headers = auth_headers(&state).await;
        let invalid_specs = vec![
            json!("invalid"),
            json!({"entrypoint":"planner"}),
            json!({"entrypoint":"","members":[{"member_id":"planner"}]}),
            json!({"entrypoint":"planner","members":[]}),
            json!({"entrypoint":"planner","members":[{"member_id":"planner"},{"member_id":"planner"}]}),
            json!({"entrypoint":"missing","members":[{"member_id":"planner"}]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"missing"}]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner","depends_on":["step-b"]}]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner","depends_on":["step-a"]}]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner"},{"step_key":"step-b","member_id":"planner","depends_on":["step-a","step-a"]}]}),
            json!({"entrypoint":"step-a","members":[{"member_id":"planner"}],"steps":[{"step_key":"step-a","member_id":"planner","depends_on":["step-b"]},{"step_key":"step-b","member_id":"planner","depends_on":["step-a"]}]}),
            json!({"spec_version":"1","entrypoint":"planner","members":[{"member_id":"planner"}]}),
            json!({"spec_version":2,"entrypoint":"planner","members":[{"member_id":"planner"}]}),
        ];
        for (index, spec) in invalid_specs.into_iter().enumerate() {
            let err = create_team(
                State(state.clone()),
                headers.clone(),
                Json(CreateTeamRequest {
                    name: format!("invalid-team-{index}"),
                    description: None,
                    spec,
                }),
            )
            .await
            .expect_err("invalid team spec should fail");
            assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn teams_api_rejects_run_for_unsupported_stored_spec_version() {
        let state = build_test_state().await;
        let headers = auth_headers(&state).await;

        let team_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            INSERT INTO team_definitions (id, name, description, spec_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&team_id)
        .bind(format!("legacy-team-{}", Uuid::new_v4()))
        .bind("legacy unsupported spec")
        .bind(
            json!({
                "spec_version": 2,
                "entrypoint": "planner",
                "members": [{"member_id":"planner"}]
            })
            .to_string(),
        )
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert legacy team");

        let err = create_team_run(
            State(state),
            headers,
            Path(team_id),
            Json(CreateTeamRunRequest {
                context_id: Some("ctx-legacy".to_string()),
                input: Some(json!({"prompt":"run legacy"})),
            }),
        )
        .await
        .expect_err("unsupported stored spec should fail");
        assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn team_runs_api_supports_lifecycle_and_event_pagination() {
        let state = build_test_state().await;
        let headers = auth_headers(&state).await;

        let Json(team) = create_team(
            State(state.clone()),
            headers.clone(),
            Json(CreateTeamRequest {
                name: "run-team".to_string(),
                description: None,
                spec: json!({"entrypoint":"executor","members":[{"member_id":"executor"}]}),
            }),
        )
        .await
        .expect("create team");

        let Json(run) = create_team_run(
            State(state.clone()),
            headers.clone(),
            Path(team.id.clone()),
            Json(CreateTeamRunRequest {
                context_id: Some("ctx-a2a".to_string()),
                input: Some(json!({"prompt":"review plan"})),
            }),
        )
        .await
        .expect("create run");
        assert_eq!(run.status, crate::team::TeamRunStatus::Submitted);

        let Json(found_run) =
            get_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
                .await
                .expect("get run");
        assert_eq!(found_run.id, run.id);

        let Json(canceled) =
            cancel_team_run(State(state.clone()), headers.clone(), Path(run.id.clone()))
                .await
                .expect("cancel run");
        assert_eq!(canceled.status, crate::team::TeamRunStatus::Canceled);
        assert!(canceled.ended_at.is_some());

        let Json(events) = list_team_run_events(
            State(state.clone()),
            headers.clone(),
            Path(run.id.clone()),
            Query(ListTeamRunEventsQuery {
                limit: Some(100),
                before_id: None,
            }),
        )
        .await
        .expect("list events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "run_submitted");
        assert_eq!(events[1].event_type, "run_canceled");
        assert!(events[0].event_id < events[1].event_id);

        let Json(first_page) = list_team_run_events(
            State(state.clone()),
            headers.clone(),
            Path(run.id.clone()),
            Query(ListTeamRunEventsQuery {
                limit: Some(1),
                before_id: Some(events[1].event_id),
            }),
        )
        .await
        .expect("page events");
        assert_eq!(first_page.len(), 1);
        assert_eq!(first_page[0].event_type, "run_submitted");

        let missing_team_run_err = create_team_run(
            State(state.clone()),
            headers.clone(),
            Path("missing-team".to_string()),
            Json(CreateTeamRunRequest {
                context_id: None,
                input: Some(json!({})),
            }),
        )
        .await
        .expect_err("missing team");
        assert_eq!(
            missing_team_run_err.into_response().status(),
            StatusCode::NOT_FOUND
        );

        let missing_run_err = get_team_run(
            State(state.clone()),
            headers.clone(),
            Path("missing-run".to_string()),
        )
        .await
        .expect_err("missing run");
        assert_eq!(
            missing_run_err.into_response().status(),
            StatusCode::NOT_FOUND
        );

        let missing_events_err = list_team_run_events(
            State(state),
            headers,
            Path("missing-run".to_string()),
            Query(ListTeamRunEventsQuery {
                limit: None,
                before_id: None,
            }),
        )
        .await
        .expect_err("missing run events");
        assert_eq!(
            missing_events_err.into_response().status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn teams_router_http_contract() {
        let state = build_test_state().await;
        let token = create_auth_token(&state).await;
        let app = super::router(state);

        let unauthorized = app
            .clone()
            .oneshot(build_json_request(Method::GET, "/", None, None))
            .await
            .expect("run unauthorized request");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let invalid_spec_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "invalid-router-team",
                    "description": null,
                    "spec": {"entrypoint":"planner","members":[]}
                })),
            ))
            .await
            .expect("create invalid team via router");
        assert_eq!(invalid_spec_resp.status(), StatusCode::BAD_REQUEST);

        let create_team_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "router-team",
                    "description": "router-level contract",
                    "spec": {"entrypoint":"planner","members":[{"member_id":"planner"}]}
                })),
            ))
            .await
            .expect("create team via router");
        assert_eq!(create_team_resp.status(), StatusCode::OK);
        let created_team = decode_json_body(create_team_resp).await;
        let team_id = created_team["id"].as_str().expect("team id").to_string();
        assert_eq!(created_team["spec"]["spec_version"], Value::from(1));

        let list_teams_resp = app
            .clone()
            .oneshot(build_json_request(Method::GET, "/", Some(&token), None))
            .await
            .expect("list teams via router");
        assert_eq!(list_teams_resp.status(), StatusCode::OK);
        let listed = decode_json_body(list_teams_resp).await;
        let listed = listed.as_array().expect("teams array");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["id"], team_id);

        let duplicate_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "router-team",
                    "description": null,
                    "spec": {"entrypoint":"planner","members":[{"member_id":"planner"}]}
                })),
            ))
            .await
            .expect("duplicate create");
        assert_eq!(duplicate_resp.status(), StatusCode::CONFLICT);

        let get_team_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("/{team_id}"),
                Some(&token),
                None,
            ))
            .await
            .expect("get team via router");
        assert_eq!(get_team_resp.status(), StatusCode::OK);

        let create_run_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/{team_id}/runs"),
                Some(&token),
                Some(json!({
                    "context_id": "ctx-router",
                    "input": {"prompt":"review this run"}
                })),
            ))
            .await
            .expect("create run via router");
        assert_eq!(create_run_resp.status(), StatusCode::OK);
        let run = decode_json_body(create_run_resp).await;
        let run_id = run["id"].as_str().expect("run id").to_string();
        assert_eq!(run["status"], "submitted");

        let cancel_run_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                &format!("/runs/{run_id}/cancel"),
                Some(&token),
                None,
            ))
            .await
            .expect("cancel run via router");
        assert_eq!(cancel_run_resp.status(), StatusCode::OK);
        let canceled = decode_json_body(cancel_run_resp).await;
        assert_eq!(canceled["status"], "canceled");

        let events_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("/runs/{run_id}/events?limit=100"),
                Some(&token),
                None,
            ))
            .await
            .expect("list events via router");
        assert_eq!(events_resp.status(), StatusCode::OK);
        let events = decode_json_body(events_resp).await;
        let events = events.as_array().expect("events array");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event_type"], "run_submitted");
        assert_eq!(events[1]["event_type"], "run_canceled");
        let first_id = events[0]["event_id"].as_i64().expect("first event id");
        let second_id = events[1]["event_id"].as_i64().expect("second event id");
        assert!(first_id < second_id);

        let paged_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                &format!("/runs/{run_id}/events?limit=1&before_id={second_id}"),
                Some(&token),
                None,
            ))
            .await
            .expect("page events via router");
        assert_eq!(paged_resp.status(), StatusCode::OK);
        let paged = decode_json_body(paged_resp).await;
        let paged = paged.as_array().expect("paged events array");
        assert_eq!(paged.len(), 1);
        assert_eq!(paged[0]["event_type"], "run_submitted");

        let missing_team_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/missing-team/runs",
                Some(&token),
                Some(json!({"input": {}})),
            ))
            .await
            .expect("missing team request");
        assert_eq!(missing_team_resp.status(), StatusCode::NOT_FOUND);

        let missing_run_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::GET,
                "/runs/missing-run/events",
                Some(&token),
                None,
            ))
            .await
            .expect("missing run request");
        assert_eq!(missing_run_resp.status(), StatusCode::NOT_FOUND);

        let unsupported_version_resp = app
            .clone()
            .oneshot(build_json_request(
                Method::POST,
                "/",
                Some(&token),
                Some(json!({
                    "name": "unsupported-version-team",
                    "description": null,
                    "spec": {
                        "spec_version": 2,
                        "entrypoint": "planner",
                        "members": [{"member_id":"planner"}]
                    }
                })),
            ))
            .await
            .expect("unsupported version request");
        assert_eq!(unsupported_version_resp.status(), StatusCode::BAD_REQUEST);
    }
}
