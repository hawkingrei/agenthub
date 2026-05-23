use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite};

use super::step_reconcile::ReconcileRoundArtifactSnapshot;
use super::{
    CONTINUITY_ARTIFACT_KIND_OUTPUT, RECONCILE_ROUND_ARTIFACT_KIND, TeamManager,
    build_team_member_actor_context_for_role, continuity_note_relative_path,
    extract_context_artifact_path, hex_encode, team_member_role_from_spec,
};
use crate::agent::{WorktreeMode, derive_team_runtime_workdir};
use crate::team::{TeamMemberContinuityStateRecord, TeamStepRecord};
use agenthub_team_domain::{
    TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY, TEAM_CONTINUITY_NOTE_SCHEMA_VERSION,
    TEAM_RUNTIME_STATE_SCHEMA_FAMILY, TEAM_RUNTIME_STATE_SCHEMA_VERSION,
};

#[derive(Debug, Clone)]
struct TeamMemberContextWorkspace {
    runtime_workdir: String,
}

#[derive(Debug, Clone)]
pub(super) struct ContinuitySnapshot {
    pub(super) summary_text: String,
    pub(super) history_window: Value,
    pub(super) redacted_output: Value,
    pub(super) redacted_output_text: String,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ContextArtifactOwner<'a> {
    pub(super) team_id: &'a str,
    pub(super) run_id: &'a str,
    pub(super) member_id: &'a str,
    pub(super) session_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeStateSnapshotWritePlan {
    pub(super) team_id: String,
    pub(super) run_id: String,
    pub(super) member_id: String,
    pub(super) state_path: PathBuf,
    pub(super) state_text: String,
    pub(super) continuity_note: Option<(PathBuf, String)>,
}

#[derive(Debug, Clone)]
pub(super) struct ContextArtifactPointer {
    pub(super) artifact_kind: String,
    pub(super) relative_path: String,
    pub(super) artifact_size_bytes: i64,
    pub(super) content_checksum: String,
}

impl TeamManager {
    pub(super) async fn persist_continuity_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        owner: ContextArtifactOwner<'_>,
        snapshot: &ContinuitySnapshot,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let artifact_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": owner.team_id,
            "run_id": owner.run_id,
            "member_id": owner.member_id,
            "session_id": owner.session_id,
            "summary_text": snapshot.summary_text,
            "redacted_output": snapshot.redacted_output,
            "created_at": now,
        });
        self.persist_context_artifact_tx(
            tx,
            owner,
            CONTINUITY_ARTIFACT_KIND_OUTPUT,
            artifact_payload,
            now,
        )
        .await
    }

    pub(super) async fn persist_reconcile_round_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        step: &TeamStepRecord,
        snapshot: ReconcileRoundArtifactSnapshot<'_>,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let team_id: String = sqlx::query_scalar(
            r#"
            SELECT team_id
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(&step.run_id)
        .fetch_one(&mut **tx)
        .await?;
        let team_id_for_payload = team_id.clone();

        let artifact_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": team_id_for_payload,
            "run_id": step.run_id,
            "step_id": step.id,
            "step_key": step.step_key,
            "member_id": step.member_id,
            "session_id": step.runtime_handle_id,
            "round": snapshot.round,
            "status": snapshot.status,
            "summary": snapshot.summary,
            "output": snapshot.output,
            "input": snapshot.input,
            "reason": snapshot.reason,
            "error_text": snapshot.error_text,
            "created_at": now,
        });
        self.persist_context_artifact_tx(
            tx,
            ContextArtifactOwner {
                team_id: &team_id,
                run_id: &step.run_id,
                member_id: &step.member_id,
                session_id: step.runtime_handle_id.as_deref(),
            },
            RECONCILE_ROUND_ARTIFACT_KIND,
            artifact_payload,
            now,
        )
        .await
    }

    pub(super) async fn persist_context_artifact_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        owner: ContextArtifactOwner<'_>,
        artifact_kind: &str,
        artifact_payload: Value,
        now: i64,
    ) -> anyhow::Result<Option<ContextArtifactPointer>> {
        let Some(workspace) =
            load_team_member_context_workspace_tx(tx, owner.team_id, owner.member_id).await?
        else {
            return Ok(None);
        };

        let artifact_seq: i64 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(artifact_seq), 0) + 1
            FROM team_context_artifacts
            WHERE run_id = ?1
            "#,
        )
        .bind(owner.run_id)
        .fetch_one(&mut **tx)
        .await?;

        let run_context_dir = PathBuf::from(&workspace.runtime_workdir)
            .join(".cache")
            .join("context")
            .join("run")
            .join(owner.run_id);
        std::fs::create_dir_all(&run_context_dir)?;

        let file_name = format!("artifact-{artifact_seq}-{artifact_kind}.json");
        let absolute_path = run_context_dir.join(&file_name);
        let relative_path = format!(".cache/context/run/{}/{file_name}", owner.run_id);
        let artifact_bytes = serde_json::to_vec(&artifact_payload)?;
        std::fs::write(&absolute_path, &artifact_bytes)?;
        let artifact_size_bytes = i64::try_from(artifact_bytes.len()).ok().unwrap_or(i64::MAX);
        let content_checksum = hex_encode(&Sha256::digest(&artifact_bytes));
        let absolute_path_string = absolute_path.to_string_lossy().to_string();

        sqlx::query(
            r#"
            INSERT INTO team_context_artifacts (
                team_id,
                run_id,
                member_id,
                session_id,
                artifact_seq,
                artifact_kind,
                artifact_path,
                artifact_size_bytes,
                content_checksum,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(owner.team_id)
        .bind(owner.run_id)
        .bind(owner.member_id)
        .bind(owner.session_id)
        .bind(artifact_seq)
        .bind(artifact_kind)
        .bind(absolute_path_string)
        .bind(artifact_size_bytes)
        .bind(&content_checksum)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        Ok(Some(ContextArtifactPointer {
            artifact_kind: artifact_kind.to_string(),
            relative_path,
            artifact_size_bytes,
            content_checksum,
        }))
    }

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

async fn load_team_member_context_workspace_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
    member_id: &str,
) -> anyhow::Result<Option<TeamMemberContextWorkspace>> {
    let row = sqlx::query(
        r#"
        SELECT a.workdir, a.worktree_mode, td.spec_json
        FROM agents a, team_definitions td
        WHERE a.id = ?2
          AND td.id = ?1
        "#,
    )
    .bind(team_id)
    .bind(member_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let workdir = row.get::<String, _>("workdir").trim().to_string();
    if workdir.is_empty() {
        return Ok(None);
    }
    let worktree_mode = match row.get::<String, _>("worktree_mode").trim() {
        "create_worktree" => WorktreeMode::CreateWorktree,
        "reuse_worktree" => WorktreeMode::ReuseWorktree,
        _ => WorktreeMode::UseExisting,
    };
    let spec_json = row.get::<String, _>("spec_json");

    let runtime_workdir = if let Some(member_role) = serde_json::from_str::<Value>(&spec_json)
        .ok()
        .and_then(|spec| team_member_role_from_spec(&spec, member_id))
    {
        let actor_context =
            build_team_member_actor_context_for_role(team_id, None, member_id, &member_role);
        derive_team_runtime_workdir(&workdir, &actor_context, &worktree_mode)
    } else {
        workdir
    };

    Ok(Some(TeamMemberContextWorkspace { runtime_workdir }))
}

fn build_runtime_state_snapshot_text(
    owner: ContextArtifactOwner<'_>,
    continuity_mode: &str,
    continuity_state: &TeamMemberContinuityStateRecord,
) -> String {
    let mut lines = vec![
        "# Team Runtime State".to_string(),
        String::new(),
        format!("- schema_family: {TEAM_RUNTIME_STATE_SCHEMA_FAMILY}"),
        format!("- schema_version: {TEAM_RUNTIME_STATE_SCHEMA_VERSION}"),
        format!("- updated_at: {}", continuity_state.updated_at),
        format!("- team_id: {}", owner.team_id),
        format!("- member_id: {}", owner.member_id),
        format!("- current_execution_run_id: {}", owner.run_id),
        format!("- continuity_mode: {continuity_mode}"),
        format!(
            "- continuity_source_execution_run_id: {}",
            continuity_state.source_run_id
        ),
    ];
    if let Some(note_path) = continuity_note_relative_path(&continuity_state.source_run_id) {
        lines.push(format!("- continuity_note_path: {note_path}"));
    }
    if let Some(artifact_path) = continuity_state
        .history_window
        .get("artifact_pointer")
        .and_then(extract_context_artifact_path)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- continuity_artifact_path: {artifact_path}"));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn build_runtime_continuity_note_text(
    owner: ContextArtifactOwner<'_>,
    continuity_mode: &str,
    continuity_state: &TeamMemberContinuityStateRecord,
) -> String {
    let history_window = serde_json::to_string_pretty(&continuity_state.history_window)
        .unwrap_or_else(|_| continuity_state.history_window.to_string());
    let mut lines = vec![
        "# Team Continuity Note".to_string(),
        String::new(),
        format!("- schema_family: {TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY}"),
        format!("- schema_version: {TEAM_CONTINUITY_NOTE_SCHEMA_VERSION}"),
        format!("- updated_at: {}", continuity_state.updated_at),
        format!("- team_id: {}", owner.team_id),
        format!("- member_id: {}", owner.member_id),
        format!("- current_execution_run_id: {}", owner.run_id),
        format!(
            "- continuity_source_execution_run_id: {}",
            continuity_state.source_run_id
        ),
        format!("- continuity_mode: {continuity_mode}"),
    ];
    if let Some(artifact_path) = continuity_state
        .history_window
        .get("artifact_pointer")
        .and_then(extract_context_artifact_path)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("- continuity_artifact_path: {artifact_path}"));
    }
    lines.extend([
        String::new(),
        "## Summary".to_string(),
        continuity_state.summary_text.clone(),
        String::new(),
        "## History Window".to_string(),
        "````json".to_string(),
        history_window,
        "````".to_string(),
        String::new(),
    ]);
    lines.join("\n")
}
