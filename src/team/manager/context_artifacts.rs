use std::path::PathBuf;

use sqlx::Sqlite;

use super::context_artifact_support::{
    build_runtime_continuity_note_text, build_runtime_state_snapshot_text,
    load_team_member_context_workspace_tx,
};
use super::context_artifact_persistence::ContextArtifactOwner;
use super::{TeamManager, continuity_note_relative_path};
use crate::team::TeamMemberContinuityStateRecord;

#[derive(Debug, Clone)]
pub(super) struct RuntimeStateSnapshotWritePlan {
    pub(super) team_id: String,
    pub(super) run_id: String,
    pub(super) member_id: String,
    pub(super) state_path: PathBuf,
    pub(super) state_text: String,
    pub(super) continuity_note: Option<(PathBuf, String)>,
}

impl TeamManager {
    pub(super) async fn prepare_runtime_state_snapshot_write_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        owner: ContextArtifactOwner<'_>,
        continuity_mode: &str,
        continuity_state: &TeamMemberContinuityStateRecord,
    ) -> anyhow::Result<Option<RuntimeStateSnapshotWritePlan>> {
        let Some(workspace) =
            load_team_member_context_workspace_tx(tx, owner.team_id, owner.member_id).await?
        else {
            return Ok(None);
        };

        let workspace_root = PathBuf::from(&workspace.runtime_workdir);
        let state_path = workspace_root
            .join(".cache")
            .join("context")
            .join("state.md");
        let continuity_note = continuity_note_relative_path(&continuity_state.source_run_id).map(
            |relative_note_path| {
                let note_path = workspace_root.join(relative_note_path);
                let note_text =
                    build_runtime_continuity_note_text(owner, continuity_mode, continuity_state);
                (note_path, note_text)
            },
        );
        let state_text =
            build_runtime_state_snapshot_text(owner, continuity_mode, continuity_state);
        Ok(Some(RuntimeStateSnapshotWritePlan {
            team_id: owner.team_id.to_string(),
            run_id: owner.run_id.to_string(),
            member_id: owner.member_id.to_string(),
            state_path,
            state_text,
            continuity_note,
        }))
    }

    pub(super) async fn write_runtime_state_snapshot_best_effort(
        plan: RuntimeStateSnapshotWritePlan,
    ) -> anyhow::Result<()> {
        if let Some((note_path, note_text)) = plan.continuity_note.as_ref() {
            if let Some(parent) = note_path.parent()
                && let Err(err) = tokio::fs::create_dir_all(parent).await
            {
                tracing::warn!(
                    team_id = plan.team_id,
                    run_id = plan.run_id,
                    member_id = plan.member_id,
                    path = %note_path.display(),
                    "team manager failed to create runtime continuity note dir: {}",
                    err
                );
            } else if let Err(err) = tokio::fs::write(note_path, note_text).await {
                tracing::warn!(
                    team_id = plan.team_id,
                    run_id = plan.run_id,
                    member_id = plan.member_id,
                    path = %note_path.display(),
                    "team manager failed to write runtime continuity note: {}",
                    err
                );
            }
        }

        if let Some(parent) = plan.state_path.parent()
            && let Err(err) = tokio::fs::create_dir_all(parent).await
        {
            tracing::warn!(
                team_id = plan.team_id,
                run_id = plan.run_id,
                member_id = plan.member_id,
                path = %plan.state_path.display(),
                "team manager failed to create runtime state dir: {}",
                err
            );
            return Ok(());
        }
        if let Err(err) = tokio::fs::write(&plan.state_path, &plan.state_text).await {
            tracing::warn!(
                team_id = plan.team_id,
                run_id = plan.run_id,
                member_id = plan.member_id,
                path = %plan.state_path.display(),
                "team manager failed to write runtime state snapshot: {}",
                err
            );
        }
        Ok(())
    }
}
