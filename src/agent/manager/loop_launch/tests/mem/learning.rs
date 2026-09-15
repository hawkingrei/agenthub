use super::*;

const READS: [&str; 4] = [
    "memory_search",
    "read_working_memory",
    "thread_search",
    "search_source_chunks",
];

pub(super) fn tools(failure: &str) -> Vec<Value> {
    let mut tools: Vec<Value> = READS
        .iter()
        .map(|name| {
            let mut properties = json!({"space_id":{"type":"string"}});
            if *name != "read_working_memory" {
                properties["query"] = json!({"type":"string"});
            }
            json!({"name":name,"inputSchema":{"type":"object","additionalProperties":false,"properties":properties}})
        })
        .collect();
    // Match the native Cloud fields. Caller-ID upsert is not in that deployment's contract.
    let mut properties = json!({"content":{"type":"string","minLength":1,"maxLength":32768}});
    if failure != "learning-legacy-schema" {
        properties["space_id"] = json!({"anyOf":[{"type":"string"},{"type":"null"}]});
        properties["source_grounding"] = json!({"anyOf":[{"type":"string"},{"type":"null"}]});
    }
    tools.push(json!({"name":"memory_add","inputSchema":{"type":"object","required":["content"],"additionalProperties":false,"properties":properties},
        "annotations":{"idempotentHint":true,"readOnlyHint":true}}));
    tools
}

pub(super) fn call(state: &Upstream, message: &Value) -> Response {
    let name = message["params"]["name"].as_str().unwrap();
    let arguments = &message["params"]["arguments"];
    let result = if READS.contains(&name) {
        let expected = if name == "read_working_memory" {
            json!({"space_id":"space-a"})
        } else {
            json!({"space_id":"space-a","query":"original retry identity"})
        };
        assert_eq!(*arguments, expected);
        let count = state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| {
                request["method"] == "tools/call" && request["params"]["name"] == name
            })
            .count();
        if state.failure == "learning-unknown" && count == 1 {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        json!({"results":[],"fixture_read":name})
    } else {
        assert_eq!(name, "memory_add");
        let declared = tools(state.failure).pop().unwrap()["inputSchema"]["properties"]
            .as_object()
            .unwrap()
            .clone();
        assert!(
            arguments
                .as_object()
                .unwrap()
                .keys()
                .all(|key| declared.contains_key(key))
        );
        if declared.contains_key("space_id") {
            assert_eq!(arguments["space_id"], "space-a");
        }
        state.writes.lock().unwrap().push(arguments.clone());
        if state.failure == "learning-unknown" {
            // The write has taken effect, but the response ends without a JSON-RPC receipt.
            return Response::builder()
                .header("content-type", "application/json")
                .body(axum::body::Body::empty())
                .unwrap();
        }
        json!({"id":"87c832f8-58ee-444f-83cf-6fd796a96e4f","status":"created","space_id":"space-a"})
    };
    Json(json!({"jsonrpc":"2.0","id":message["id"],"result":{"content":[{"type":"text","text":result.to_string()}]}})).into_response()
}

#[tokio::test]
async fn selected_learning_preserves_provenance_and_uncertain_writes_across_activations() {
    super::super::mcp::run_configured_child(
        "agent::manager::loop_launch::tests::mem::learning::selected_learning_child",
    )
    .await;
}

#[tokio::test]
#[ignore = "Executed by the parent with an isolated inherited environment"]
async fn selected_learning_child() {
    for case in [
        "learning-success",
        "learning-legacy-schema",
        "learning-unknown",
    ] {
        let (fixture, upstream, server) = fixture(case).await;
        let legacy = fixture.directory.join(".agenthubmemory/legacy.md");
        std::fs::create_dir(legacy.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy,
            "# Original retry identity\nRetain original evidence when reconciling.\n",
        )
        .unwrap();
        std::fs::write(fixture.directory.join("learning-case"), case).unwrap();
        std::fs::write(
            fixture.directory.join("unselected-transcript.txt"),
            "Private transcript must stay local",
        )
        .unwrap();
        let first = fixture.execute("learning-first").await;
        assert_progress(&fixture, &first, "mem_context_ready").await;
        let task_id = std::fs::read_to_string(fixture.directory.join("local-task-id")).unwrap();
        let selected =
            std::fs::read_to_string(fixture.directory.join("selected-learning.json")).unwrap();
        let writes = upstream.writes.lock().unwrap().clone();
        assert_eq!(writes.len(), 1);
        let write = &writes[0];
        let grounding = write["source_grounding"].as_str().unwrap_or_else(|| {
            write["content"]
                .as_str()
                .unwrap()
                .split_once("Source evidence: ")
                .unwrap()
                .1
        });
        let grounding: Value = serde_json::from_str(grounding).unwrap();
        assert_eq!(grounding["task_id"], task_id);
        assert_eq!(grounding["activation_id"], first.id);
        assert_eq!(
            grounding["artifacts"],
            json!(["artifacts/retry-evidence.md", ".agenthubmemory/legacy.md"])
        );
        assert!(!write.to_string().contains("Private transcript"));
        assert!(!write.to_string().contains("Independent local progress"));
        assert_eq!(
            std::fs::read_to_string(&legacy).unwrap(),
            "# Original retry identity\nRetain original evidence when reconciling.\n"
        );
        if case == "learning-unknown" {
            let second = fixture.execute("learning-second").await;
            assert_ne!(first.id, second.id);
            assert_progress(&fixture, &second, "mem_context_ready").await;
            assert_eq!(
                upstream.writes.lock().unwrap().len(),
                1,
                "new activation and JSON-RPC ID cannot replay the uncertain write"
            );
            assert_eq!(
                std::fs::read_to_string(fixture.directory.join("selected-learning.json")).unwrap(),
                selected
            );
            let log = std::fs::read_to_string(fixture.directory.join("requests.jsonl")).unwrap();
            let responses: Vec<Value> = log
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .filter_map(|event| event.get("learning_response").cloned())
                .collect();
            assert_eq!(responses.len(), 2);
            assert_ne!(responses[0]["id"], responses[1]["id"]);
            assert_eq!(responses[1]["error"]["code"], -32000);
            for name in READS {
                let requests = upstream.requests.lock().unwrap();
                let calls: Vec<_> = requests
                    .iter()
                    .filter(|request| {
                        request["method"] == "tools/call" && request["params"]["name"] == name
                    })
                    .collect();
                assert_eq!(
                    calls.len(),
                    2,
                    "trusted retrieval can recover after uncertain transport"
                );
                assert_eq!(calls[0]["params"], calls[1]["params"]);
            }
        }
        let notes = fixture
            .state
            .teams
            .list_task_notes(&task_id, 20)
            .await
            .unwrap();
        let receipt = if case == "learning-unknown" {
            "unresolved"
        } else {
            "retained"
        };
        assert!(notes.iter().any(
            |note| note.text.contains(receipt) && note.text.contains("selected-learning.json")
        ));
        if case != "learning-unknown" {
            assert!(
                notes
                    .iter()
                    .any(|note| note.text.contains("87c832f8-58ee-444f-83cf-6fd796a96e4f"))
            );
        }
        let operations: Vec<(String, String)> =
            sqlx::query_as("SELECT intent_json, status FROM mcp_operations")
                .fetch_all(&fixture.state.db)
                .await
                .unwrap();
        let mut writes = 0;
        for (intent, status) in operations {
            assert!(
                !intent.contains("original evidence") && !intent.contains("Private transcript")
            );
            let intent: Value = serde_json::from_str(&intent).unwrap();
            if intent["tool_name"] == "memory_add" {
                writes += 1;
                assert_eq!(intent["replay_safety"]["kind"], "non_idempotent");
                assert_eq!(
                    status,
                    if case == "learning-unknown" {
                        "outcome_unknown"
                    } else {
                        "succeeded"
                    }
                );
            } else {
                assert_eq!(intent["replay_safety"]["kind"], "read_only");
            }
        }
        assert_eq!(writes, 1);
        fixture.close().await;
        server.abort();
    }
}
