use agenthub_agent_domain::loop_runtime::{
    LoopCleanupDisposition, LoopOutcome, LoopOutcomeKind, LoopToolStatus, LoopToolSurface,
};

use super::{lifecycle_tests::running, *};

#[tokio::test]
async fn loop_tool_history_preserves_incomplete_boundaries_and_late_completion_after_reopen() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "private-source-key", 100).await;
    let id = current.activation_id.as_deref().unwrap();
    let mut handles = Vec::new();
    for name in ["list_tasks", "finish_loop_activation", "read_context"] {
        handles.push(
            fixture
                .store
                .begin_tool_observation(&current, LoopToolSurface::ControlRpc, name, None, 100)
                .await
                .unwrap(),
        );
    }
    let first = handles.remove(0);
    fixture
        .store
        .complete_tool_observation(first, LoopToolStatus::Succeeded, 101)
        .await
        .unwrap();
    fixture
        .store
        .finish(
            &current,
            &LoopOutcome {
                kind: LoopOutcomeKind::NoActionableWork,
                wait_reason: None,
                task_note_id: None,
                continuation: None,
            },
            101,
        )
        .await
        .unwrap();
    fixture
        .store
        .cleanup_verified(&current, LoopCleanupDisposition::Exited, 102)
        .await
        .unwrap();
    // Completion is evidence, not executor authority. Wall-clock regression must not corrupt
    // the monotonic duration or prevent recording an observed result after cleanup.
    fixture
        .store
        .complete_tool_observation(handles.remove(0), LoopToolStatus::OutcomeUnknown, 99)
        .await
        .unwrap();
    drop(handles);
    assert!(
        fixture
            .store
            .begin_tool_observation(&current, LoopToolSurface::ControlRpc, "stale", None, 102)
            .await
            .is_err()
    );
    fixture.store.pool.close().await;
    let reopened = LoopStore::new(crate::init_db_at_path(&fixture.path).await.unwrap());
    let page = reopened
        .activation_tool_history("team", "worker", id, None, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.tools.len(), 3);
    assert_eq!(page.tools[0].status, LoopToolStatus::Succeeded);
    assert_eq!(page.tools[1].status, LoopToolStatus::OutcomeUnknown);
    assert_eq!(page.tools[1].completed_at, Some(99));
    assert!(page.tools[1].duration_ms.unwrap() >= 0);
    assert_eq!(page.tools[2].status, LoopToolStatus::Started);
    assert_eq!(page.tools[2].completed_at, None);
    assert_eq!(page.tools[2].duration_ms, None);
    assert!(
        page.tools
            .iter()
            .all(|tool| tool.generation == current.generation)
    );
    assert_eq!(
        reopened
            .events("team", id, 0, 100)
            .await
            .unwrap()
            .iter()
            .filter(|event| event.kind.as_str() == "tool_completed")
            .count(),
        2
    );
    let encoded = serde_json::to_string(&page).unwrap();
    for private in [
        "private-source-key",
        "arguments",
        "output",
        "owner_id",
        "intent_json",
    ] {
        assert!(!encoded.contains(private), "{private}");
    }
    reopened.pool.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn loop_tool_history_fences_recording_and_scopes_every_page_and_cursor() {
    let fixture = Fixture::new().await;
    fixture.enable("worker", &LoopLimits::default()).await;
    let current = running(&fixture, "first", 100).await;
    let id = current.activation_id.as_deref().unwrap();
    let other_id = fixture
        .store
        .accept_trigger(&trigger("second"), 101)
        .await
        .unwrap()
        .activation_id;
    let mut stale = current.clone();
    stale.generation += 1;
    for invalid in [&stale, &current] {
        let now = if invalid.generation == current.generation {
            current.lease_expires_at
        } else {
            100
        };
        assert!(
            fixture
                .store
                .begin_tool_observation(
                    invalid,
                    LoopToolSurface::ControlRpc,
                    "list_tasks",
                    None,
                    now
                )
                .await
                .is_err()
        );
    }
    for name in ["first", "second", "third"] {
        let handle = fixture
            .store
            .begin_tool_observation(
                &current,
                LoopToolSurface::ControlRpc,
                name,
                Some("worker"),
                101,
            )
            .await
            .unwrap();
        fixture
            .store
            .complete_tool_observation(handle, LoopToolStatus::Failed, 101)
            .await
            .unwrap();
    }
    let first = fixture
        .store
        .activation_tool_history("team", "worker", id, None, 1)
        .await
        .unwrap()
        .unwrap();
    let second = fixture
        .store
        .activation_tool_history("team", "worker", id, first.next_cursor, 1)
        .await
        .unwrap()
        .unwrap();
    let third = fixture
        .store
        .activation_tool_history("team", "worker", id, second.next_cursor, 1)
        .await
        .unwrap()
        .unwrap();
    assert!(third.next_cursor.is_none());
    assert_eq!(
        [
            &first.tools[0].tool_name,
            &second.tools[0].tool_name,
            &third.tools[0].tool_name
        ],
        ["first", "second", "third"]
    );
    for (team, actor) in [("elsewhere", "worker"), ("team", "other")] {
        assert!(
            fixture
                .store
                .activation_tool_history(team, actor, id, None, 1)
                .await
                .unwrap()
                .is_none()
        );
    }
    for (activation, after, limit) in [
        (&other_id, first.next_cursor, 1),
        (&id.to_owned(), Some(999999), 1),
        (&id.to_owned(), None, 0),
        (&id.to_owned(), None, 101),
    ] {
        assert!(
            fixture
                .store
                .activation_tool_history("team", "worker", activation, after, limit)
                .await
                .unwrap_err()
                .downcast_ref::<LoopStoreError>()
                .is_some_and(|error| matches!(error, LoopStoreError::InvalidHistoryQuery))
        );
    }
    // Running migration again neither removes observations nor assumes they completed.
    migrate_loop_runtime(&fixture.store.pool).await.unwrap();
    assert_eq!(
        fixture
            .store
            .activation_tool_history("team", "worker", id, None, 100)
            .await
            .unwrap()
            .unwrap()
            .tools
            .len(),
        3
    );
    fixture.close().await;
}
