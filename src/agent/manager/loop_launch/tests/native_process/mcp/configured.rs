use anyhow::{Context, ensure};

use super::super::configured::{configure, execute, set_member};
use super::*;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn configured_provider_controlled_tools_preserve_scope_and_uncertainty() {
    for uncertain in ["false", "true"] {
        super::super::super::mcp::run_configured_child_with_timeout(
            "agent::manager::loop_launch::tests::native_process::mcp::configured::configured_provider_proxy_child",
            &[("TEST_NATIVE_APP_TOKEN", "native-app-secret"), ("TEST_NATIVE_UNCERTAIN", uncertain)],
            Duration::from_secs(240),
        ).await;
    }
}

#[tokio::test]
#[ignore = "isolated environment child of the configured-provider proxy test"]
async fn configured_provider_proxy_child() {
    let upstream = Arc::new(Upstream {
        uncertain: std::env::var("TEST_NATIVE_UNCERTAIN").unwrap() == "true",
        revoked: std::sync::atomic::AtomicBool::new(false),
        calls: StdMutex::new(Vec::new()),
        requests: StdMutex::new(Vec::new()),
        owner: StdMutex::new(None),
        finish_command: StdMutex::new(String::new()),
    });
    let (mut fixture, server) = proxy_fixture(upstream.clone()).await;
    let result = check(&mut fixture, &upstream).await;
    fixture.close().await;
    server.abort();
    result.unwrap();
}

async fn check(fixture: &mut Fixture, upstream: &Upstream) -> anyhow::Result<()> {
    configure(fixture).await?;
    let finish = upstream.finish_command.lock().unwrap().clone();
    let instructions = format!(
        "Perform this isolated controlled-tool acceptance in order. Each step is exactly one \
         tool call; wait for its result before the next step. \
         1. Call the tool described as 'Native Mem write' with body='foreign' and space_id='foreign'; denial is expected. \
         2. Call 'Native Mem write' with body='native-memory' and omit space_id. \
         3. Call 'Native App write' with body='native-app'. The write may report an uncertain result; never repeat it. \
         4. Call 'Native App write' with body='after-revocation'; access is now revoked, so denial is expected. \
         5. Call bash with command={finish:?}, sandbox_permissions=require_escalated, \
         justification=Finish the isolated controlled-tool fixture, current workspace and no environment overrides. \
         The prepared outcome file is complete. Do not inspect or edit files. Do not call other tools."
    );
    set_member(fixture, "worker", json!({
        "description":"Performs isolated synthetic acceptance of controlled App and memory tool authorization.",
        "prompt_append":instructions,
    })).await?;
    let reservation = fixture.admit("configured-proxy").await;
    let activation = execute(fixture, reservation, Some(&finish)).await?;
    let calls = upstream.calls.lock().unwrap().clone();
    ensure!(
        calls.len() == 2,
        "foreign, revoked or repeated write reached upstream"
    );
    ensure!(
        calls[0]["params"]["arguments"] == json!({"body":"native-memory", "space_id":"space-a"})
    );
    ensure!(calls[1]["params"]["arguments"] == json!({"body":"native-app"}));
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM mcp_operation_attempts ORDER BY rowid")
            .fetch_all(&fixture.state.db)
            .await?;
    ensure!(
        statuses
            .iter()
            .filter(|status| *status == "succeeded")
            .count()
            == if upstream.uncertain { 1 } else { 2 }
    );
    ensure!(
        statuses
            .iter()
            .filter(|status| *status == "outcome_unknown")
            .count()
            == usize::from(upstream.uncertain)
    );
    let session = activation.session_id.as_deref().context("session")?;
    let history = fixture
        .state
        .agents
        .runtime_history("worker", session, 100, None)
        .await?
        .context("history")?;
    ensure!(
        history
            .receipts
            .iter()
            .filter(|receipt| receipt.kind == RuntimeRequestKind::McpSource
                && receipt.status == RuntimeRequestStatus::Accepted)
            .count()
            == 2
    );
    let events: Vec<Value> = fixture
        .state
        .agents
        .list_events_for_session("worker", session, 500, None)
        .await?
        .iter()
        .filter_map(|event| serde_json::from_str(&event.message).ok())
        .collect();
    for body in ["foreign", "after-revocation"] {
        let call = events
            .iter()
            .find(|event| event["type"] == "tool_call" && event["raw_input"]["body"] == body)
            .context("denial probe was not attempted")?;
        ensure!(
            events
                .iter()
                .any(|event| event["type"] == "tool_call_update"
                    && event["id"] == call["id"]
                    && event["status"] == "failed"),
            "controlled tool denial was not preserved"
        );
    }
    let inherited: Vec<String> = serde_json::from_slice(&std::fs::read(
        fixture.directory.join("native-environment.json"),
    )?)?;
    ensure!(
        inherited.is_empty(),
        "upstream credentials reached the native process"
    );
    ensure!(
        upstream.requests.lock().unwrap().is_empty(),
        "mock provider was called"
    );
    Ok(())
}
