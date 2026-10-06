use std::time::Duration;

use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, RequestPermissionRequest,
    SelectedPermissionOutcome, ToolCallUpdate, ToolCallUpdateFields,
};
use sqlx::sqlite::SqlitePoolOptions;

use super::{AcpPermissionRespondResult, AcpPermissionService};

#[tokio::test]
async fn permission_is_not_reviewable_before_its_response_callback_can_be_registered() {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("CREATE TABLE acp_permission_requests (id TEXT PRIMARY KEY, agent_id TEXT, session_id TEXT, acp_session_id TEXT, team_id TEXT, requester_actor_id TEXT, requester_role TEXT, tool_call_id TEXT, options_json TEXT, tool_call_json TEXT, status TEXT, created_at INTEGER, selected_option_id TEXT, reviewed_by_actor_id TEXT, responded_at INTEGER)")
        .execute(&db).await.unwrap();
    sqlx::query("CREATE TABLE agent_sessions (id TEXT PRIMARY KEY, status TEXT, ended_at INTEGER)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_sessions VALUES ('local', 'running', NULL)")
        .execute(&db)
        .await
        .unwrap();
    let service = AcpPermissionService::new(db.clone());
    // Block callback registration while an independent reviewer observes durable rows.
    let callback_gate = service.pending.lock().await;
    let creator = service.clone();
    let creation = tokio::spawn(async move {
        creator
            .create_request(
                "actor",
                "local",
                &RequestPermissionRequest::new(
                    "native".to_owned(),
                    ToolCallUpdate::new("tool".to_owned(), ToolCallUpdateFields::default()),
                    vec![PermissionOption::new(
                        "once",
                        "Allow once",
                        PermissionOptionKind::AllowOnce,
                    )],
                ),
                None,
            )
            .await
            .unwrap()
    });
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    // Cancelling pool acquisition can discard the sole in-memory connection and
    // its schema. Bound observation between completed queries instead.
    let premature_publication = loop {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM acp_permission_requests WHERE status = 'pending'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        if count > 0 {
            break true;
        }
        if tokio::time::Instant::now() >= deadline {
            break false;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    };
    drop(callback_gate);
    let (id, response) = creation.await.unwrap();
    let outcome =
        RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("once".to_owned()));
    assert_eq!(
        service
            .respond(&id, outcome.clone(), Some("once".into()), None)
            .await
            .unwrap(),
        AcpPermissionRespondResult::Applied
    );
    assert_eq!(response.await.unwrap(), outcome);
    assert!(service.pending.lock().await.is_empty());
    let status: String = sqlx::query_scalar("SELECT status FROM agent_sessions WHERE id = 'local'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "running");
    assert!(
        !premature_publication,
        "a reviewer can answer before the live callback can receive the response"
    );
}
