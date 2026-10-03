use agenthub_agent_event_codec::encode_message_for_storage;
use agenthub_db::runtime_events::{
    RuntimeEventStore, RuntimeHistoryEntry, RuntimeRequestAck, RuntimeRequestIntent,
    RuntimeRequestKind,
};
use serde_json::{Value, json};

use super::*;

#[tokio::test]
async fn native_history_uses_current_receipts_without_rewriting_or_resurrecting_events() {
    let (agents, index, _) = build_agent_manager_with_counting_index().await;
    let actor = insert_agent_with_sessions(&agents.db, "native-history", &["local", "other"]).await;
    let pool = agents.test_event_pool_for_agent(&actor).await.unwrap();
    let store = RuntimeEventStore::bind(pool.clone(), "local", "runtime")
        .await
        .unwrap();
    store.bind_stream("native").await.unwrap();
    let message = json!({
        "type":"user_message", "text":"retained input ".repeat(100), "message_id":"input",
        "meta":{"delivery":"pending", "provider_runtime":{
            "provider":"rara", "runtime_id":"runtime",
            "native_session_id":"native", "request_id":"input"
        }}
    });
    let encoded = encode_message_for_storage(&OutputStream::Acp, &message.to_string());
    let input_id = store
        .prepare_input_request(
            RuntimeRequestIntent {
                request_id: "input",
                kind: RuntimeRequestKind::Prompt,
                target_session_id: Some("native"),
                expected_turn_id: None,
            },
            1,
            RuntimeHistoryEntry {
                seq: "input",
                ts: 1,
                stream: OutputStream::Acp,
                message: &encoded,
            },
        )
        .await
        .unwrap();
    let pending = agents.get_event(&actor, input_id).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&pending.message).unwrap(),
        message
    );
    store
        .prepare_request(
            RuntimeRequestIntent {
                request_id: "control",
                kind: RuntimeRequestKind::Query,
                target_session_id: Some("native"),
                expected_turn_id: None,
            },
            1,
        )
        .await
        .unwrap();
    let permit = store.mark_request_sent("input", 2).await.unwrap();
    store.close(3).await.unwrap();
    let stale_receipt = json!({
        "type":"input_receipt", "message_id":"input",
        "receipt":store.request_receipt("input").await.unwrap().unwrap(),
        "meta":message["meta"],
    });
    let receipt_id =
        insert_agent_event(&pool, "local", "receipt", 3, &stale_receipt.to_string()).await;
    sqlx::query("UPDATE agent_events SET stream = 'acp' WHERE id = ?")
        .bind(receipt_id)
        .execute(&pool)
        .await
        .unwrap();
    // A crash between receipt persistence and conversation projection must be recoverable.
    store
        .record_request_ack(
            &permit,
            RuntimeRequestAck::Accepted {
                session_id: "native".into(),
                turn_id: Some("turn".into()),
                last_sequence: None,
            },
            4,
        )
        .await
        .unwrap();
    for id in [input_id, receipt_id] {
        put_agent_event_ref(&index, &actor, "local", id, 1);
    }
    index
        .put_high_water(&format!("agent_events:agent:{actor}"), receipt_id as u64)
        .unwrap();
    let events = agents.list_events(&actor, 100, None).await.unwrap();
    assert_eq!(index.scan_count(), 1, "exercise fresh indexed history");
    assert_eq!(events.len(), 2);
    let input: Value = serde_json::from_str(&events[0].message).unwrap();
    let receipt: Value = serde_json::from_str(&events[1].message).unwrap();
    assert_eq!(input["meta"]["delivery"], "accepted");
    assert_eq!(receipt["receipt"]["status"], "accepted");
    assert_eq!(receipt["receipt"]["ack"]["turn_id"], "turn");
    let page = agents
        .list_events_for_session(&actor, "local", 1, Some(receipt_id))
        .await
        .unwrap();
    assert_eq!(
        page[0].message, events[0].message,
        "receipt may be on another page"
    );

    // A history row cannot borrow a receipt across any ownership boundary.
    for (path, replacement, session) in [
        ("/meta/provider_runtime/provider", "other", "local"),
        ("/meta/provider_runtime/runtime_id", "other", "local"),
        ("/meta/provider_runtime/native_session_id", "other", "local"),
        ("/meta/provider_runtime/request_id", "other", "local"),
        ("/message_id", "other", "local"),
        ("/message_id", "invalid request", "local"),
        ("/message_id", "control", "local"),
        ("/message_id", "input", "other"),
        ("/type", "agent_message", "local"),
    ] {
        let mut foreign = message.clone();
        *foreign.pointer_mut(path).unwrap() = json!(replacement);
        if path == "/message_id" {
            foreign["meta"]["provider_runtime"]["request_id"] = json!(replacement);
        }
        let id = insert_agent_event(
            &pool,
            session,
            &Uuid::new_v4().to_string(),
            5,
            &foreign.to_string(),
        )
        .await;
        sqlx::query("UPDATE agent_events SET stream = 'acp' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let event = agents.get_event(&actor, id).await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&event.message).unwrap(),
            foreign
        );
    }
    let stored: Vec<u8> = sqlx::query_scalar("SELECT message FROM agent_events WHERE id = ?")
        .bind(input_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        stored, encoded,
        "read projection must not rewrite compressed history"
    );
    sqlx::query("DELETE FROM agent_events")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        agents
            .list_events(&actor, 100, None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(agents.get_event(&actor, input_id).await.is_err());
    assert!(store.request_receipt("input").await.unwrap().is_some());
}
