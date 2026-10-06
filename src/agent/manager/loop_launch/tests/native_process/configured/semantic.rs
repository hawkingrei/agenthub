use agenthub_agent_domain::loop_runtime::{LoopOutcomeKind, LoopWaitReason};

use super::*;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn configured_provider_semantic_outcomes_finish_without_worker_tools() {
    for (title, description, expected) in [
        (
            "Compose an original wedding poem",
            "Only reviews Rust database query plans. Creative writing is outside this Card.",
            LoopOutcomeKind::NoActionableWork,
        ),
        (
            "Review the query plan, but no query, plan, database or target has been provided",
            "Reviews a supplied database query and its execution plan; requires those inputs before useful work.",
            LoopOutcomeKind::Waiting,
        ),
    ] {
        let mut fixture = Fixture::new("no-outcome").await;
        let result = check(&mut fixture, title, description, expected).await;
        fixture.close().await;
        result.unwrap();
    }
}

async fn check(
    fixture: &mut Fixture,
    title: &str,
    description: &str,
    expected: LoopOutcomeKind,
) -> anyhow::Result<()> {
    configure(fixture).await?;
    set_member(fixture, "worker", json!({"description":description})).await?;
    let task = fixture
        .state
        .teams
        .create_task(
            &fixture.team_id,
            title,
            "user",
            json!({}),
            "group_chat",
            None,
        )
        .await?
        .0;
    let store = LoopStore::new(fixture.state.db.clone());
    let now = Utc::now().timestamp();
    let trigger = store
        .accept_trigger(
            &LoopTriggerInput {
                actor_id: "worker".into(),
                team_id: fixture.team_id.clone(),
                kind: LoopTriggerKind::Operator,
                source_key: "configured-semantics".into(),
                due_at: None,
                references: LoopSourceReferences {
                    task_id: Some(task.id.clone()),
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
    let activation = execute(fixture, reservation, None).await?;
    let outcome = activation.outcome.context("semantic finish")?;
    ensure!(
        outcome.kind == expected,
        "unexpected configured semantic outcome"
    );
    ensure!(
        outcome.wait_reason
            == (expected == LoopOutcomeKind::Waiting).then_some(LoopWaitReason::Input)
    );
    let registrations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loop_registrations WHERE actor_id = 'worker'")
            .fetch_one(&fixture.state.db)
            .await?;
    ensure!(registrations == i64::from(expected == LoopOutcomeKind::Waiting));
    let events = fixture
        .state
        .agents
        .list_events_for_session(
            "worker",
            activation.session_id.as_deref().context("session")?,
            500,
            None,
        )
        .await?;
    ensure!(
        !events
            .iter()
            .filter_map(|event| serde_json::from_str::<Value>(&event.message).ok())
            .any(|event| event["type"] == "tool_call"),
        "semantic decline executed a worker tool"
    );
    let status: String = sqlx::query_scalar("SELECT status FROM team_tasks WHERE id = ?")
        .bind(&task.id)
        .fetch_one(&fixture.state.db)
        .await?;
    ensure!(
        status == "open",
        "semantic result changed canonical task status"
    );
    if expected == LoopOutcomeKind::Waiting {
        let messages: Vec<String> = sqlx::query_scalar("SELECT payload_json FROM team_conversation_messages WHERE task_id = ? AND from_actor_id = 'worker'")
            .bind(&task.id).fetch_all(&fixture.state.db).await?;
        ensure!(messages.len() == 1);
        let question: Value = serde_json::from_str(&messages[0])?;
        ensure!(
            question["text"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty())
        );
    }
    Ok(())
}
