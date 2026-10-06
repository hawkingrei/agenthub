use std::path::PathBuf;

use agenthub_db::runtime_events::RuntimeHistory;

use super::*;

pub(super) struct ConfiguredFixture {
    pub native: NativeFixture,
    database_dir: PathBuf,
    database: PathBuf,
    task_id: String,
    memory_prefix: Option<String>,
}

impl ConfiguredFixture {
    pub async fn new() -> Self {
        let database_dir =
            std::env::temp_dir().join(format!("native-team-recovery-{}", uuid::Uuid::new_v4(),));
        std::fs::create_dir(&database_dir).unwrap();
        std::fs::set_permissions(&database_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let database = database_dir.join("control.sqlite");
        agenthub_db::init_db_at_path(&database)
            .await
            .unwrap()
            .close()
            .await;
        let state = crate::api::team_tests::reopen_test_state_with_db_path(&database).await;
        let fixture = Fixture::with_state(state, "no-outcome", None).await;
        let native = NativeFixture::with_fixture("clean", fixture).await;
        let task_id = native
            .fixture
            .state
            .teams
            .create_task(
                &native.fixture.team_id,
                "Exercise isolated native conversation recovery",
                "user",
                json!({}),
                "group_chat",
                None,
            )
            .await
            .unwrap()
            .0
            .id;
        Self {
            native,
            database_dir,
            database,
            task_id,
            memory_prefix: None,
        }
    }

    pub async fn configure(&mut self) -> anyhow::Result<()> {
        configure(&mut self.native.fixture).await?;
        set_member(&self.native.fixture, "worker", json!({
            "description":"Performs isolated synthetic conversation, shell-approval and recovery acceptance.",
            "prompt_append":"For this isolated acceptance task, follow the current pinned task summary. It specifies the one permitted action and exact reply. Do not inspect files or call other tools. Ordinary textual completion must leave the canonical task open; do not call loop-finish unless the summary explicitly requests it.",
        })).await?;
        Ok(())
    }

    pub async fn admit(&mut self, key: &str, instruction: &str) -> anyhow::Result<LoopReservation> {
        let fixture = &self.native.fixture;
        fixture
            .state
            .teams
            .update_task_with_context(
                &self.task_id,
                None,
                crate::team::TeamTaskAssignmentUpdate::Unchanged,
                Some(crate::team::TeamTaskContextPatch::Merge(
                    json!({"summary":instruction}),
                )),
            )
            .await?;
        let store = LoopStore::new(fixture.state.db.clone());
        let now = Utc::now().timestamp();
        let trigger = store
            .accept_trigger(
                &LoopTriggerInput {
                    actor_id: "worker".into(),
                    team_id: fixture.team_id.clone(),
                    kind: LoopTriggerKind::Operator,
                    source_key: key.into(),
                    due_at: None,
                    references: LoopSourceReferences {
                        task_id: Some(self.task_id.clone()),
                        ..Default::default()
                    },
                },
                now,
            )
            .await?;
        let LoopAdmission::Admitted(reservation) = store
            .admit(
                &fixture.team_id,
                &trigger.activation_id,
                fixture.state.agents.loop_owner_id(),
                now,
            )
            .await?
        else {
            anyhow::bail!("not admitted")
        };
        fixture
            .state
            .agents
            .track_loop_reservation(reservation.clone())
            .await?;
        let context = store
            .pin_task_context(&reservation, &self.task_id, now)
            .await?;
        if let Some(prefix) = &self.memory_prefix {
            ensure!(
                *prefix == context.memory_prefix,
                "task memory scope changed on resume"
            );
        } else {
            self.memory_prefix = Some(context.memory_prefix);
        }
        Ok(reservation)
    }

    pub async fn run(&self, reservation: LoopReservation) -> anyhow::Result<LoopActivation> {
        let fixture = &self.native.fixture;
        let id = reservation.activation_id.clone().context("activation")?;
        tokio::time::timeout(
            Duration::from_secs(180),
            fixture
                .state
                .agents
                .execute_loop_activation(fixture.state.teams.clone(), reservation),
        )
        .await
        .context("configured Team activation timed out")??;
        let store = LoopStore::new(fixture.state.db.clone());
        ensure!(
            store
                .reservation(&fixture.team_id, "worker")
                .await?
                .is_none()
        );
        ensure!(
            !fixture
                .state
                .agents
                .inner
                .read()
                .await
                .contains_key("worker")
        );
        store
            .activation(&fixture.team_id, &id)
            .await?
            .context("retained activation")
    }

    pub async fn restart(&mut self) -> anyhow::Result<()> {
        let fixture = &mut self.native.fixture;
        let mut config = (*fixture.state.agents.loop_app_config).clone();
        let events = fixture.state.agents.event_dbs.clone();
        fixture.state.agents.stop_all_on_shutdown().await?;
        fixture
            .state
            .agents
            .daemon_tasks()
            .shutdown_runtime(Duration::from_secs(5))
            .await?;
        fixture
            .state
            .agents
            .daemon_tasks()
            .shutdown_background(Duration::from_secs(5))
            .await?;
        fixture.state.db.close().await;
        let mut state =
            crate::api::team_tests::reopen_test_state_with_db_path(&self.database).await;
        config.internal_grpc = Some(agenthub_config::InternalGrpcConfig {
            enabled: Some(true),
            listen: Some("127.0.0.1:0".into()),
            security: Some(agenthub_config::InternalGrpcSecurityConfig {
                mode: Some("disabled".into()),
                cert_dir: Some(
                    fixture
                        .directory
                        .join("certs")
                        .to_string_lossy()
                        .into_owned(),
                ),
            }),
            auth: None,
            bootstrap: None,
        });
        let mut manager = (*state.agents).clone().with_loop_app_config(config.clone());
        manager.event_dbs = events;
        manager.mark_exited_on_startup().await?;
        state.agents = Arc::new(manager);
        crate::internal::maybe_spawn_internal_grpc(state.clone(), &config).await?;
        fixture.state = state;
        Ok(())
    }

    pub async fn pending(&self, activation: &str) -> anyhow::Result<(String, String)> {
        tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                let pending = sqlx::query_as("SELECT p.id, p.session_id FROM acp_permission_requests p JOIN loop_activations a ON a.session_id = p.session_id WHERE a.id = ? AND p.agent_id = 'worker' AND p.status = 'pending'")
                    .bind(activation).fetch_optional(&self.native.fixture.state.db).await?;
                if let Some(pending) = pending { return Ok(pending); }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await.context("configured Team approval timed out")?
    }

    pub async fn recovery(
        &self,
    ) -> anyhow::Result<crate::agent::manager::rara::NativeRecoveryView> {
        tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                let runtime = self
                    .native
                    .fixture
                    .state
                    .agents
                    .inner
                    .read()
                    .await
                    .get("worker")
                    .and_then(|handle| match &handle.input {
                        AgentInput::Rara(runtime) => Some(runtime.clone()),
                        _ => None,
                    });
                if let Some(runtime) = runtime
                    && let Ok(view) = runtime.query_recovery().await
                {
                    return view;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .context("configured Team recovery entry timed out")
    }

    pub async fn history(&self, activation: &LoopActivation) -> anyhow::Result<RuntimeHistory> {
        let history = self
            .native
            .fixture
            .state
            .agents
            .runtime_history(
                "worker",
                activation.session_id.as_deref().context("local session")?,
                100,
                None,
            )
            .await?
            .context("runtime history")?;
        ensure!(history.closed && !history.streams_truncated);
        ensure!(
            history
                .streams
                .iter()
                .all(|stream| stream.cursor.gap.is_none())
        );
        Ok(history)
    }

    pub async fn assert_resume(
        &self,
        first: &LoopActivation,
        next: &LoopActivation,
    ) -> anyhow::Result<()> {
        ensure!(first.mailbox_run_id == next.mailbox_run_id && first.session_id != next.session_id);
        ensure!(next.generation > first.generation);
        let before = self.history(first).await?;
        let after = self.history(next).await?;
        ensure!(before.runtime_id != after.runtime_id);
        ensure!(before.streams.len() == 1 && after.streams.len() == 1);
        ensure!(before.streams[0].native_session_id == after.streams[0].native_session_id);
        ensure!(
            after
                .receipts
                .iter()
                .any(|receipt| receipt.kind == RuntimeRequestKind::ResumeSession
                    && receipt.status == RuntimeRequestStatus::Accepted)
        );
        ensure!(
            !after
                .receipts
                .iter()
                .any(|receipt| receipt.kind == RuntimeRequestKind::CreateSession)
        );
        Ok(())
    }

    pub async fn assistant_text(
        &self,
        activation: &LoopActivation,
        tools_allowed: bool,
    ) -> anyhow::Result<String> {
        let events = self
            .native
            .fixture
            .state
            .agents
            .list_events_for_session(
                "worker",
                activation.session_id.as_deref().context("local session")?,
                500,
                None,
            )
            .await?;
        let mut text = String::new();
        for event in events {
            ensure!(
                event.message != "Runtime turn failed.",
                "configured provider turn failed"
            );
            if let Ok(value) = serde_json::from_str::<Value>(&event.message) {
                ensure!(
                    tools_allowed || value["type"] != "tool_call",
                    "unexpected worker tool"
                );
                if value["type"] == "agent_message" {
                    text.push_str(value["text"].as_str().unwrap_or_default());
                }
            }
        }
        Ok(text)
    }

    pub async fn assert_task_unchanged(&self) -> anyhow::Result<()> {
        let fixture = &self.native.fixture;
        let (count, open): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), SUM(id = ? AND status = 'open') FROM team_tasks WHERE team_id = ?",
        )
        .bind(&self.task_id)
        .bind(&fixture.team_id)
        .fetch_one(&fixture.state.db)
        .await?;
        ensure!(count == 1 && open == 1);
        ensure!(
            self.native.requests.lock().await.is_empty(),
            "fallback mock provider was called"
        );
        Ok(())
    }

    pub async fn close(self) {
        self.native.close().await;
        std::fs::remove_dir_all(self.database_dir).unwrap();
    }
}
