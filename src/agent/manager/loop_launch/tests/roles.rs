use super::*;
use agenthub_agent_domain::loop_runtime::{LoopOutcomeKind, LoopWaitReason};

async fn set_worker_prompt(fixture: &Fixture, prompt: &str) {
    let mut team = fixture
        .state
        .teams
        .get_team(&fixture.team_id)
        .await
        .unwrap();
    team.spec["members"][1]["prompt"] = serde_json::json!(prompt);
    sqlx::query("UPDATE team_definitions SET spec_json = ? WHERE id = ?")
        .bind(team.spec.to_string())
        .bind(&fixture.team_id)
        .execute(&fixture.state.db)
        .await
        .unwrap();
}

#[tokio::test]
async fn loop_role_preflight_reports_invalid_configuration_without_starting_a_provider() {
    let fixture = Fixture::new("no-outcome").await;
    let mut team = fixture
        .state
        .teams
        .get_team(&fixture.team_id)
        .await
        .unwrap();
    for prompt in [serde_json::json!(17), serde_json::json!("x".repeat(20_001))] {
        team.spec["members"][1]["prompt"] = prompt;
        let preflight = fixture
            .state
            .agents
            .loop_preflight(
                &fixture.team_id,
                &team.spec,
                "worker",
                LoopSessionPolicy::Fresh,
            )
            .await
            .unwrap();
        assert!(!preflight.ready);
        assert_eq!(preflight.blockers, vec!["role_prompt_invalid"]);
    }
    team.spec["members"][1]["prompt"] = serde_json::json!("");
    assert!(
        fixture
            .state
            .agents
            .loop_preflight(
                &fixture.team_id,
                &team.spec,
                "worker",
                LoopSessionPolicy::Fresh,
            )
            .await
            .unwrap()
            .ready
    );
    assert!(!fixture.directory.join("requests.jsonl").exists());
    fixture.close().await;
}

#[tokio::test]
async fn loop_role_prompt_is_pinned_before_provider_start_and_reselected_next_activation() {
    let fixture = Fixture::new("role-pin").await;
    set_worker_prompt(&fixture, "Configured worker policy alpha.").await;
    let (first, ()) = tokio::join!(fixture.execute("role-alpha"), async {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !fixture.directory.join("role-ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        set_worker_prompt(&fixture, "Configured worker policy beta.").await;
        std::fs::write(fixture.directory.join("role-release"), "continue").unwrap();
    });
    assert_eq!(first.state, LoopActivationState::Finished);
    let second = fixture.execute("role-beta").await;
    assert_eq!(second.state, LoopActivationState::Finished);
    assert_eq!(first.mailbox_run_id, second.mailbox_run_id);
    assert_ne!(first.session_id, second.session_id);
    let first_launch = first.launch.unwrap();
    let second_launch = second.launch.unwrap();
    assert!(
        first_launch
            .entry_prompt_version
            .ends_with(":worker:configured")
    );
    assert_eq!(
        first_launch.entry_prompt_version,
        second_launch.entry_prompt_version
    );
    assert_ne!(
        first_launch.configuration_digest,
        second_launch.configuration_digest
    );
    let transcript = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    let prompts: Vec<_> = transcript
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter_map(|mut item| item.get_mut("role_prompt").map(serde_json::Value::take))
        .map(|prompt| prompt.to_string())
        .collect();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("Configured worker policy alpha."));
    assert!(!prompts[0].contains("Configured worker policy beta."));
    assert!(prompts[1].contains("Configured worker policy beta."));
    assert!(!prompts[1].contains("Configured worker policy alpha."));
    assert!(
        prompts
            .iter()
            .all(|prompt| prompt.contains("team-loop-runtime"))
    );
    fixture.close().await;
}

#[tokio::test]
async fn loop_role_wait_exits_and_continuation_recovers_without_a_transcript() {
    let fixture = Fixture::new("role-wait").await;
    let first = fixture.execute("role-wait").await;
    assert_eq!(first.state, LoopActivationState::Finished);
    let outcome = first.outcome.unwrap();
    assert_eq!(outcome.kind, LoopOutcomeKind::Waiting);
    assert_eq!(outcome.wait_reason, Some(LoopWaitReason::DueTime));
    assert!(outcome.continuation.is_some());
    let second = fixture.execute_pending("worker").await;
    assert_ne!(first.session_id, second.session_id);
    assert_eq!(first.mailbox_run_id, second.mailbox_run_id);
    assert_eq!(
        second.outcome.unwrap().kind,
        LoopOutcomeKind::NoActionableWork
    );
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loop_activations WHERE state = 'pending'")
            .fetch_one(&fixture.state.db)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    let transcript = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
    assert_eq!(transcript.matches("session/new").count(), 2);
    assert_eq!(transcript.matches("session/prompt").count(), 2);
    assert!(!transcript.contains("session/load"));
    fixture.close().await;
}
