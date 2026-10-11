use super::*;
use agenthub_agent_domain::loop_runtime::{LoopOutcomeKind, LoopWaitReason};

#[tokio::test]
async fn native_loop_semantic_declines_finish_only_the_admitted_activation() {
    for (mode, kind, wait) in [
        ("guard-mismatch", LoopOutcomeKind::NoActionableWork, None),
        (
            "guard-clarification",
            LoopOutcomeKind::Waiting,
            Some(LoopWaitReason::Input),
        ),
    ] {
        let fixture = fixture(mode).await;
        let activation = fixture.execute(mode).await;
        assert_eq!(activation.state, LoopActivationState::Finished);
        let outcome = activation.outcome.unwrap();
        assert_eq!(outcome.kind, kind);
        assert_eq!(outcome.wait_reason, wait);
        let registrations: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM loop_registrations WHERE actor_id = 'worker'")
                .fetch_one(&fixture.state.db)
                .await
                .unwrap();
        assert_eq!(registrations, i64::from(wait.is_some()));
        let log = std::fs::read_to_string(fixture.directory.join("native-requests.jsonl")).unwrap();
        assert_eq!(log.matches("submit_guarded_prompt").count(), 1);
        assert!(!log.contains("submit_user_prompt"));
        fixture.close().await;
    }
}

#[tokio::test]
async fn native_loop_semantic_ack_correlation_accepts_late_receipt_and_rejects_another_turn() {
    for (mode, expected) in [
        ("guard-late-ack", LoopActivationState::Finished),
        ("guard-foreign-ack", LoopActivationState::Interrupted),
    ] {
        let fixture = fixture(mode).await;
        let activation = fixture.execute(mode).await;
        assert_eq!(activation.state, expected);
        assert_eq!(
            activation.outcome.is_some(),
            expected == LoopActivationState::Finished
        );
        fixture.close().await;
    }
}
