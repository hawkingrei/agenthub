use serde_json::{Value, json};
use sqlx::Row;
use tonic::{Code, Request, metadata::MetadataValue};
use uuid::Uuid;

use super::super::auth::{InternalAction, InternalAuthz, InternalAuthzConfig, InternalRole};
use super::super::proto::agenthub::internal::v1::team_internal_control_server::TeamInternalControl;
use super::super::proto::agenthub::internal::v1::{
    AckActorMessageRequest, AppendTeamTaskNoteRequest, CancelTimeTriggerRequest,
    CreateTeamTaskRequest, CreateTimeTriggerRequest, DescribeTeamContextRequest,
    GetTeamTaskRequest, IssueNodeCredentialRequest, ListActorInboxRequest, ListTeamTasksRequest,
    ListTimeTriggersRequest, RespondPermissionReviewRequest, SendActorMessageRequest,
    UpdateTeamTaskRequest,
};
pub(super) use super::super::tls::InternalGrpcSecurityMode;
pub(super) use super::resolve_team_leader_member_id;
use super::{BOOTSTRAP_TOKEN_HEADER, TeamInternalControlService, map_actor_service_status};
use crate::agent::AgentTimeTriggerRecord;
use crate::api::team_tests::build_test_state;
use crate::team::{TeamDefinitionConfig, TeamTaskDetailRecord, TeamTaskRecord};
use agenthub_team_actor::ActorMessageStatus;

const TEST_INTERNAL_SHARED_SECRET: &str = "agenthub-internal-service-test-secret";

fn build_authz() -> InternalAuthz {
    InternalAuthz::new(InternalAuthzConfig {
        shared_secret: TEST_INTERNAL_SHARED_SECRET.to_string(),
        expected_issuer: Some("agenthub".to_string()),
        expected_audience: Some("agenthub-internal".to_string()),
    })
}

fn issue_token(
    authz: &InternalAuthz,
    role: InternalRole,
    actor_id: Option<&str>,
    run_id: Option<&str>,
) -> String {
    let permissions = vec![
        InternalAction::MessageSend.as_str().to_string(),
        InternalAction::InboxList.as_str().to_string(),
        InternalAction::MessageAck.as_str().to_string(),
        InternalAction::TeamRead.as_str().to_string(),
        InternalAction::TeamTaskWrite.as_str().to_string(),
        InternalAction::TimeTriggerManage.as_str().to_string(),
        InternalAction::PermissionReview.as_str().to_string(),
    ];
    let (token, _expires_at) = authz
        .issue_access_token(role, actor_id, run_id, permissions, 600)
        .expect("issue internal token");
    token
}

fn authenticated_request<T>(payload: T, token: &str) -> Request<T> {
    let mut request = Request::new(payload);
    request.metadata_mut().insert(
        "authorization",
        MetadataValue::try_from(format!("Bearer {token}")).expect("authorization metadata"),
    );
    request
}

async fn create_team_run(state: &crate::state::AppState) -> crate::team::TeamRunRecord {
    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("internal-grpc-mailbox-{}", Uuid::new_v4()),
            description: Some("internal grpc mailbox test team".to_string()),
            spec: json!({
                "entrypoint":"planner",
                "leader_member_id":"planner",
                "members":[
                    {"member_id":"planner","role":"leader"},
                    {"member_id":"reviewer","role":"worker"}
                ]
            }),
        })
        .await
        .expect("create test team");
    state
        .teams
        .create_run(
            &team.id,
            Some("ctx-internal-grpc-mailbox"),
            json!({"prompt":"validate internal grpc mailbox"}),
        )
        .await
        .expect("create test run")
}

fn default_permission_review_team_spec() -> Value {
    json!({
        "entrypoint":"planner",
        "leader_member_id":"planner",
        "members":[
            {"member_id":"planner","role":"leader"},
            {"member_id":"reviewer","role":"worker"},
            {"member_id":"observer","role":"worker"}
        ]
    })
}

async fn create_permission_review_run_with_spec(
    state: &crate::state::AppState,
    name_suffix: &str,
    prompt: &str,
    spec: Value,
) -> crate::team::TeamRunRecord {
    let context_id = format!("ctx-internal-grpc-{name_suffix}");
    let team = state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("internal-grpc-{name_suffix}-{}", Uuid::new_v4()),
            description: Some(format!("{name_suffix} permission review test")),
            spec,
        })
        .await
        .expect("create permission review team");
    state
        .teams
        .create_run(
            &team.id,
            Some(context_id.as_str()),
            json!({"prompt": prompt}),
        )
        .await
        .expect("create permission review run")
}

struct PermissionReviewFixture {
    state: crate::state::AppState,
    run: crate::team::TeamRunRecord,
    service: TeamInternalControlService,
    token: String,
    now: i64,
}

struct PermissionReviewSeed<'a> {
    request_id: &'a str,
    agent_id: &'a str,
    session_id: &'a str,
    acp_session_id: &'a str,
    requester_actor_id: &'a str,
    requester_role: &'a str,
    review_target_actor_id: Option<&'a str>,
    tool_call_id: &'a str,
    status: &'a str,
}

async fn setup_permission_review_fixture_with_spec(
    name_suffix: &str,
    prompt: &str,
    spec: Value,
    token_role: InternalRole,
    token_actor_id: &str,
) -> PermissionReviewFixture {
    let state = build_test_state().await;
    let run = create_permission_review_run_with_spec(&state, name_suffix, prompt, spec).await;
    let authz = build_authz();
    let token = issue_token(&authz, token_role, Some(token_actor_id), None);
    let service = TeamInternalControlService::new(
        state.clone(),
        authz,
        super::InternalGrpcSecurityMode::Disabled,
        std::env::temp_dir(),
        "bootstrap-token".to_string(),
    );
    PermissionReviewFixture {
        state,
        run,
        service,
        token,
        now: chrono::Utc::now().timestamp(),
    }
}

async fn setup_permission_review_fixture(
    name_suffix: &str,
    prompt: &str,
) -> PermissionReviewFixture {
    setup_permission_review_fixture_with_spec(
        name_suffix,
        prompt,
        default_permission_review_team_spec(),
        InternalRole::Worker,
        "observer",
    )
    .await
}

async fn seed_permission_review_request(
    state: &crate::state::AppState,
    run: &crate::team::TeamRunRecord,
    seed: PermissionReviewSeed<'_>,
    now: i64,
) {
    sqlx::query(
            r#"
            INSERT OR IGNORE INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 1, 'running', ?7, ?8)
            "#,
        )
        .bind(seed.agent_id)
        .bind(seed.agent_id)
        .bind("/tmp")
        .bind("agenthub-codex-acp")
        .bind("[]")
        .bind("use_existing")
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await
        .expect("insert permission review agent");
    sqlx::query(
        r#"
            INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
            VALUES (?1, ?2, 'running', ?3, NULL)
            "#,
    )
    .bind(seed.session_id)
    .bind(seed.agent_id)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert permission review session");
    sqlx::query(
        r#"
            INSERT INTO acp_permission_requests (
                id,
                agent_id,
                session_id,
                acp_session_id,
                team_id,
                requester_actor_id,
                requester_role,
                review_target_actor_id,
                tool_call_id,
                options_json,
                tool_call_json,
                status,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
    )
    .bind(seed.request_id)
    .bind(seed.agent_id)
    .bind(seed.session_id)
    .bind(seed.acp_session_id)
    .bind(&run.team_id)
    .bind(seed.requester_actor_id)
    .bind(seed.requester_role)
    .bind(seed.review_target_actor_id)
    .bind(seed.tool_call_id)
    .bind(
        json!([
            {
                "option_id": "allow",
                "name": "Allow once",
                "kind": "allow_once"
            }
        ])
        .to_string(),
    )
    .bind(json!({"tool":{"name":"mcp__fs__read"}}).to_string())
    .bind(seed.status)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert permission review request");
}

mod context_tasks;
mod mailbox;
mod misc;
mod node_credentials;
mod permission_review;
mod time_triggers;
