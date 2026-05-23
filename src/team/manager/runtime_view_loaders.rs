use std::collections::HashMap;

use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::runtime_views::TeamMemberSpecView;

#[derive(Debug, Clone)]
pub(super) struct AgentRuntimeRow {
    pub(super) name: String,
    pub(super) status: Option<String>,
    pub(super) code_mode: bool,
    pub(super) worktree_mode: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct AgentRunningSessionRow {
    pub(super) session_id: String,
    pub(super) session_status: String,
}

pub(super) async fn load_agent_runtime_rows(
    db: &SqlitePool,
    members: &[TeamMemberSpecView],
) -> anyhow::Result<HashMap<String, AgentRuntimeRow>> {
    if members.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, name, status, code_mode, worktree_mode FROM agents WHERE id IN (",
    );
    let mut separated = builder.separated(", ");
    for member in members {
        separated.push_bind(member.member_id.as_str());
    }
    separated.push_unseparated(")");
    let rows = builder.build().fetch_all(db).await?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let code_mode_raw: i64 = row.get("code_mode");
        out.insert(
            id,
            AgentRuntimeRow {
                name: row.get("name"),
                status: row.get::<Option<String>, _>("status"),
                code_mode: code_mode_raw != 0,
                worktree_mode: row.get::<Option<String>, _>("worktree_mode"),
            },
        );
    }
    Ok(out)
}

pub(super) async fn load_session_status_rows(
    db: &SqlitePool,
    session_ids: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    if session_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder =
        QueryBuilder::<Sqlite>::new("SELECT id, status FROM agent_sessions WHERE id IN (");
    let mut separated = builder.separated(", ");
    for session_id in session_ids {
        separated.push_bind(session_id.as_str());
    }
    separated.push_unseparated(")");
    let rows = builder.build().fetch_all(db).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id");
        let status: String = row.get("status");
        out.insert(id, status);
    }
    Ok(out)
}

pub(super) async fn load_running_session_rows_by_agent(
    db: &SqlitePool,
    members: &[TeamMemberSpecView],
) -> anyhow::Result<HashMap<String, AgentRunningSessionRow>> {
    if members.is_empty() {
        return Ok(HashMap::new());
    }
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT agent_id, id, status
        FROM agent_sessions
        WHERE ended_at IS NULL
          AND agent_id IN (
        "#,
    );
    let mut separated = builder.separated(", ");
    for member in members {
        separated.push_bind(member.member_id.as_str());
    }
    separated.push_unseparated(
        r#")
        ORDER BY started_at DESC, id DESC
        "#,
    );
    let rows = builder.build().fetch_all(db).await?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let member_id: String = row.get("agent_id");
        let session_id = row.get::<String, _>("id").trim().to_string();
        if session_id.is_empty() {
            continue;
        }
        if out.contains_key(member_id.as_str()) {
            continue;
        }
        out.insert(
            member_id,
            AgentRunningSessionRow {
                session_id,
                session_status: row.get("status"),
            },
        );
    }
    Ok(out)
}
