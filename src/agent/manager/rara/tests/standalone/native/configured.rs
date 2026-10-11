//! Explicitly opt-in checks that send synthetic prompts to a configured provider.

use anyhow::{Context, ensure};

use super::*;

mod approval;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn standalone_configured_provider_recalls_history_after_manager_restart() {
    let mut native = fixture().await;
    let result = check_continuity(&mut native).await;
    native.close().await;
    result.unwrap();
}

async fn fixture() -> NativeFixture {
    let path = std::env::var_os("AGENTHUB_RARA_PROVIDER_CONFIG")
        .expect("explicit private provider configuration path");
    let config = std::fs::read(path).expect("read explicit provider configuration");
    let mut native = NativeFixture::new("clean").await;
    std::fs::set_permissions(
        &native.fixture.directory,
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let state = native.fixture.directory.join("native-state");
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = state.join("config.json");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(path, config).unwrap();
    let mut app_config = (*native.fixture.manager.loop_app_config).clone();
    let runtime_config = app_config.rara.as_mut().unwrap();
    runtime_config.startup_timeout_seconds = Some(30);
    runtime_config.shutdown_timeout_seconds = Some(5);
    native.fixture.manager = native
        .fixture
        .manager
        .clone()
        .with_loop_app_config(app_config);

    native
}

async fn check_continuity(native: &mut NativeFixture) -> anyhow::Result<()> {
    let marker = format!("continuity-{}", Uuid::new_v4());
    let first = native.start().await;
    native
        .prompt(
            &first,
            &format!(
                "Remember this continuity code for this conversation: {marker}. \
                 Reply with exactly READY. Do not call any tools."
            ),
        )
        .await;
    let text = wait_for_response(native, &first, false).await?;
    ensure!(
        text.trim() == "READY",
        "provider did not acknowledge the synthetic instruction"
    );
    native.restart_manager().await;
    let second = native.start().await;
    native.assert_resume(&first, &second).await;
    native
        .prompt(
            &second,
            "What continuity code did I give in the preceding instruction? \
             Reply with only that code. Do not call any tools.",
        )
        .await;
    let text = wait_for_response(native, &second, false).await?;
    ensure!(
        text.trim() == marker,
        "provider did not recall the earlier conversation"
    );
    ensure!(
        native.requests.lock().await.is_empty(),
        "mock provider was called"
    );
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM acp_permission_requests WHERE agent_id = ?")
            .bind(&native.fixture.agent_id)
            .fetch_one(&native.fixture.manager.db)
            .await?;
    ensure!(
        pending == 0,
        "unexpected approval during the tool-free continuity probe"
    );
    Ok(())
}

async fn wait_for_response(
    native: &NativeFixture,
    local: &str,
    allow_tools: bool,
) -> anyhow::Result<String> {
    let runtime = native.fixture.runtime().await;
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            // Recovery inspection is unavailable while the model turn is running.
            let recovery = runtime.query_recovery().await.ok();
            if let Some(view) = &recovery {
                ensure!(view.recovery.blocked.is_none(), "unexpected recovery block");
            }
            let events = native
                .fixture
                .manager
                .list_events(&native.fixture.agent_id, 500, None)
                .await?;
            let mut text = String::new();
            for event in events.iter().filter(|event| event.session_id == local) {
                ensure!(
                    event.message != "Runtime turn failed.",
                    "configured provider turn failed"
                );
                if let Ok(value) = serde_json::from_str::<Value>(&event.message) {
                    ensure!(
                        allow_tools || value["type"] != "tool_call",
                        "unexpected tool call"
                    );
                    if value["type"] == "agent_message" {
                        text.push_str(value["text"].as_str().unwrap_or_default());
                    }
                }
            }
            if recovery.is_some_and(|view| view.recovery.waiting_turn_id.is_none())
                && !text.is_empty()
            {
                return Ok(text);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .context("configured provider response timed out")?
}
