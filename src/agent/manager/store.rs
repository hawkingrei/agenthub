use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, sqlite::SqliteRow};

use super::codec::{status_from_str, worktree_mode_from_opt};
use super::codec::{status_to_str, worktree_mode_to_str};
use super::{AGENT_SOURCE_MANUAL, AGENT_SOURCE_TEAM_FORGE};
use crate::agent::{AgentConfig, AgentRecord, AgentStatus, normalize_target_node_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AgentSchemaCaps {
    pub(super) has_source_column: bool,
    pub(super) has_target_node_id_column: bool,
}

impl AgentSchemaCaps {
    fn from_column_names<'a>(columns: impl IntoIterator<Item = &'a str>) -> Self {
        let mut caps = Self {
            has_source_column: false,
            has_target_node_id_column: false,
        };
        for column in columns {
            match column {
                "source" => caps.has_source_column = true,
                "target_node_id" => caps.has_target_node_id_column = true,
                _ => {}
            }
        }
        caps
    }

    pub(super) async fn load(db: &SqlitePool) -> anyhow::Result<Self> {
        let rows = sqlx::query(
            r#"
            SELECT name
            FROM pragma_table_info('agents')
            "#,
        )
        .fetch_all(db)
        .await?;
        let names = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        Ok(Self::from_column_names(names.iter().map(String::as_str)))
    }
}

pub(super) fn decode_target_node_id(row: &SqliteRow) -> Option<String> {
    normalize_target_node_id(
        row.try_get::<Option<String>, _>("target_node_id")
            .ok()
            .flatten()
            .as_deref(),
    )
}

pub(super) async fn list_agent_rows(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    excluded_source: Option<&str>,
) -> anyhow::Result<Vec<SqliteRow>> {
    match (caps.has_source_column, caps.has_target_node_id_column, excluded_source) {
        (true, true, Some(source)) => {
            sqlx::query(
                r#"
                SELECT id, name, workdir, command, args, target_node_id, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
                FROM agents
                WHERE COALESCE(source, 'manual') != ?1
                ORDER BY created_at DESC
                "#,
            )
            .bind(source)
            .fetch_all(db)
            .await
            .map_err(Into::into)
        }
        (true, false, Some(source)) => {
            sqlx::query(
                r#"
                SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
                FROM agents
                WHERE COALESCE(source, 'manual') != ?1
                ORDER BY created_at DESC
                "#,
            )
            .bind(source)
            .fetch_all(db)
            .await
            .map_err(Into::into)
        }
        (_, true, _) => {
            sqlx::query(
                r#"
                SELECT id, name, workdir, command, args, target_node_id, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
                FROM agents
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(db)
            .await
            .map_err(Into::into)
        }
        _ => {
            sqlx::query(
                r#"
                SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
                FROM agents
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(db)
            .await
            .map_err(Into::into)
        }
    }
}

pub(super) async fn get_agent_row(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    agent_id: &str,
) -> anyhow::Result<SqliteRow> {
    if caps.has_target_node_id_column {
        return sqlx::query(
            r#"
            SELECT id, name, workdir, command, args, target_node_id, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
            FROM agents
            WHERE id = ?1
            "#,
        )
        .bind(agent_id)
        .fetch_one(db)
        .await
        .map_err(Into::into);
    }

    sqlx::query(
        r#"
        SELECT id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref, code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt, status, created_at, updated_at
        FROM agents
        WHERE id = ?1
        "#,
    )
    .bind(agent_id)
    .fetch_one(db)
    .await
    .map_err(Into::into)
}

pub(super) fn decode_agent_record(row: &SqliteRow) -> anyhow::Result<AgentRecord> {
    let args = serde_json::from_str::<Vec<String>>(row.get("args"))?;
    let worktree_mode = worktree_mode_from_opt(row.try_get("worktree_mode").ok());
    let code_mode: i64 = row.try_get("code_mode").unwrap_or(0);
    let agent_loop_enabled: i64 = row.try_get("agent_loop_enabled").unwrap_or(0);
    Ok(AgentRecord {
        id: row.get("id"),
        name: row.get("name"),
        workdir: row.get("workdir"),
        command: row.get("command"),
        args,
        target_node_id: decode_target_node_id(row),
        worktree_mode,
        worktree_repo: row.try_get("worktree_repo").ok(),
        worktree_ref: row.try_get("worktree_ref").ok(),
        code_mode: code_mode != 0,
        agent_loop_enabled: agent_loop_enabled != 0,
        agent_loop_idle_seconds: row.try_get("agent_loop_idle_seconds").ok(),
        agent_loop_prompt: row.try_get("agent_loop_prompt").ok(),
        status: status_from_str(row.get::<String, _>("status").as_str()),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) struct AgentInsertRecord<'a> {
    pub(super) id: &'a str,
    pub(super) config: &'a AgentConfig,
    pub(super) workdir: &'a str,
    pub(super) args_json: &'a str,
    pub(super) target_node_id: Option<&'a str>,
    pub(super) worktree_repo: Option<&'a str>,
    pub(super) source: &'a str,
    pub(super) status: &'a AgentStatus,
    pub(super) now: i64,
}

pub(super) async fn insert_agent_record(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    record: AgentInsertRecord<'_>,
) -> anyhow::Result<()> {
    debug_assert!(record.source == AGENT_SOURCE_MANUAL || record.source == AGENT_SOURCE_TEAM_FORGE);

    // Keep legacy-schema compatibility local to this module so manager flows
    // can stay focused on orchestration instead of column-presence branching.
    let mut builder = QueryBuilder::<Sqlite>::new("INSERT INTO agents (");
    let mut first = true;
    push_insert_column(&mut builder, &mut first, "id");
    push_insert_column(&mut builder, &mut first, "name");
    push_insert_column(&mut builder, &mut first, "workdir");
    push_insert_column(&mut builder, &mut first, "command");
    push_insert_column(&mut builder, &mut first, "args");
    if caps.has_target_node_id_column {
        push_insert_column(&mut builder, &mut first, "target_node_id");
    }
    push_insert_column(&mut builder, &mut first, "worktree_mode");
    push_insert_column(&mut builder, &mut first, "worktree_repo");
    push_insert_column(&mut builder, &mut first, "worktree_ref");
    push_insert_column(&mut builder, &mut first, "code_mode");
    push_insert_column(&mut builder, &mut first, "agent_loop_enabled");
    push_insert_column(&mut builder, &mut first, "agent_loop_idle_seconds");
    push_insert_column(&mut builder, &mut first, "agent_loop_prompt");
    if caps.has_source_column {
        push_insert_column(&mut builder, &mut first, "source");
    }
    push_insert_column(&mut builder, &mut first, "status");
    push_insert_column(&mut builder, &mut first, "created_at");
    push_insert_column(&mut builder, &mut first, "updated_at");
    builder.push(") VALUES (");

    let mut first = true;
    push_bound_value(&mut builder, &mut first, record.id);
    push_bound_value(&mut builder, &mut first, &record.config.name);
    push_bound_value(&mut builder, &mut first, record.workdir);
    push_bound_value(&mut builder, &mut first, &record.config.command);
    push_bound_value(&mut builder, &mut first, record.args_json);
    if caps.has_target_node_id_column {
        push_bound_value(&mut builder, &mut first, record.target_node_id);
    }
    push_bound_value(
        &mut builder,
        &mut first,
        worktree_mode_to_str(&record.config.worktree_mode),
    );
    push_bound_value(&mut builder, &mut first, record.worktree_repo);
    push_bound_value(
        &mut builder,
        &mut first,
        record.config.worktree_ref.as_deref(),
    );
    push_bound_value(
        &mut builder,
        &mut first,
        if record.config.code_mode { 1 } else { 0 },
    );
    push_bound_value(
        &mut builder,
        &mut first,
        if record.config.agent_loop_enabled {
            1
        } else {
            0
        },
    );
    push_bound_value(
        &mut builder,
        &mut first,
        record.config.agent_loop_idle_seconds,
    );
    push_bound_value(
        &mut builder,
        &mut first,
        record.config.agent_loop_prompt.as_deref().map(str::trim),
    );
    if caps.has_source_column {
        push_bound_value(&mut builder, &mut first, record.source);
    }
    push_bound_value(&mut builder, &mut first, status_to_str(record.status));
    push_bound_value(&mut builder, &mut first, record.now);
    push_bound_value(&mut builder, &mut first, record.now);
    builder.push(")");

    builder.build().execute(db).await?;
    Ok(())
}

pub(super) struct RemoteManagedAgentUpsert<'a> {
    pub(super) agent_id: &'a str,
    pub(super) config: &'a AgentConfig,
    pub(super) workdir: &'a str,
    pub(super) args_json: &'a str,
    pub(super) worktree_repo: Option<&'a str>,
    pub(super) source: &'a str,
    pub(super) exists: bool,
    pub(super) now: i64,
}

pub(super) async fn upsert_remote_managed_agent_record(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    record: RemoteManagedAgentUpsert<'_>,
) -> anyhow::Result<()> {
    debug_assert!(record.source == AGENT_SOURCE_MANUAL || record.source == AGENT_SOURCE_TEAM_FORGE);

    if record.exists {
        let mut builder = QueryBuilder::<Sqlite>::new("UPDATE agents SET ");
        let mut first = true;
        push_assignment(&mut builder, &mut first, "name", &record.config.name);
        push_assignment(&mut builder, &mut first, "workdir", record.workdir);
        push_assignment(&mut builder, &mut first, "command", &record.config.command);
        push_assignment(&mut builder, &mut first, "args", record.args_json);
        if caps.has_target_node_id_column {
            push_assignment(
                &mut builder,
                &mut first,
                "target_node_id",
                Option::<&str>::None,
            );
        }
        push_assignment(
            &mut builder,
            &mut first,
            "worktree_mode",
            worktree_mode_to_str(&record.config.worktree_mode),
        );
        push_assignment(
            &mut builder,
            &mut first,
            "worktree_repo",
            record.worktree_repo,
        );
        push_assignment(
            &mut builder,
            &mut first,
            "worktree_ref",
            record.config.worktree_ref.as_deref(),
        );
        push_assignment(
            &mut builder,
            &mut first,
            "code_mode",
            if record.config.code_mode { 1 } else { 0 },
        );
        if caps.has_source_column {
            push_assignment(&mut builder, &mut first, "source", record.source);
        }
        push_assignment(&mut builder, &mut first, "updated_at", record.now);
        builder.push(" WHERE id = ");
        builder.push_bind(record.agent_id);
        builder.build().execute(db).await?;
        return Ok(());
    }

    let status = AgentStatus::Created;
    let mut builder = QueryBuilder::<Sqlite>::new("INSERT INTO agents (");
    let mut first = true;
    push_insert_column(&mut builder, &mut first, "id");
    push_insert_column(&mut builder, &mut first, "name");
    push_insert_column(&mut builder, &mut first, "workdir");
    push_insert_column(&mut builder, &mut first, "command");
    push_insert_column(&mut builder, &mut first, "args");
    if caps.has_target_node_id_column {
        push_insert_column(&mut builder, &mut first, "target_node_id");
    }
    push_insert_column(&mut builder, &mut first, "worktree_mode");
    push_insert_column(&mut builder, &mut first, "worktree_repo");
    push_insert_column(&mut builder, &mut first, "worktree_ref");
    push_insert_column(&mut builder, &mut first, "code_mode");
    if caps.has_source_column {
        push_insert_column(&mut builder, &mut first, "source");
    }
    push_insert_column(&mut builder, &mut first, "status");
    push_insert_column(&mut builder, &mut first, "created_at");
    push_insert_column(&mut builder, &mut first, "updated_at");
    builder.push(") VALUES (");

    let mut first = true;
    push_bound_value(&mut builder, &mut first, record.agent_id);
    push_bound_value(&mut builder, &mut first, &record.config.name);
    push_bound_value(&mut builder, &mut first, record.workdir);
    push_bound_value(&mut builder, &mut first, &record.config.command);
    push_bound_value(&mut builder, &mut first, record.args_json);
    if caps.has_target_node_id_column {
        push_bound_value(&mut builder, &mut first, Option::<&str>::None);
    }
    push_bound_value(
        &mut builder,
        &mut first,
        worktree_mode_to_str(&record.config.worktree_mode),
    );
    push_bound_value(&mut builder, &mut first, record.worktree_repo);
    push_bound_value(
        &mut builder,
        &mut first,
        record.config.worktree_ref.as_deref(),
    );
    push_bound_value(
        &mut builder,
        &mut first,
        if record.config.code_mode { 1 } else { 0 },
    );
    if caps.has_source_column {
        push_bound_value(&mut builder, &mut first, record.source);
    }
    push_bound_value(&mut builder, &mut first, status_to_str(&status));
    push_bound_value(&mut builder, &mut first, record.now);
    push_bound_value(&mut builder, &mut first, record.now);
    builder.push(")");

    builder.build().execute(db).await?;
    Ok(())
}

fn push_separator(builder: &mut QueryBuilder<'_, Sqlite>, first: &mut bool) {
    if !*first {
        builder.push(", ");
        return;
    }
    *first = false;
}

fn push_insert_column(builder: &mut QueryBuilder<'_, Sqlite>, first: &mut bool, column: &str) {
    push_separator(builder, first);
    builder.push(column);
}

fn push_bound_value<'args, T>(builder: &mut QueryBuilder<'args, Sqlite>, first: &mut bool, value: T)
where
    T: 'args + Send + sqlx::Encode<'args, Sqlite> + sqlx::Type<Sqlite>,
{
    push_separator(builder, first);
    builder.push_bind(value);
}

fn push_assignment<'args, T>(
    builder: &mut QueryBuilder<'args, Sqlite>,
    first: &mut bool,
    column: &str,
    value: T,
) where
    T: 'args + Send + sqlx::Encode<'args, Sqlite> + sqlx::Type<Sqlite>,
{
    push_separator(builder, first);
    builder.push(column);
    builder.push(" = ");
    builder.push_bind(value);
}

#[cfg(test)]
mod tests {
    use super::AgentSchemaCaps;

    #[test]
    fn agent_schema_caps_detects_supported_columns() {
        let caps = AgentSchemaCaps::from_column_names(["id", "name", "source", "target_node_id"]);
        assert!(caps.has_source_column);
        assert!(caps.has_target_node_id_column);
    }

    #[test]
    fn agent_schema_caps_defaults_missing_columns_to_false() {
        let caps = AgentSchemaCaps::from_column_names(["id", "name", "status"]);
        assert!(!caps.has_source_column);
        assert!(!caps.has_target_node_id_column);
    }
}
