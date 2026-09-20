use super::*;

#[tokio::test]
async fn mcp_task_notifications_migrate_deduplicate_and_settle_after_executor_exit() {
    let mut f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    sqlx::query("DROP TABLE mcp_operation_task_notifications")
        .execute(&f.store.pool)
        .await
        .unwrap();
    migrate_mcp_operations(&f.store.pool).await.unwrap();
    migrate_mcp_operations(&f.store.pool).await.unwrap();
    let permit = f
        .store
        .authorize_task_notifications(&e, &authority(&op), &receipt(), 104)
        .await
        .unwrap();
    f.stop(&e, 105).await;
    assert!(
        f.store
            .authorize_task_notifications(&e, &authority(&op), &receipt(), 106)
            .await
            .is_err()
    );
    f.store
        .record_task_notification(&permit, &hash(100), None, None, 106)
        .await
        .unwrap();
    assert_eq!(cancellation::completion(&f, &op).await, deferred());
    f.store
        .record_task_notification(&permit, &hash(101), Some(&success()), None, 107)
        .await
        .unwrap();
    f.reopen(true).await;
    f.store
        .record_task_notification(&permit, &hash(101), Some(&success()), None, 108)
        .await
        .unwrap();
    f.store
        .record_task_notification(
            &permit,
            &hash(102),
            Some(&cancellation::cancelled()),
            None,
            109,
        )
        .await
        .unwrap();
    assert_eq!(cancellation::completion(&f, &op).await, success());
    let records = f
        .store
        .task_notifications("team", "worker", &op.id, 0, 2)
        .await
        .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].outcome, Some(success()));
    let last = f
        .store
        .task_notifications("team", "worker", &op.id, records[1].sequence, 100)
        .await
        .unwrap();
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].outcome, Some(cancellation::cancelled()));
    assert!(
        f.store
            .task_notifications("team", "other", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        f.store
            .task_notifications("other", "worker", &op.id, 0, 100)
            .await
            .unwrap()
            .is_empty()
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

#[tokio::test]
async fn mcp_task_notifications_bind_inputs_without_resetting_consumption_or_conflicts() {
    use agenthub_agent_domain::mcp_operations::{
        McpTaskInputRequest, McpTaskInputResponse, McpTaskUpdateInput,
    };
    let mut f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    let scope = authority(&op);
    let permit = f
        .store
        .authorize_task_notifications(&e, &scope, &receipt(), 104)
        .await
        .unwrap();
    let request = McpTaskInputRequest {
        input_id_digest: hash(110),
        request_digest: hash(111),
    };
    f.store
        .record_task_notification(
            &permit,
            &hash(112),
            None,
            Some(std::slice::from_ref(&request)),
            105,
        )
        .await
        .unwrap();
    let update = McpTaskUpdateInput {
        receipt: receipt(),
        request_key: hash(113),
        request_digest: hash(114),
        inputs: vec![McpTaskInputResponse {
            input_id_digest: hash(110),
            response_digest: hash(115),
        }],
    };
    f.store
        .begin_task_update(&e, &scope, &update, 106)
        .await
        .unwrap();
    f.store
        .record_task_notification(
            &permit,
            &hash(112),
            None,
            Some(std::slice::from_ref(&request)),
            107,
        )
        .await
        .unwrap();
    f.store
        .record_task_notification(
            &permit,
            &hash(116),
            None,
            Some(std::slice::from_ref(&request)),
            107,
        )
        .await
        .unwrap();
    assert!(
        f.store
            .task_inputs("team", "worker", &op.id, 0, 100)
            .await
            .unwrap()[0]
            .update_id
            .is_some()
    );
    let changed = McpTaskInputRequest {
        request_digest: hash(117),
        ..request
    };
    assert_journal_error(
        f.store
            .record_task_notification(
                &permit,
                &hash(118),
                None,
                Some(std::slice::from_ref(&changed)),
                108,
            )
            .await,
        McpJournalError::TaskInputConflict,
    );
    f.reopen(false).await;
    assert_journal_error(
        f.store
            .record_task_notification(&permit, &hash(118), None, Some(&[changed]), 109)
            .await,
        McpJournalError::TaskInputConflict,
    );
    let inputs = f
        .store
        .task_inputs("team", "worker", &op.id, 0, 100)
        .await
        .unwrap();
    assert!(inputs[0].conflicted);
    assert_eq!(inputs[0].request_digest, hash(111));
    let records = f
        .store
        .task_notifications("team", "worker", &op.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(records.len(), 3);
    assert!(!records[2].inputs_valid);
    assert_eq!(cancellation::completion(&f, &op).await, deferred());
    f.close().await;
}

#[tokio::test]
async fn mcp_task_notification_admission_checks_original_scope_and_has_bounded_history() {
    let f = Fixture::new().await;
    let e = f.running("worker", 100).await;
    let op = pending(&f, &e).await;
    for field in [
        "actor", "team", "server", "binding", "scope", "schema", "handle", "version", "session",
    ] {
        let mut executor = e.clone();
        let mut scope = authority(&op);
        let mut task = receipt();
        match field {
            "actor" => executor.actor_id = "other".into(),
            "team" => executor.team_id = "other".into(),
            "server" => scope.server_id = "other".into(),
            "binding" => scope.binding_digest = hash(120),
            "scope" => scope.scope_digest = hash(120),
            "schema" => scope.tools.clear(),
            "handle" => task.task_digest = hash(120),
            "version" => task.version = McpTaskVersion::November2025,
            "session" => task.session_digest = Some(hash(120)),
            _ => unreachable!(),
        }
        assert!(
            f.store
                .authorize_task_notifications(&executor, &scope, &task, 104)
                .await
                .is_err(),
            "{field}"
        );
    }
    let permit = f
        .store
        .authorize_task_notifications(&e, &authority(&op), &receipt(), 104)
        .await
        .unwrap();
    sqlx::query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < 4096) \
        INSERT INTO mcp_operation_task_notifications(operation_id, attempt_number, activation_id, response_digest, inputs_valid, observed_at) \
        SELECT ?, 1, ?, printf('%064x', x), 1, 105 FROM n")
        .bind(&op.id).bind(&e.activation_id).execute(&f.store.pool).await.unwrap();
    assert_journal_error(
        f.store
            .record_task_notification(&permit, &hash(5000), None, None, 106)
            .await,
        McpJournalError::ContinuationRequired,
    );
    f.close().await;
}
