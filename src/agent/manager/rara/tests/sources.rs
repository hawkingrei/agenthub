use agenthub_rara::SourceRegistration;

use super::*;

async fn source_attempts(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM runtime_control_receipts WHERE kind IN ('prompt_source', 'skill_source')",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn source_registration_waits_for_each_durable_ack_prefix() {
    for scenario in ["source_delayed", "source_gap"] {
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
                    SourceRegistration::Prompt {
                        source_id: "role".into(),
                        content: "Review the assigned task.".into(),
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
