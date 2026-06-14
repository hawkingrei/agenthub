use sqlx::{AssertSqlSafe, QueryBuilder, Row, Sqlite, SqlitePool, sqlite::SqliteRow};

use agenthub_config::{
    normalize_optional_codex_acp_mode_id, normalize_optional_runtime_model,
    normalize_optional_thinking_level,
};

use super::codec::{status_from_str, worktree_mode_from_opt};
use super::codec::{status_to_str, worktree_mode_to_str};
use super::{AGENT_SOURCE_MANUAL, AGENT_SOURCE_TEAM_FORGE};
use crate::agent::{AgentConfig, AgentRecord, AgentStatus, normalize_target_node_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AgentSchemaCaps {
    pub(super) has_source_column: bool,
    pub(super) has_target_node_id_column: bool,
    pub(super) has_codex_acp_default_mode_column: bool,
    pub(super) has_runtime_model_column: bool,
    pub(super) has_thinking_level_column: bool,
}

impl AgentSchemaCaps {
    fn from_column_names<'a>(columns: impl IntoIterator<Item = &'a str>) -> Self {
        let mut caps = Self {
            has_source_column: false,
            has_target_node_id_column: false,
            has_codex_acp_default_mode_column: false,
            has_runtime_model_column: false,
            has_thinking_level_column: false,
        };
        for column in columns {
            match column {
                "source" => caps.has_source_column = true,
                "target_node_id" => caps.has_target_node_id_column = true,
                "codex_acp_default_mode" => caps.has_codex_acp_default_mode_column = true,
                "runtime_model" => caps.has_runtime_model_column = true,
                "thinking_level" => caps.has_thinking_level_column = true,
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

fn agent_select_columns(caps: AgentSchemaCaps) -> String {
    let mut columns = vec![
        "id",
        "name",
        "workdir",
        "command",
        "args",
        "worktree_mode",
        "worktree_repo",
        "worktree_ref",
        "code_mode",
    ];
    if caps.has_target_node_id_column {
        columns.insert(5, "target_node_id");
    }
    if caps.has_codex_acp_default_mode_column {
        columns.push("codex_acp_default_mode");
    }
    if caps.has_runtime_model_column {
        columns.push("runtime_model");
    }
    if caps.has_thinking_level_column {
        columns.push("thinking_level");
    }
    columns.extend([
        "agent_loop_enabled",
        "agent_loop_idle_seconds",
        "agent_loop_prompt",
        "status",
        "created_at",
        "updated_at",
    ]);
    columns.join(", ")
}

pub(super) async fn list_agent_rows(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    excluded_source: Option<&str>,
) -> anyhow::Result<Vec<SqliteRow>> {
    let mut query = format!("SELECT {} FROM agents", agent_select_columns(caps));
    if caps.has_source_column && excluded_source.is_some() {
        query.push_str(" WHERE COALESCE(source, 'manual') != ?1");
    }
    query.push_str(" ORDER BY created_at DESC");
    let mut query = sqlx::query(AssertSqlSafe(query));
    if let Some(source) = excluded_source.filter(|_| caps.has_source_column) {
        query = query.bind(source);
    }
    query.fetch_all(db).await.map_err(Into::into)
}

pub(super) async fn get_agent_row(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    agent_id: &str,
) -> anyhow::Result<SqliteRow> {
    let query = format!(
        "SELECT {} FROM agents WHERE id = ?1",
        agent_select_columns(caps)
    );
    sqlx::query(AssertSqlSafe(query))
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
        codex_acp_default_mode: normalize_optional_codex_acp_mode_id(
            row.try_get::<Option<String>, _>("codex_acp_default_mode")
                .ok()
                .flatten()
                .as_deref(),
        ),
        runtime_model: normalize_optional_runtime_model(
            row.try_get::<Option<String>, _>("runtime_model")
                .ok()
                .flatten()
                .as_deref(),
        ),
        thinking_level: normalize_optional_thinking_level(
            row.try_get::<Option<String>, _>("thinking_level")
                .ok()
                .flatten()
                .as_deref(),
        ),
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
    if caps.has_codex_acp_default_mode_column {
        push_insert_column(&mut builder, &mut first, "codex_acp_default_mode");
    }
    if caps.has_runtime_model_column {
        push_insert_column(&mut builder, &mut first, "runtime_model");
    }
    if caps.has_thinking_level_column {
        push_insert_column(&mut builder, &mut first, "thinking_level");
    }
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
    if caps.has_codex_acp_default_mode_column {
        push_bound_value(
            &mut builder,
            &mut first,
            normalize_optional_codex_acp_mode_id(record.config.codex_acp_default_mode.as_deref()),
        );
    }
    if caps.has_runtime_model_column {
        push_bound_value(
            &mut builder,
            &mut first,
            normalize_optional_runtime_model(record.config.runtime_model.as_deref()),
        );
    }
    if caps.has_thinking_level_column {
        push_bound_value(
            &mut builder,
            &mut first,
            normalize_optional_thinking_level(record.config.thinking_level.as_deref()),
        );
    }
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

struct RemoteManagedAgentPersisted<'a> {
    agent_id: &'a str,
    name: &'a str,
    workdir: &'a str,
    command: &'a str,
    args_json: &'a str,
    worktree_mode: &'static str,
    worktree_repo: Option<&'a str>,
    worktree_ref: Option<&'a str>,
    code_mode: i64,
    codex_acp_default_mode: Option<String>,
    runtime_model: Option<String>,
    thinking_level: Option<String>,
    source: &'a str,
    now: i64,
}

impl<'a> RemoteManagedAgentPersisted<'a> {
    fn from_upsert(record: &'a RemoteManagedAgentUpsert<'_>) -> Self {
        Self {
            agent_id: record.agent_id,
            name: &record.config.name,
            workdir: record.workdir,
            command: &record.config.command,
            args_json: record.args_json,
            worktree_mode: worktree_mode_to_str(&record.config.worktree_mode),
            worktree_repo: record.worktree_repo,
            worktree_ref: record.config.worktree_ref.as_deref(),
            code_mode: if record.config.code_mode { 1 } else { 0 },
            codex_acp_default_mode: normalize_optional_codex_acp_mode_id(
                record.config.codex_acp_default_mode.as_deref(),
            ),
            runtime_model: normalize_optional_runtime_model(record.config.runtime_model.as_deref()),
            thinking_level: normalize_optional_thinking_level(
                record.config.thinking_level.as_deref(),
            ),
            source: record.source,
            now: record.now,
        }
    }

    fn push_insert_columns(&self, builder: &mut QueryBuilder<Sqlite>, caps: AgentSchemaCaps) {
        let mut first = true;
        push_insert_column(builder, &mut first, "id");
        push_insert_column(builder, &mut first, "name");
        push_insert_column(builder, &mut first, "workdir");
        push_insert_column(builder, &mut first, "command");
        push_insert_column(builder, &mut first, "args");
        if caps.has_target_node_id_column {
            push_insert_column(builder, &mut first, "target_node_id");
        }
        push_insert_column(builder, &mut first, "worktree_mode");
        push_insert_column(builder, &mut first, "worktree_repo");
        push_insert_column(builder, &mut first, "worktree_ref");
        push_insert_column(builder, &mut first, "code_mode");
        if caps.has_codex_acp_default_mode_column {
            push_insert_column(builder, &mut first, "codex_acp_default_mode");
        }
        if caps.has_runtime_model_column {
            push_insert_column(builder, &mut first, "runtime_model");
        }
        if caps.has_thinking_level_column {
            push_insert_column(builder, &mut first, "thinking_level");
        }
        if caps.has_source_column {
            push_insert_column(builder, &mut first, "source");
        }
        push_insert_column(builder, &mut first, "status");
        push_insert_column(builder, &mut first, "created_at");
        push_insert_column(builder, &mut first, "updated_at");
    }

    fn push_insert_values(&'a self, builder: &mut QueryBuilder<Sqlite>, caps: AgentSchemaCaps) {
        let mut first = true;
        push_bound_value(builder, &mut first, self.agent_id);
        push_bound_value(builder, &mut first, self.name);
        push_bound_value(builder, &mut first, self.workdir);
        push_bound_value(builder, &mut first, self.command);
        push_bound_value(builder, &mut first, self.args_json);
        if caps.has_target_node_id_column {
            push_bound_value(builder, &mut first, Option::<&str>::None);
        }
        push_bound_value(builder, &mut first, self.worktree_mode);
        push_bound_value(builder, &mut first, self.worktree_repo);
        push_bound_value(builder, &mut first, self.worktree_ref);
        push_bound_value(builder, &mut first, self.code_mode);
        if caps.has_codex_acp_default_mode_column {
            push_bound_value(builder, &mut first, self.codex_acp_default_mode.as_deref());
        }
        if caps.has_runtime_model_column {
            push_bound_value(builder, &mut first, self.runtime_model.as_deref());
        }
        if caps.has_thinking_level_column {
            push_bound_value(builder, &mut first, self.thinking_level.as_deref());
        }
        if caps.has_source_column {
            push_bound_value(builder, &mut first, self.source);
        }
        push_bound_value(builder, &mut first, status_to_str(&AgentStatus::Created));
        push_bound_value(builder, &mut first, self.now);
        push_bound_value(builder, &mut first, self.now);
    }

    fn push_update_assignments(
        &'a self,
        builder: &mut QueryBuilder<Sqlite>,
        caps: AgentSchemaCaps,
    ) {
        let mut first = true;
        push_assignment(builder, &mut first, "name", self.name);
        push_assignment(builder, &mut first, "workdir", self.workdir);
        push_assignment(builder, &mut first, "command", self.command);
        push_assignment(builder, &mut first, "args", self.args_json);
        if caps.has_target_node_id_column {
            push_assignment(builder, &mut first, "target_node_id", Option::<&str>::None);
        }
        push_assignment(builder, &mut first, "worktree_mode", self.worktree_mode);
        push_assignment(builder, &mut first, "worktree_repo", self.worktree_repo);
        push_assignment(builder, &mut first, "worktree_ref", self.worktree_ref);
        push_assignment(builder, &mut first, "code_mode", self.code_mode);
        if caps.has_codex_acp_default_mode_column {
            push_assignment(
                builder,
                &mut first,
                "codex_acp_default_mode",
                self.codex_acp_default_mode.as_deref(),
            );
        }
        if caps.has_runtime_model_column {
            push_assignment(
                builder,
                &mut first,
                "runtime_model",
                self.runtime_model.as_deref(),
            );
        }
        if caps.has_thinking_level_column {
            push_assignment(
                builder,
                &mut first,
                "thinking_level",
                self.thinking_level.as_deref(),
            );
        }
        if caps.has_source_column {
            push_assignment(builder, &mut first, "source", self.source);
        }
        push_assignment(builder, &mut first, "updated_at", self.now);
    }
}

pub(super) async fn upsert_remote_managed_agent_record(
    db: &SqlitePool,
    caps: AgentSchemaCaps,
    record: RemoteManagedAgentUpsert<'_>,
) -> anyhow::Result<()> {
    debug_assert!(record.source == AGENT_SOURCE_MANUAL || record.source == AGENT_SOURCE_TEAM_FORGE);
    let persisted = RemoteManagedAgentPersisted::from_upsert(&record);

    if record.exists {
        let mut builder = QueryBuilder::<Sqlite>::new("UPDATE agents SET ");
        persisted.push_update_assignments(&mut builder, caps);
        builder.push(" WHERE id = ");
        builder.push_bind(persisted.agent_id);
        builder.build().execute(db).await?;
        return Ok(());
    }

    let mut builder = QueryBuilder::<Sqlite>::new("INSERT INTO agents (");
    persisted.push_insert_columns(&mut builder, caps);
    builder.push(") VALUES (");
    persisted.push_insert_values(&mut builder, caps);
    builder.push(")");

    builder.build().execute(db).await?;
    Ok(())
}

fn push_separator(builder: &mut QueryBuilder<Sqlite>, first: &mut bool) {
    if !*first {
        builder.push(", ");
        return;
    }
    *first = false;
}

fn push_insert_column(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, column: &str) {
    push_separator(builder, first);
    builder.push(column);
}

fn push_bound_value<'args, T>(builder: &mut QueryBuilder<Sqlite>, first: &mut bool, value: T)
where
    T: 'args + Send + sqlx::Encode<'args, Sqlite> + sqlx::Type<Sqlite>,
{
    push_separator(builder, first);
    builder.push_bind(value);
}

fn push_assignment<'args, T>(
    builder: &mut QueryBuilder<Sqlite>,
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
    use sqlx::{AssertSqlSafe, Row, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};

    use super::{
        AgentConfig, AgentSchemaCaps, RemoteManagedAgentUpsert, upsert_remote_managed_agent_record,
    };
    use crate::agent::WorktreeMode;

    #[test]
    fn agent_schema_caps_detects_supported_columns() {
        let caps = AgentSchemaCaps::from_column_names([
            "id",
            "name",
            "source",
            "target_node_id",
            "codex_acp_default_mode",
        ]);
        assert!(caps.has_source_column);
        assert!(caps.has_target_node_id_column);
        assert!(caps.has_codex_acp_default_mode_column);
    }

    #[test]
    fn agent_schema_caps_defaults_missing_columns_to_false() {
        let caps = AgentSchemaCaps::from_column_names(["id", "name", "status"]);
        assert!(!caps.has_source_column);
        assert!(!caps.has_target_node_id_column);
        assert!(!caps.has_codex_acp_default_mode_column);
    }

    async fn create_test_db() -> sqlx::SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .expect("connect sqlite")
    }

    async fn create_agents_table(db: &sqlx::SqlitePool, caps: AgentSchemaCaps) {
        let mut builder = String::from(
            r#"
            CREATE TABLE agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                workdir TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL,
            "#,
        );
        if caps.has_target_node_id_column {
            builder.push_str("target_node_id TEXT,\n");
        }
        builder.push_str(
            r#"
                worktree_mode TEXT NOT NULL,
                worktree_repo TEXT,
                worktree_ref TEXT,
                code_mode INTEGER NOT NULL DEFAULT 0,
            "#,
        );
        if caps.has_codex_acp_default_mode_column {
            builder.push_str("codex_acp_default_mode TEXT,\n");
        }
        if caps.has_runtime_model_column {
            builder.push_str("runtime_model TEXT,\n");
        }
        if caps.has_thinking_level_column {
            builder.push_str("thinking_level TEXT,\n");
        }
        builder.push_str(
            r#"
                agent_loop_enabled INTEGER NOT NULL DEFAULT 0,
                agent_loop_idle_seconds INTEGER,
                agent_loop_prompt TEXT,
            "#,
        );
        if caps.has_source_column {
            builder.push_str("source TEXT NOT NULL DEFAULT 'manual',\n");
        }
        builder.push_str(
            r#"
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        );
        sqlx::query(AssertSqlSafe(builder))
            .execute(db)
            .await
            .expect("create agents");
    }

    fn build_remote_managed_config() -> AgentConfig {
        AgentConfig {
            name: "remote-agent".to_string(),
            workdir: "/tmp/agent".to_string(),
            command: "codex".to_string(),
            args: vec!["--model".to_string(), "gpt-5.4".to_string()],
            target_node_id: None,
            worktree_mode: WorktreeMode::CreateWorktree,
            worktree_repo: Some("/tmp/repo".to_string()),
            worktree_ref: Some("HEAD".to_string()),
            code_mode: true,
            codex_acp_default_mode: Some("yolo".to_string()),
            runtime_model: Some("gpt-5.4-codex".to_string()),
            thinking_level: Some("high".to_string()),
            agent_loop_enabled: true,
            agent_loop_idle_seconds: Some(900),
            agent_loop_prompt: Some("follow up".to_string()),
        }
    }

    #[tokio::test]
    async fn remote_managed_upsert_insert_respects_full_schema() {
        let caps = AgentSchemaCaps {
            has_source_column: true,
            has_target_node_id_column: true,
            has_codex_acp_default_mode_column: true,
            has_runtime_model_column: true,
            has_thinking_level_column: true,
        };
        let db = create_test_db().await;
        create_agents_table(&db, caps).await;
        let config = build_remote_managed_config();
        let args_json = serde_json::to_string(&config.args).expect("serialize args");

        upsert_remote_managed_agent_record(
            &db,
            caps,
            RemoteManagedAgentUpsert {
                agent_id: "agent-1",
                config: &config,
                workdir: "/srv/agent-1",
                args_json: &args_json,
                worktree_repo: Some("/srv/repo"),
                source: "team_forge",
                exists: false,
                now: 123,
            },
        )
        .await
        .expect("insert remote managed agent");

        let row = sqlx::query("SELECT * FROM agents WHERE id = ?1")
            .bind("agent-1")
            .fetch_one(&db)
            .await
            .expect("load inserted agent");
        assert_eq!(row.get::<String, _>("name"), "remote-agent");
        assert_eq!(row.get::<String, _>("workdir"), "/srv/agent-1");
        assert_eq!(row.get::<String, _>("command"), "codex");
        assert_eq!(row.get::<String, _>("args"), args_json);
        assert_eq!(row.get::<Option<String>, _>("target_node_id"), None);
        assert_eq!(row.get::<String, _>("worktree_mode"), "create_worktree");
        assert_eq!(
            row.get::<Option<String>, _>("worktree_repo"),
            Some("/srv/repo".to_string())
        );
        assert_eq!(
            row.get::<Option<String>, _>("worktree_ref"),
            Some("HEAD".to_string())
        );
        assert_eq!(row.get::<i64, _>("code_mode"), 1);
        assert_eq!(
            row.get::<Option<String>, _>("runtime_model"),
            Some("gpt-5.4-codex".to_string())
        );
        assert_eq!(
            row.get::<Option<String>, _>("thinking_level"),
            Some("high".to_string())
        );
        assert_eq!(
            row.get::<Option<String>, _>("codex_acp_default_mode"),
            Some("full-access".to_string())
        );
        assert_eq!(row.get::<String, _>("source"), "team_forge");
        assert_eq!(row.get::<String, _>("status"), "created");
        assert_eq!(row.get::<i64, _>("created_at"), 123);
        assert_eq!(row.get::<i64, _>("updated_at"), 123);
        assert_eq!(row.get::<i64, _>("agent_loop_enabled"), 0);
        assert_eq!(row.get::<Option<i64>, _>("agent_loop_idle_seconds"), None);
        assert_eq!(row.get::<Option<String>, _>("agent_loop_prompt"), None);
    }

    #[tokio::test]
    async fn remote_managed_upsert_update_respects_legacy_schema() {
        let caps = AgentSchemaCaps {
            has_source_column: false,
            has_target_node_id_column: false,
            has_codex_acp_default_mode_column: false,
            has_runtime_model_column: false,
            has_thinking_level_column: false,
        };
        let db = create_test_db().await;
        create_agents_table(&db, caps).await;
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, name, workdir, command, args, worktree_mode, worktree_repo, worktree_ref,
                code_mode, agent_loop_enabled, agent_loop_idle_seconds, agent_loop_prompt,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )
        .bind("agent-1")
        .bind("old-name")
        .bind("/old/workdir")
        .bind("old-command")
        .bind("[\"--old\"]")
        .bind("use_existing")
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(0)
        .bind(1)
        .bind(600)
        .bind("keep-me")
        .bind("running")
        .bind(100)
        .bind(100)
        .execute(&db)
        .await
        .expect("seed legacy row");

        let config = build_remote_managed_config();
        let args_json = serde_json::to_string(&config.args).expect("serialize args");
        upsert_remote_managed_agent_record(
            &db,
            caps,
            RemoteManagedAgentUpsert {
                agent_id: "agent-1",
                config: &config,
                workdir: "/srv/agent-1",
                args_json: &args_json,
                worktree_repo: Some("/srv/repo"),
                source: "manual",
                exists: true,
                now: 456,
            },
        )
        .await
        .expect("update legacy remote managed agent");

        let row = sqlx::query("SELECT * FROM agents WHERE id = ?1")
            .bind("agent-1")
            .fetch_one(&db)
            .await
            .expect("load updated agent");
        assert_eq!(row.get::<String, _>("name"), "remote-agent");
        assert_eq!(row.get::<String, _>("workdir"), "/srv/agent-1");
        assert_eq!(row.get::<String, _>("command"), "codex");
        assert_eq!(row.get::<String, _>("args"), args_json);
        assert_eq!(row.get::<String, _>("worktree_mode"), "create_worktree");
        assert_eq!(
            row.get::<Option<String>, _>("worktree_repo"),
            Some("/srv/repo".to_string())
        );
        assert_eq!(
            row.get::<Option<String>, _>("worktree_ref"),
            Some("HEAD".to_string())
        );
        assert_eq!(row.get::<i64, _>("code_mode"), 1);
        assert_eq!(row.get::<String, _>("status"), "running");
        assert_eq!(row.get::<i64, _>("created_at"), 100);
        assert_eq!(row.get::<i64, _>("updated_at"), 456);
        assert_eq!(row.get::<i64, _>("agent_loop_enabled"), 1);
        assert_eq!(
            row.get::<Option<i64>, _>("agent_loop_idle_seconds"),
            Some(600)
        );
        assert_eq!(
            row.get::<Option<String>, _>("agent_loop_prompt"),
            Some("keep-me".to_string())
        );
    }
}
