use serde_json::Value;
use sqlx::{Row, Sqlite};

use super::context_artifact_persistence::ContextArtifactOwner;
use super::{
    build_team_member_actor_context_for_role, continuity_note_relative_path,
    extract_context_artifact_path, team_member_role_from_spec,
};
use crate::agent::{WorktreeMode, derive_team_runtime_workdir};
use crate::team::TeamMemberContinuityStateRecord;
use agenthub_team_domain::{
    TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY, TEAM_CONTINUITY_NOTE_SCHEMA_VERSION,
    TEAM_RUNTIME_STATE_SCHEMA_FAMILY, TEAM_RUNTIME_STATE_SCHEMA_VERSION,
};

#[derive(Debug, Clone)]
pub(super) struct TeamMemberContextWorkspace {
    pub(super) runtime_workdir: String,
}

pub(super) async fn load_team_member_context_workspace_tx(
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

pub(super) fn build_runtime_state_snapshot_text(
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

pub(super) fn build_runtime_continuity_note_text(
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
