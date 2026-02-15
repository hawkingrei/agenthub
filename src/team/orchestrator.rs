use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::acp::AcpActorSkillContext;
use crate::agent::AgentManager;

use super::{TeamManager, TeamStepRecord, TeamStepStatus};

#[derive(Debug, Clone, Copy)]
pub struct TeamOrchestratorWorkerSettings {
    pub poll_interval_secs: i64,
    pub max_dispatch_per_tick: i64,
}

impl Default for TeamOrchestratorWorkerSettings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 2,
            max_dispatch_per_tick: 32,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TeamOrchestratorDispatchSummary {
    pub scanned: i64,
    pub dispatched: i64,
    pub failed: i64,
}

#[async_trait]
pub trait TeamMemberAgentStarter: Send + Sync {
    async fn start_member_agent(
        &self,
        member_id: &str,
        actor_context: AcpActorSkillContext,
    ) -> anyhow::Result<String>;
}

#[async_trait]
impl TeamMemberAgentStarter for AgentManager {
    async fn start_member_agent(
        &self,
        member_id: &str,
        actor_context: AcpActorSkillContext,
    ) -> anyhow::Result<String> {
        self.start_agent_with_actor_context(member_id, Some(actor_context))
            .await
    }
}

#[derive(Clone)]
pub struct TeamOrchestratorWorker {
    teams: Arc<TeamManager>,
    agent_starter: Arc<dyn TeamMemberAgentStarter>,
}

impl TeamOrchestratorWorker {
    pub fn new(teams: Arc<TeamManager>, agents: Arc<AgentManager>) -> Self {
        Self::with_agent_starter(teams, agents)
    }

    pub fn with_agent_starter(
        teams: Arc<TeamManager>,
        agent_starter: Arc<dyn TeamMemberAgentStarter>,
    ) -> Self {
        Self {
            teams,
            agent_starter,
        }
    }

    pub fn spawn(self, settings: TeamOrchestratorWorkerSettings) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(
                settings.poll_interval_secs.max(1) as u64,
            ));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                match self.dispatch_once(settings.max_dispatch_per_tick).await {
                    Ok(summary) => {
                        if summary.scanned > 0 || summary.failed > 0 {
                            tracing::debug!(
                                scanned = summary.scanned,
                                dispatched = summary.dispatched,
                                failed = summary.failed,
                                "team orchestrator tick"
                            );
                        }
                    }
                    Err(err) => {
                        tracing::warn!("team orchestrator tick failed: {}", err);
                    }
                }
            }
        });
    }

    pub async fn dispatch_once(
        &self,
        max_dispatch_per_tick: i64,
    ) -> anyhow::Result<TeamOrchestratorDispatchSummary> {
        let max_dispatch = max_dispatch_per_tick.max(1) as usize;
        let runs = self
            .teams
            .list_active_runs(max_dispatch_per_tick.max(64))
            .await?;
        let mut summary = TeamOrchestratorDispatchSummary::default();

        for run in runs {
            if summary.dispatched as usize >= max_dispatch {
                break;
            }
            let steps = self.teams.list_steps(&run.id).await?;
            if steps.is_empty() {
                continue;
            }
            let status_by_key: HashMap<String, TeamStepStatus> = steps
                .iter()
                .map(|step| (step.step_key.clone(), step.status.clone()))
                .collect();

            for step in steps {
                if summary.dispatched as usize >= max_dispatch {
                    break;
                }
                if step.status != TeamStepStatus::Submitted {
                    continue;
                }
                summary.scanned += 1;
                if !is_step_ready(&step, &status_by_key) {
                    continue;
                }
                match self.dispatch_step(&run.id, &step).await {
                    Ok(()) => {
                        summary.dispatched += 1;
                    }
                    Err(err) => {
                        summary.failed += 1;
                        tracing::warn!(
                            run_id = %run.id,
                            step_id = %step.id,
                            step_key = %step.step_key,
                            member_id = %step.member_id,
                            "team orchestrator dispatch failed: {}",
                            err
                        );
                    }
                }
            }
        }
        Ok(summary)
    }

    async fn dispatch_step(&self, run_id: &str, step: &TeamStepRecord) -> anyhow::Result<()> {
        let actor_context = AcpActorSkillContext {
            run_id: run_id.to_string(),
            actor_id: step.member_id.clone(),
            default_channel: "default".to_string(),
            actor_cli_path: default_actor_cli_path(),
        };
        let start_result = self
            .agent_starter
            .start_member_agent(&step.member_id, actor_context)
            .await;
        let session_id = match start_result {
            Ok(session_id) => session_id,
            Err(err) => {
                let err_text = format!(
                    "orchestrator failed to start member agent '{}' for step '{}': {}",
                    step.member_id, step.step_key, err
                );
                if let Err(fail_err) = self.teams.fail_step(&step.id, &err_text).await {
                    tracing::warn!(
                        run_id = %run_id,
                        step_id = %step.id,
                        "team orchestrator failed to mark step as failed: {}",
                        fail_err
                    );
                }
                return Err(err.context(err_text));
            }
        };
        let started = self.teams.start_step(&step.id, Some(&session_id)).await?;
        if started.status != TeamStepStatus::Working {
            tracing::debug!(
                run_id = %run_id,
                step_id = %started.id,
                status = ?started.status,
                "team orchestrator skip non-working start result"
            );
        }
        Ok(())
    }
}

fn default_actor_cli_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "agenthub".to_string())
}

fn is_step_ready(step: &TeamStepRecord, status_by_key: &HashMap<String, TeamStepStatus>) -> bool {
    step.depends_on.iter().all(|dep| {
        status_by_key
            .get(dep)
            .is_some_and(|status| matches!(status, TeamStepStatus::Completed))
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::json;
    use sqlx::SqlitePool;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::{
        TeamMemberAgentStarter, TeamOrchestratorWorker, TeamOrchestratorWorkerSettings,
        is_step_ready,
    };
    use crate::acp::AcpActorSkillContext;
    use crate::team::{
        TeamActorMessageStatus, TeamActorMessageTransport, TeamDefinitionConfig, TeamManager,
        TeamStepRecord, TeamStepStatus,
    };

    fn make_step(step_key: &str, depends_on: &[&str]) -> TeamStepRecord {
        TeamStepRecord {
            id: format!("step-{step_key}"),
            run_id: "run-1".to_string(),
            step_key: step_key.to_string(),
            member_id: "planner".to_string(),
            remote_task_id: None,
            status: TeamStepStatus::Submitted,
            attempt: 0,
            depends_on: depends_on.iter().map(|item| item.to_string()).collect(),
            input: Some(json!({"goal":"x"})),
            output: None,
            error_text: None,
            started_at: None,
            ended_at: None,
        }
    }

    #[test]
    fn step_is_ready_when_all_dependencies_completed() {
        let step = make_step("b", &["a", "c"]);
        let mut status_by_key = HashMap::new();
        status_by_key.insert("a".to_string(), TeamStepStatus::Completed);
        status_by_key.insert("c".to_string(), TeamStepStatus::Completed);
        assert!(is_step_ready(&step, &status_by_key));
    }

    #[test]
    fn step_is_not_ready_when_any_dependency_is_not_completed() {
        let step = make_step("b", &["a", "c"]);
        let mut status_by_key = HashMap::new();
        status_by_key.insert("a".to_string(), TeamStepStatus::Completed);
        status_by_key.insert("c".to_string(), TeamStepStatus::Working);
        assert!(!is_step_ready(&step, &status_by_key));
    }

    #[test]
    fn step_is_not_ready_when_dependency_is_missing() {
        let step = make_step("b", &["missing"]);
        let status_by_key = HashMap::new();
        assert!(!is_step_ready(&step, &status_by_key));
    }

    #[derive(Debug, Clone)]
    struct FakeStartCall {
        member_id: String,
        actor_context: AcpActorSkillContext,
    }

    #[derive(Clone, Default)]
    struct FakeAgentStarter {
        calls: Arc<Mutex<Vec<FakeStartCall>>>,
        fail_members: Arc<Mutex<HashSet<String>>>,
    }

    impl FakeAgentStarter {
        fn mark_fail_for(&self, member_id: &str) {
            self.fail_members
                .lock()
                .expect("lock fail_members")
                .insert(member_id.to_string());
        }

        fn calls(&self) -> Vec<FakeStartCall> {
            self.calls.lock().expect("lock calls").clone()
        }
    }

    #[async_trait]
    impl TeamMemberAgentStarter for FakeAgentStarter {
        async fn start_member_agent(
            &self,
            member_id: &str,
            actor_context: AcpActorSkillContext,
        ) -> anyhow::Result<String> {
            self.calls.lock().expect("lock calls").push(FakeStartCall {
                member_id: member_id.to_string(),
                actor_context,
            });
            if self
                .fail_members
                .lock()
                .expect("lock fail_members")
                .contains(member_id)
            {
                return Err(anyhow::anyhow!("forced starter failure for {}", member_id));
            }
            Ok(format!("session-{member_id}"))
        }
    }

    async fn setup_test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite");

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
        .execute(&pool)
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
        .execute(&pool)
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
        .execute(&pool)
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
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_run_events");

        sqlx::query(
            r#"
            CREATE TABLE team_actor_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                from_actor_id TEXT NOT NULL,
                to_actor_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                transport TEXT NOT NULL,
                route_json TEXT,
                payload_json TEXT NOT NULL,
                idempotency_key TEXT,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                delivered_at INTEGER,
                relay_attempt INTEGER NOT NULL DEFAULT 0,
                relay_next_retry_at INTEGER,
                relay_last_error TEXT,
                dead_letter_at INTEGER,
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_actor_messages");

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX idx_team_actor_messages_idempotency
            ON team_actor_messages(run_id, from_actor_id, idempotency_key)
            WHERE idempotency_key IS NOT NULL
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_actor_messages idempotency index");

        pool
    }

    #[tokio::test]
    async fn dispatch_once_injects_actor_runtime_and_supports_inbox_ack_flow() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-team".to_string(),
                description: Some("team for orchestrator dispatch test".to_string()),
                spec: json!({
                    "entrypoint":"planner",
                    "members":[{"member_id":"planner"},{"member_id":"reviewer"}]
                }),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-orchestrator"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");

        let starter = Arc::new(FakeAgentStarter::default());
        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter.clone());
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.scanned, 1);
        assert_eq!(summary.dispatched, 1);
        assert_eq!(summary.failed, 0);

        let calls = starter.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].member_id, "planner");
        assert_eq!(calls[0].actor_context.run_id, run.id);
        assert_eq!(calls[0].actor_context.actor_id, "planner");
        assert_eq!(calls[0].actor_context.default_channel, "default");

        let started_step = teams.get_step(&step.id).await.expect("get started step");
        assert_eq!(started_step.status, TeamStepStatus::Working);
        assert_eq!(
            started_step.remote_task_id.as_deref(),
            Some("session-planner")
        );

        let sent = teams
            .send_actor_message(
                &run.id,
                "reviewer",
                &calls[0].actor_context.actor_id,
                "default",
                TeamActorMessageTransport::Local,
                None,
                json!({"text":"ping"}),
                Some("orchestrator-inbox-ack"),
            )
            .await
            .expect("send actor message");

        let inbox = teams
            .list_actor_inbox(&run.id, &calls[0].actor_context.actor_id, 10, None, false)
            .await
            .expect("list inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].message_id, sent.message_id);
        assert_eq!(inbox[0].status, TeamActorMessageStatus::Pending);

        let acked = teams
            .ack_actor_message(&run.id, &calls[0].actor_context.actor_id, sent.message_id)
            .await
            .expect("ack message");
        assert_eq!(acked.status, TeamActorMessageStatus::Delivered);
    }

    #[tokio::test]
    async fn dispatch_once_marks_step_failed_when_member_start_fails() {
        let db = setup_test_db().await;
        let teams = Arc::new(TeamManager::new(db));
        let team = teams
            .create_team(TeamDefinitionConfig {
                name: "orchestrator-fail-team".to_string(),
                description: Some("team for orchestrator failure test".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = teams
            .create_run(&team.id, Some("ctx-fail"), json!({"prompt":"go"}))
            .await
            .expect("create run");
        let step = teams
            .submit_step(
                &run.id,
                "plan",
                "planner",
                Vec::new(),
                Some(json!({"goal":"plan"})),
            )
            .await
            .expect("submit step");

        let starter = Arc::new(FakeAgentStarter::default());
        starter.mark_fail_for("planner");

        let worker = TeamOrchestratorWorker::with_agent_starter(teams.clone(), starter);
        let summary = worker.dispatch_once(10).await.expect("dispatch once");

        assert_eq!(summary.scanned, 1);
        assert_eq!(summary.dispatched, 0);
        assert_eq!(summary.failed, 1);

        let failed_step = teams.get_step(&step.id).await.expect("get failed step");
        assert_eq!(failed_step.status, TeamStepStatus::Failed);
        assert!(
            failed_step
                .error_text
                .as_deref()
                .is_some_and(|text| text.contains("failed to start member agent"))
        );
    }

    #[test]
    fn orchestrator_settings_have_expected_defaults() {
        let settings = TeamOrchestratorWorkerSettings::default();
        assert_eq!(settings.poll_interval_secs, 2);
        assert_eq!(settings.max_dispatch_per_tick, 32);
    }
}
