use super::*;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn standalone_configured_provider_restores_approval_under_new_owner() {
    let mut native = fixture().await;
    let result = check_approval(&mut native, false).await;
    native.close().await;
    result.unwrap();
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn standalone_configured_provider_does_not_repeat_uncertain_effects() {
    let mut native = fixture().await;
    let result = check_approval(&mut native, true).await;
    native.close().await;
    result.unwrap();
}

async fn check_approval(native: &mut NativeFixture, uncertain: bool) -> anyhow::Result<()> {
    let command = if uncertain {
        "printf 'executed\\n' >> configured-effects; sleep 60"
    } else {
        "printf 'executed\\n' >> configured-effects"
    };
    let effects = native.fixture.directory.join("configured-effects");
    let first = native.start().await;
    native
        .prompt(
            &first,
            &format!(
                "Run exactly one bash tool call with command {command:?}, \
                 sandbox_permissions=require_escalated and justification=Verify isolated approval recovery. \
                 Use the current workspace, no environment overrides and no other tools. \
                 Wait for approval before execution. After the tool finishes, reply APPROVAL-COMPLETE."
            ),
        )
        .await;
    let old = pending(native).await?;
    verify_command(native, &old, command).await?;
    ensure!(!effects.exists(), "tool ran before approval");
    if uncertain {
        ensure!(native.answer(&old).await == crate::acp::AcpPermissionRespondResult::Applied);
        tokio::time::timeout(Duration::from_secs(15), async {
            while !effects.exists() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .context("approved append did not begin")?;
    }
    native.crash().await;
    native.restart_manager().await;
    let second = native.start().await;
    native.assert_resume(&first, &second).await;
    ensure!(native.answer(&old).await != crate::acp::AcpPermissionRespondResult::Applied);
    if uncertain {
        let runtime = native.fixture.runtime().await;
        let view = runtime.query_recovery().await?;
        ensure!(view.recovery.waiting_turn_id.is_none());
        ensure!(
            view.recovery
                .decisions
                .iter()
                .any(|decision| decision.state == agenthub_rara::DecisionState::Uncertain)
        );
        let blocked = view
            .recovery
            .blocked
            .context("missing uncertain recovery block")?;
        runtime
            .reconcile_recovery(
                agenthub_rara::RecoveryTarget {
                    runtime_id: view.runtime_id,
                    session_id: view.session_id,
                    recovery_id: blocked.recovery_id,
                },
                "Confirmed one isolated append; do not repeat the command".into(),
            )
            .await?;
        let view = runtime.query_recovery().await?;
        ensure!(view.recovery.blocked.is_none() && view.recovery.waiting_turn_id.is_none());
        ensure!(std::fs::read_to_string(&effects)? == "executed\n");
        native
            .prompt(
                &second,
                "The interrupted operation was reviewed. Do not run any tools or repeat it. \
                 Reply with only RECOVERY-COMPLETE.",
            )
            .await;
        let text = wait_for_response(native, &second, false).await?;
        ensure!(text.trim() == "RECOVERY-COMPLETE");
    } else {
        let current = pending(native).await?;
        ensure!(old != current && !effects.exists());
        verify_command(native, &current, command).await?;
        ensure!(native.answer(&current).await == crate::acp::AcpPermissionRespondResult::Applied);
        let text = wait_for_response(native, &second, true).await?;
        ensure!(text.contains("APPROVAL-COMPLETE"));
    }
    ensure!(
        std::fs::read_to_string(effects)? == "executed\n",
        "unexpected repeated effect"
    );
    ensure!(
        native.requests.lock().await.is_empty(),
        "mock provider was called"
    );
    Ok(())
}

async fn pending(native: &NativeFixture) -> anyhow::Result<String> {
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let id: Option<String> = sqlx::query_scalar(
                "SELECT id FROM acp_permission_requests WHERE agent_id = ? AND status = 'pending' ORDER BY created_at DESC LIMIT 1",
            )
            .bind(&native.fixture.agent_id)
            .fetch_optional(&native.fixture.manager.db)
            .await?;
            if let Some(id) = id {
                return Ok(id);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .context("configured provider did not request approval")?
}

async fn verify_command(native: &NativeFixture, id: &str, command: &str) -> anyhow::Result<()> {
    let record = native
        .fixture
        .manager
        .permissions
        .get(id)
        .await?
        .context("approval missing")?;
    let call = record.tool_call.context("approval tool details missing")?;
    let input = &call["rawInput"];
    ensure!(
        input["command"] == command,
        "unexpected command; refusing approval"
    );
    ensure!(input["program"].is_null());
    ensure!(input["args"].as_array().is_none_or(Vec::is_empty));
    ensure!(
        input["env"]
            .as_object()
            .is_none_or(serde_json::Map::is_empty)
    );
    ensure!(input["allow_net"] != true && input["run_in_background"] != true);
    ensure!(input["sandbox_permissions"] == "require_escalated");
    if let Some(cwd) = input["cwd"].as_str() {
        ensure!(cwd == "." || std::fs::canonicalize(cwd)? == native.fixture.directory);
    }
    Ok(())
}
