use super::*;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn configured_team_restores_approval_under_a_new_manager() {
    let mut configured = ConfiguredFixture::new().await;
    let result = check(&mut configured, false).await;
    configured.close().await;
    result.unwrap();
}

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn configured_team_reviews_uncertain_effects_before_new_durable_input() {
    let mut configured = ConfiguredFixture::new().await;
    let result = check(&mut configured, true).await;
    configured.close().await;
    result.unwrap();
}

async fn check(configured: &mut ConfiguredFixture, uncertain: bool) -> anyhow::Result<()> {
    configured.configure().await?;
    let command = if uncertain {
        "printf 'executed\\n' >> configured-team-effects; sleep 60"
    } else {
        "printf 'executed\\n' >> configured-team-effects"
    };
    let instruction = format!(
        "CONFIGURED-TEAM-APPROVAL: Run exactly one bash call with command={command:?}, \
         sandbox_permissions=require_escalated and justification=Verify isolated Team recovery. \
         Use the current workspace with no environment overrides. Wait for approval before execution. \
         Do not call other tools or repeat the operation. After it finishes, reply APPROVAL-COMPLETE."
    );
    let effects = configured
        .native
        .fixture
        .directory
        .join("configured-team-effects");
    let first = configured.admit("initial-approval", &instruction).await?;
    let first_id = first.activation_id.clone().context("activation")?;
    let crash = async {
        let old = configured.pending(&first_id).await?;
        verify_command(&configured.native.fixture, &first_id, &old.0, command).await?;
        ensure!(!effects.exists(), "effect happened before approval");
        if uncertain {
            ensure!(
                configured.native.answer(&old.0, "once").await
                    == crate::acp::AcpPermissionRespondResult::Applied
            );
            tokio::time::timeout(Duration::from_secs(15), async {
                while !effects.exists() {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .context("approved append did not begin")?;
        }
        configured.native.crash();
        Ok::<_, anyhow::Error>(old)
    };
    let (first, old) = tokio::try_join!(configured.run(first), crash)?;
    ensure!(first.state == LoopActivationState::Interrupted && first.outcome.is_none());
    configured.restart().await?;
    let second = configured.admit("restore-approval", &instruction).await?;
    let second_id = second.activation_id.clone().context("activation")?;
    let interact = async {
        if uncertain {
            let view = configured.recovery().await?;
            ensure!(view.local_session_id != old.1 && view.recovery.waiting_turn_id.is_none());
            ensure!(
                configured.native.answer(&old.0, "once").await
                    != crate::acp::AcpPermissionRespondResult::Applied
            );
            ensure!(
                view.recovery
                    .decisions
                    .iter()
                    .any(|decision| decision.state == agenthub_rara::DecisionState::Uncertain)
            );
            let blocked = view.recovery.blocked.context("uncertain recovery block")?;
            ensure!(std::fs::read_to_string(&effects)? == "executed\n");
            ensure!(
                configured
                    .native
                    .fixture
                    .state
                    .agents
                    .send_input(
                        "worker",
                        "Do not replay",
                        Some("blocked-input"),
                        Some(&view.local_session_id),
                    )
                    .await
                    .is_err()
            );
            configured
                .native
                .fixture
                .state
                .agents
                .reconcile_native_recovery(
                    "worker",
                    &view.local_session_id,
                    agenthub_rara::RecoveryTarget {
                        runtime_id: view.runtime_id,
                        session_id: view.session_id,
                        recovery_id: blocked.recovery_id,
                    },
                    "Confirmed one isolated append; do not repeat it".into(),
                )
                .await?;
        } else {
            let current = configured.pending(&second_id).await?;
            ensure!(current.0 != old.0 && current.1 != old.1 && !effects.exists());
            ensure!(
                configured.native.answer(&old.0, "once").await
                    != crate::acp::AcpPermissionRespondResult::Applied
            );
            verify_command(&configured.native.fixture, &second_id, &current.0, command).await?;
            ensure!(
                configured.native.answer(&current.0, "once").await
                    == crate::acp::AcpPermissionRespondResult::Applied
            );
        }
        Ok::<_, anyhow::Error>(())
    };
    let (second, ()) = tokio::try_join!(configured.run(second), interact)?;
    configured.assert_resume(&first, &second).await?;
    let history = configured.history(&second).await?;
    ensure!(history.receipts.iter().any(|receipt| receipt.kind
        == RuntimeRequestKind::EvaluateReentry
        && receipt.status == RuntimeRequestStatus::Accepted));
    ensure!(!history.receipts.iter().any(|receipt| matches!(
        receipt.kind,
        RuntimeRequestKind::Prompt | RuntimeRequestKind::GuardedPrompt
    )));
    if uncertain {
        let outcome = second.outcome.as_ref().context("recovery outcome")?;
        ensure!(
            second.state == LoopActivationState::Finished && outcome.kind.as_str() == "waiting"
        );
        ensure!(
            outcome
                .wait_reason
                .is_some_and(|reason| reason.as_str() == "input")
                && outcome.continuation.is_none()
        );
        ensure!(history.receipts.iter().any(|receipt| receipt.kind
            == RuntimeRequestKind::ResolveRecovery
            && receipt.status == RuntimeRequestStatus::Accepted));
        ensure!(
            !history
                .receipts
                .iter()
                .any(|receipt| receipt.kind == RuntimeRequestKind::ShellAnswer)
        );
        configured.restart().await?;
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loop_activations WHERE actor_id = 'worker' AND state = 'pending'",
        )
        .fetch_one(&configured.native.fixture.state.db)
        .await?;
        ensure!(pending == 0, "recovery scheduled automatic continuation");
        let third = configured.admit("explicit-after-review",
            "CONFIGURED-TEAM-REVIEWED: The interrupted operation was reviewed. Do not call any tools or repeat it. Reply only RECOVERY-COMPLETE.",
        ).await?;
        let third = configured.run(third).await?;
        ensure!(third.state == LoopActivationState::Interrupted && third.outcome.is_none());
        configured.assert_resume(&second, &third).await?;
        ensure!(configured.assistant_text(&third, false).await?.trim() == "RECOVERY-COMPLETE");
    } else {
        ensure!(second.state == LoopActivationState::Interrupted && second.outcome.is_none());
        ensure!(
            configured
                .assistant_text(&second, true)
                .await?
                .contains("APPROVAL-COMPLETE")
        );
        ensure!(
            history
                .receipts
                .iter()
                .filter(|receipt| receipt.kind == RuntimeRequestKind::ShellAnswer
                    && receipt.status == RuntimeRequestStatus::Accepted)
                .count()
                == 1
        );
    }
    ensure!(
        std::fs::read_to_string(effects)? == "executed\n",
        "effect was repeated"
    );
    configured.assert_task_unchanged().await?;
    Ok(())
}
