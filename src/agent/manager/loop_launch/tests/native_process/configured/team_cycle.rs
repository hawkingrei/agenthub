use super::*;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn configured_provider_team_cycle_keeps_native_children_private() {
    let mut fixture = Fixture::new("no-outcome").await;
    let result = check(&mut fixture).await;
    fixture.close().await;
    result.unwrap();
}

async fn check(fixture: &mut Fixture) -> anyhow::Result<()> {
    configure(fixture).await?;
    let original_agents: Vec<String> = sqlx::query_scalar("SELECT id FROM agents ORDER BY id")
        .fetch_all(&fixture.state.db)
        .await?;
    let script = fixture.directory.join("native-cycle.py");
    std::fs::write(&script, cycle::SCRIPT)?;
    let command = format!(
        "python3 '{}'",
        script.to_string_lossy().replace('\'', "'\\''")
    );
    let task_subject = format!("Private child evidence {}", uuid::Uuid::new_v4());
    let child_instruction = format!(
        "Use task_create exactly once to create a private native task with subject {task_subject:?} \
         and description 'Local execution detail only'. Then reply CHILD-COMPLETE. \
         Do not use shell, external tools, delegation, or canonical Team task tools."
    );
    let instructions = format!(
        "This activation performs an isolated lifecycle acceptance task. \
         First call spawn_agent with name=general, run_in_background=false and instruction={child_instruction:?}. \
         Once that child returns, call bash exactly once with command={command:?}, \
         sandbox_permissions=require_escalated and justification=Run the isolated lifecycle fixture. \
         Use the current workspace with no environment overrides. \
         The prepared script performs the role-specific dispatch, report or acceptance and canonical finish. \
         Do not read or edit files or call other tools. Do not repeat either operation."
    );
    for actor in ["planner", "worker"] {
        set_member(fixture, actor, json!({
            "description":"Performs synthetic lifecycle acceptance through the prepared local control fixture.",
            "prompt_append":instructions,
        })).await?;
    }
    let store = LoopStore::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    store
        .configure(
            LoopPolicyUpdate {
                actor_id: "planner",
                team_id: &fixture.team_id,
                expected_revision: 1,
                state: LoopPolicyState::Enabled,
                session_policy: LoopSessionPolicy::Fresh,
                limits: &LoopLimits::default(),
            },
            now,
        )
        .await?;
    let task = fixture
        .state
        .teams
        .create_task(
            &fixture.team_id,
            "Perform isolated lifecycle acceptance with the prepared fixture",
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await?
        .0;
    store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "planner".into(),
                team_id: fixture.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "configured-dispatch".into(),
                due_at: None,
                references: LoopSourceReferences {
                    task_id: Some(task.id),
                    ..Default::default()
                },
            },
            now,
        )
        .await?;
    let mut activations = Vec::new();
    for actor in ["planner", "worker", "planner"] {
        let id: String = sqlx::query_scalar("SELECT id FROM loop_activations WHERE actor_id = ? AND state = 'pending' ORDER BY id LIMIT 1")
            .bind(actor).fetch_one(&fixture.state.db).await?;
        let LoopAdmission::Admitted(reservation) = store
            .admit(
                &fixture.team_id,
                &id,
                fixture.state.agents.loop_owner_id(),
                Utc::now().timestamp(),
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
        let activation = execute(fixture, reservation, Some(&command)).await?;
        let events = fixture
            .state
            .agents
            .list_events_for_session(
                actor,
                activation.session_id.as_deref().context("session")?,
                500,
                None,
            )
            .await?;
        let completed_children = events
            .iter()
            .filter_map(|event| serde_json::from_str::<Value>(&event.message).ok())
            .filter(|event| {
                event["type"] == "tool_call_update"
                    && event["title"] == "spawn_agent"
                    && event["status"] == "completed"
            })
            .count();
        ensure!(
            completed_children == 1,
            "native child did not complete exactly once"
        );
        activations.push(activation);
    }
    ensure!(activations[0].session_id != activations[2].session_id);
    ensure!(activations[0].mailbox_run_id == activations[2].mailbox_run_id);
    let (tasks, completed): (i64, i64) = sqlx::query_as("SELECT COUNT(*), SUM(title = 'Native offline review' AND status = 'completed') FROM team_tasks WHERE team_id = ?")
        .bind(&fixture.team_id).fetch_one(&fixture.state.db).await?;
    ensure!(
        tasks == 2 && completed == 1,
        "native tasks leaked into canonical task state"
    );
    let agents: Vec<String> = sqlx::query_scalar("SELECT id FROM agents ORDER BY id")
        .fetch_all(&fixture.state.db)
        .await?;
    ensure!(
        agents == original_agents,
        "native child acquired an outer identity"
    );
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loop_activations WHERE state = 'pending'")
            .fetch_one(&fixture.state.db)
            .await?;
    ensure!(pending == 0);
    ensure!(
        fixture
            .state
            .agents
            .loop_credentials
            .lock()
            .await
            .is_empty()
    );
    let transcript: Vec<Value> =
        std::fs::read_to_string(fixture.directory.join("native-cycle.jsonl"))?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
    ensure!(transcript.len() == 3);
    for (entry, stage) in transcript.iter().zip(["dispatch", "report", "accept"]) {
        ensure!(entry["stage"] == stage && entry["child_identity_denied"] == true);
    }
    let mut private_tasks = 0;
    for workspace in std::fs::read_dir(fixture.directory.join("native-state/workspaces"))? {
        let workspace = workspace?;
        let tasks = workspace.path().join("tasks");
        if workspace.file_type()?.is_dir() && tasks.is_dir() {
            private_tasks += count_private_tasks(&tasks, &task_subject)?;
        }
    }
    ensure!(
        private_tasks == 3,
        "native children did not create their private task artifacts"
    );
    Ok(())
}

fn count_private_tasks(directory: &std::path::Path, subject: &str) -> anyhow::Result<usize> {
    let mut count = 0;
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            count += count_private_tasks(&entry.path(), subject)?;
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "json") {
            let task: Value = serde_json::from_slice(&std::fs::read(entry.path())?)?;
            count += usize::from(task["subject"] == subject);
        }
    }
    Ok(count)
}
