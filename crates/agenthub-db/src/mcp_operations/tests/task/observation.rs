use super::*;
use agenthub_agent_domain::mcp_operations::{McpTaskObservation, McpTaskObservationBinding};

#[tokio::test]
async fn mcp_task_observation_owner_fences_scope_and_preserves_late_facts() {
    let mut f = Fixture::new().await;
    let executor = f.running("worker", 100).await;
    let other = f.running("other", 100).await;
    let op = pending(&f, &executor).await;
    let owner = f
        .store
        .authorize_task_observation_owner(&executor, 104)
        .await
        .unwrap();
    let foreign = f
        .store
        .authorize_task_observation_owner(&other, 104)
        .await
        .unwrap();
    let scope = authority(&op);
    let binding = McpTaskObservationBinding {
        server_id: scope.server_id,
        scope_digest: scope.scope_digest,
        binding_digest: scope.binding_digest,
    };
    let observation = McpTaskObservation {
        receipt: receipt(),
        response_digest: hash(200),
        outcome: Some(success()),
        inputs: None,
    };
    assert_journal_error(
        f.store
            .record_scoped_task_notification(&foreign, &binding, &observation, 105)
            .await,
        McpJournalError::TaskReceiptMissing,
    );
    for field in ["server", "scope", "binding", "handle", "version", "session"] {
        let mut scoped = binding.clone();
        let mut fact = McpTaskObservation {
            receipt: receipt(),
            response_digest: hash(200),
            outcome: Some(success()),
            inputs: None,
        };
        match field {
            "server" => scoped.server_id = "other".into(),
            "scope" => scoped.scope_digest = hash(201),
            "binding" => scoped.binding_digest = hash(201),
            "handle" => fact.receipt.task_digest = hash(201),
            "version" => fact.receipt.version = McpTaskVersion::November2025,
            "session" => fact.receipt.session_digest = Some(hash(201)),
            _ => unreachable!(),
        }
        assert_journal_error(
            f.store
                .record_scoped_task_notification(&owner, &scoped, &fact, 105)
                .await,
            McpJournalError::TaskReceiptMissing,
        );
    }
    assert!(
        f.store
            .task_notifications("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    f.stop(&executor, 106).await;
    assert!(
        f.store
            .authorize_task_observation_owner(&executor, 107)
            .await
            .is_err()
    );
    f.store
        .record_scoped_task_notification(&owner, &binding, &observation, 107)
        .await
        .unwrap();
    f.reopen(true).await;
    f.store
        .record_scoped_task_notification(&owner, &binding, &observation, 108)
        .await
        .unwrap();
    let later = McpTaskObservation {
        response_digest: hash(202),
        outcome: Some(cancellation::cancelled()),
        ..observation
    };
    f.store
        .record_scoped_task_notification(&owner, &binding, &later, 109)
        .await
        .unwrap();
    assert_eq!(cancellation::completion(&f, &op).await, success());
    assert_eq!(
        f.store
            .task_notifications("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        f.store
            .attempts("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    f.close().await;
}
