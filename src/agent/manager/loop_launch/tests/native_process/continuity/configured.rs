//! Provider-independent recovery oracles over reopened production-schema state.

use anyhow::{Context, ensure};

use super::super::configured::{configure, set_member, verify_command};
use super::*;

mod approval;
mod fixture;

use fixture::ConfiguredFixture;

#[tokio::test]
#[ignore = "requires AGENTHUB_RARA_TEST_BINARY and a private AGENTHUB_RARA_PROVIDER_CONFIG"]
async fn configured_team_recalls_history_after_database_and_manager_restart() {
    let mut configured = ConfiguredFixture::new().await;
    let result = check_continuity(&mut configured).await;
    configured.close().await;
    result.unwrap();
}

async fn check_continuity(configured: &mut ConfiguredFixture) -> anyhow::Result<()> {
    configured.configure().await?;
    let marker = format!("team-continuity-{}", uuid::Uuid::new_v4());
    let first = configured
        .admit(
            "remember",
            &format!(
                "CONFIGURED-TEAM-REMEMBER: Remember this conversation code: {marker}. \
         Reply with exactly READY and do not call any tools."
            ),
        )
        .await?;
    let first = configured.run(first).await?;
    ensure!(configured.assistant_text(&first, false).await?.trim() == "READY");
    ensure!(
        first.state == LoopActivationState::Interrupted && first.outcome.is_none(),
        "ordinary provider completion must not become a canonical finish"
    );
    configured.restart().await?;
    let second = configured.admit("recall",
        "CONFIGURED-TEAM-RECALL: What conversation code did I give in the preceding instruction? \
         Reply with only that code and do not call any tools.",
    ).await?;
    let second = configured.run(second).await?;
    ensure!(second.state == LoopActivationState::Interrupted && second.outcome.is_none());
    ensure!(
        configured.assistant_text(&second, false).await?.trim() == marker,
        "provider did not recall the earlier conversation"
    );
    configured.assert_resume(&first, &second).await?;
    configured.assert_task_unchanged().await?;
    Ok(())
}
