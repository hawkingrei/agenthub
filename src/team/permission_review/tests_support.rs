use std::collections::HashMap;

use crate::state::AppState;
use crate::team::mailbox_hint::{RunningActorRuntime, TeamMailboxHintAgentNudger};
use crate::team::{TeamDefinitionConfig, TeamDefinitionRecord};
use agent_client_protocol::schema::v1::PermissionOptionKind;
use agenthub_acp::{AcpPermissionOption, AcpPermissionReviewRequest, AcpPermissionRoutingMetadata};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct TestMailboxHintAgentNudger {
    pub(super) runtimes: HashMap<String, RunningActorRuntime>,
    pub(super) idle_anchor_by_actor: HashMap<String, Option<i64>>,
    pub(super) prompts: Mutex<Vec<(String, Option<String>, String)>>,
}

#[async_trait::async_trait]
impl TeamMailboxHintAgentNudger for TestMailboxHintAgentNudger {
    async fn running_actor_runtime(&self, actor_id: &str) -> Option<RunningActorRuntime> {
        self.runtimes.get(actor_id).cloned()
    }

    async fn mailbox_idle_anchor_ts(
        &self,
        actor_id: &str,
        _session_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        Ok(self.idle_anchor_by_actor.get(actor_id).cloned().flatten())
    }

    async fn nudge_mailbox_prompt(
        &self,
        actor_id: &str,
        expected_session_id: Option<&str>,
        _delivery_id: &str,
        prompt: &str,
    ) -> anyhow::Result<()> {
        self.prompts.lock().await.push((
            actor_id.to_string(),
            expected_session_id.map(str::to_string),
            prompt.to_string(),
        ));
        Ok(())
    }
}

pub(super) struct PermissionReviewRequestFixture<'a> {
    pub(super) request_id: &'a str,
    pub(super) agent_id: &'a str,
    pub(super) session_id: &'a str,
    pub(super) acp_session_id: &'a str,
    pub(super) requester_actor_id: &'a str,
    pub(super) requester_role: &'a str,
    pub(super) tool_call_id: &'a str,
    pub(super) tool_name: &'a str,
}

impl PermissionReviewRequestFixture<'_> {
    pub(super) fn request(&self, team_id: &str) -> AcpPermissionReviewRequest {
        AcpPermissionReviewRequest {
            request_id: self.request_id.to_string(),
            agent_id: self.agent_id.to_string(),
            agent_session_id: self.session_id.to_string(),
            acp_session_id: self.acp_session_id.to_string(),
            tool_call_id: Some(self.tool_call_id.to_string()),
            options: vec![AcpPermissionOption {
                option_id: "allow".to_string(),
                name: "Allow once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            }],
            tool_call: Some(json!({"tool":{"name": self.tool_name}})),
            current_run_id: None,
            routing: AcpPermissionRoutingMetadata {
                team_id: Some(team_id.to_string()),
                requester_actor_id: Some(self.requester_actor_id.to_string()),
                requester_role: Some(self.requester_role.to_string()),
            },
        }
    }
}

pub(super) async fn create_permission_review_team(
    state: &AppState,
    name_prefix: &str,
    description: &str,
    spec: Value,
) -> TeamDefinitionRecord {
    state
        .teams
        .create_team(TeamDefinitionConfig {
            name: format!("{name_prefix}-{}", Uuid::new_v4()),
            description: Some(description.to_string()),
            spec,
        })
        .await
        .expect("create team")
}

pub(super) async fn insert_running_agent(
    state: &AppState,
    agent_id: &str,
    session_id: &str,
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
    .bind(agent_id)
    .bind(agent_id)
    .bind("/tmp")
    .bind("agenthub-codex-acp")
    .bind("[]")
    .bind("use_existing")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert agent");

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO agent_sessions (id, agent_id, status, started_at, ended_at)
        VALUES (?1, ?2, 'running', ?3, NULL)
        "#,
    )
    .bind(session_id)
    .bind(agent_id)
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert agent session");
}

pub(super) async fn insert_pending_permission_request(
    state: &AppState,
    team_id: &str,
    fixture: &PermissionReviewRequestFixture<'_>,
    now: i64,
) {
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
            tool_call_id,
            options_json,
            tool_call_json,
            status,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'pending', ?11)
        "#,
    )
    .bind(fixture.request_id)
    .bind(fixture.agent_id)
    .bind(fixture.session_id)
    .bind(fixture.acp_session_id)
    .bind(team_id)
    .bind(fixture.requester_actor_id)
    .bind(fixture.requester_role)
    .bind(fixture.tool_call_id)
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
    .bind(json!({"tool":{"name": fixture.tool_name}}).to_string())
    .bind(now)
    .execute(&state.db)
    .await
    .expect("insert permission request");
}
