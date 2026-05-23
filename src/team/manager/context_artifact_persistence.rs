use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Sqlite;

use super::context_artifact_support::load_team_member_context_workspace_tx;
use super::step_reconcile::ReconcileRoundArtifactSnapshot;
use super::{
    CONTINUITY_ARTIFACT_KIND_OUTPUT, RECONCILE_ROUND_ARTIFACT_KIND, TeamManager, hex_encode,
};
use crate::team::TeamStepRecord;

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
}
