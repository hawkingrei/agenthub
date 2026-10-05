use agenthub_rara::SourceRegistration;

use super::*;

async fn source_attempts(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM runtime_control_receipts WHERE kind IN ('prompt_source', 'skill_source', 'mcp_source')",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn source_registration_without_ack_cursor_aborts_before_next_source() {
    assert_invalid_source_cursor("source_missing_cursor").await;
}

#[tokio::test]
async fn source_registration_with_zero_ack_cursor_aborts_before_next_source() {
    assert_invalid_source_cursor("source_zero_cursor").await;
}

#[tokio::test]
async fn source_registration_with_stale_ack_cursor_aborts_before_next_source() {
    assert_invalid_source_cursor("source_stale_cursor").await;
}

async fn assert_invalid_source_cursor(scenario: &str) {
    let fixture = Fixture::new(scenario).await;
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let runtime = fixture.runtime().await;
    let result = runtime
        .register_loop_sources(vec![
            SourceRegistration::Prompt {
                source_id: "role".into(),
                content: "Review the task.".into(),
            },
            SourceRegistration::Skill {
                source_id: "skills".into(),
                name: "review".into(),
                content: "Check the boundary.".into(),
            },
        ])
        .await;
    let pool = fixture
        .manager
        .event_dbs
        .pool_for_agent(&fixture.agent_id)
        .await
        .unwrap();
    let attempts = source_attempts(&pool).await;
    let sent = std::fs::read_to_string(fixture.directory.join("requests.jsonl"))
        .unwrap()
        .lines()
        .count();
    // Clean up the old implementation as well, so a regression cannot leak a provider.
    if result.is_ok() {
        fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    } else {
        fixture.assert_clean().await;
    }
    fixture.finish().await;
    assert!(
        result.is_err(),
        "a source ACK must identify its committed prefix"
    );
    assert_eq!(attempts, 1);
    assert_eq!(sent, 1);
}

#[tokio::test]
async fn source_registration_waits_for_each_durable_ack_prefix() {
    for (scenario, mcp) in [
        ("source_delayed", false),
        ("source_gap", false),
        ("source_delayed", true),
        ("source_gap", true),
    ] {
        let fixture = Fixture::new(scenario).await;
        fixture
            .manager
            .start_agent(&fixture.agent_id)
            .await
            .unwrap();
        let runtime = fixture.runtime().await;
        let registration = tokio::spawn(async move {
            runtime
                .register_loop_sources(vec![
                    if mcp {
                        mcp_source()
                    } else {
                        SourceRegistration::Prompt {
                            source_id: "role".into(),
                            content: "Review the assigned task.".into(),
                        }
                    },
                    SourceRegistration::Skill {
                        source_id: "skills".into(),
                        name: "review".into(),
                        content: "Check the task boundary.".into(),
                    },
                ])
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !fixture.directory.join("source-ack").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let pool = fixture
            .manager
            .event_dbs
            .pool_for_agent(&fixture.agent_id)
            .await
            .unwrap();
        // Give an incorrectly pipelined second control time to reach the durable ledger.
        let _ = tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                if source_attempts(&pool).await > 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        let before = source_attempts(&pool).await;
        let cursor: i64 = sqlx::query_scalar("SELECT last_sequence FROM runtime_event_streams")
            .fetch_one(&pool)
            .await
            .unwrap();
        std::fs::write(fixture.directory.join("release-source"), "ready").unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), registration)
            .await
            .unwrap()
            .unwrap();
        if scenario == "source_gap" {
            fixture.assert_clean().await;
        } else {
            fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
        }
        let after = source_attempts(&pool).await;
        fixture.finish().await;

        assert_eq!(
            before, 1,
            "the next source must not be prepared before the first event commits"
        );
        assert_eq!(cursor, 1, "an ACK does not advance durable event history");
        assert_eq!(result.is_ok(), scenario == "source_delayed");
        assert_eq!(after, if scenario == "source_delayed" { 2 } else { 1 });
    }
}

fn mcp_source() -> SourceRegistration {
    SourceRegistration::Mcp(agenthub_rara::McpSource {
        source_id: "nowledge-mem".into(),
        command: "/usr/bin/agenthub".into(),
        args: vec![
            "mcp-proxy".into(),
            "--server-id".into(),
            "nowledge-mem".into(),
        ],
        env: [(
            "AGENTHUB_LOOP_CREDENTIAL_FILE".into(),
            "/private/activation.json".into(),
        )]
        .into(),
    })
}

#[tokio::test]
async fn missing_mcp_capability_rejects_the_entire_bootstrap_before_any_send() {
    let fixture = Fixture::new("source_missing_mcp").await;
    fixture
        .manager
        .start_agent(&fixture.agent_id)
        .await
        .unwrap();
    let result = fixture
        .runtime()
        .await
        .register_loop_sources(vec![
            SourceRegistration::Prompt {
                source_id: "role".into(),
                content: "Review the task".into(),
            },
            mcp_source(),
        ])
        .await;
    assert!(result.is_err());
    let pool = fixture
        .manager
        .event_dbs
        .pool_for_agent(&fixture.agent_id)
        .await
        .unwrap();
    assert_eq!(source_attempts(&pool).await, 0);
    assert!(!fixture.directory.join("requests.jsonl").exists());
    fixture.manager.stop_agent(&fixture.agent_id).await.unwrap();
    fixture.finish().await;
}
