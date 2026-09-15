use super::*;

#[tokio::test]
async fn task_notification_facts_share_lookup_classification_and_hide_subscription_identity() {
    for (status, tool_error) in [
        ("completed", false),
        ("completed", true),
        ("failed", false),
        ("cancelled", false),
        ("input_required", false),
    ] {
        let f = Fixture::new().await;
        let executor = f.running().await;
        let upstream = Upstream::new(f.pool.clone()).await;
        let binding = upstream.binding(TrustedReplayPolicy::NonIdempotent);
        let version = ProtocolVersion::July2026;
        let created = create_task(&f, &upstream, &executor, &binding, version).await;
        let McpCompletion::Deferred {
            task_receipt: Some(receipt),
            ..
        } = &created.completion
        else {
            panic!("expected task receipt");
        };
        let client =
            JournaledMcpClient::new(f.journal.clone(), ByteBudget::new(crate::MAX_MESSAGE_BYTES));
        let permit = client
            .authorize_task_notifications(
                &executor,
                &binding.task_authority(&task_catalog(version)),
                receipt,
            )
            .await
            .unwrap();
        let mut params = task_state(version, status);
        match status {
            "completed" => {
                params["result"] = json!({"content":[{"type":"text","text":"private-tool-result"}],"isError":tool_error,"extension":{"preserved":true}})
            }
            "failed" => params["error"] = json!({"code":-32000,"message":"private-task-failure"}),
            "input_required" => {
                params["inputRequests"] = json!({"private-input":{"method":"elicitation/create","params":{"message":"private-question"}}})
            }
            _ => {}
        }
        params["_meta"] = json!({"io.modelcontextprotocol/subscriptionId":"private-subscription"});
        let mut notice = json!({"jsonrpc":"2.0","method":"notifications/tasks","params":params});
        let mut foreign = notice.clone();
        foreign["params"]["taskId"] = "foreign-task".into();
        assert!(
            client
                .record_task_notification(&permit, &foreign)
                .await
                .is_err()
        );
        client
            .record_task_notification(&permit, &notice)
            .await
            .unwrap();
        notice["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"] = "reconnected".into();
        client
            .record_task_notification(&permit, &notice)
            .await
            .unwrap();
        let records = f
            .journal
            .task_notifications("team", "worker", &created.operation_id, 0, 100)
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        let outcome = &f.operations().await[0].completion;
        match status {
            "completed" if !tool_error => {
                assert!(matches!(outcome, Some(McpCompletion::Succeeded { .. })))
            }
            "input_required" => assert_eq!(outcome.as_ref(), Some(&created.completion)),
            _ => assert!(matches!(outcome, Some(McpCompletion::Failed { .. }))),
        }
        if status == "input_required" {
            assert_eq!(
                f.journal
                    .task_inputs("team", "worker", &created.operation_id, 0, 100)
                    .await
                    .unwrap()
                    .len(),
                1
            );
            notice["params"]["inputRequests"]["private-input"]["params"]["message"] =
                "changed-question".into();
            assert!(
                client
                    .record_task_notification(&permit, &notice)
                    .await
                    .is_err()
            );
            assert!(
                f.journal
                    .task_inputs("team", "worker", &created.operation_id, 0, 100)
                    .await
                    .unwrap()[0]
                    .conflicted
            );
        }
        let stored = serde_json::to_string(&records).unwrap();
        for secret in [
            "private-tool-result",
            "private-subscription",
            "reconnected",
            "private-input",
            "private-question",
        ] {
            assert!(!stored.contains(secret));
        }
        assert_eq!(
            upstream.count(),
            1,
            "notifications cannot send another tool call"
        );
        f.close().await;
    }
}
