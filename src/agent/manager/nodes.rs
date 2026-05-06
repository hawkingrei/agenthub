use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use crate::agent::AgentNodeRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AgentNodeSchemaCaps {
    pub(super) has_agent_nodes_table: bool,
    pub(super) has_default_worktree_root_column: bool,
    pub(super) has_last_seen_at_column: bool,
    pub(super) has_group_id_column: bool,
}

impl AgentNodeSchemaCaps {
    pub(super) async fn load(db: &SqlitePool) -> anyhow::Result<Self> {
        let has_agent_nodes_table: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sqlite_master
            WHERE type = 'table' AND name = 'agent_nodes'
            "#,
        )
        .fetch_one(db)
        .await?;
        if has_agent_nodes_table == 0 {
            return Ok(Self {
                has_agent_nodes_table: false,
                has_default_worktree_root_column: false,
                has_last_seen_at_column: false,
                has_group_id_column: false,
            });
        }

        let rows = sqlx::query(
            r#"
            SELECT name
            FROM pragma_table_info('agent_nodes')
            "#,
        )
        .fetch_all(db)
        .await?;
        let has_default_worktree_root_column = rows
            .iter()
            .any(|row| row.get::<String, _>("name") == "default_worktree_root");
        let has_last_seen_at_column = rows
            .iter()
            .any(|row| row.get::<String, _>("name") == "last_seen_at");
        let has_group_id_column = rows
            .iter()
            .any(|row| row.get::<String, _>("name") == "group_id");
        Ok(Self {
            has_agent_nodes_table: true,
            has_default_worktree_root_column,
            has_last_seen_at_column,
            has_group_id_column,
        })
    }
}

pub(super) struct AgentNodeInsertRecord<'a> {
    pub(super) id: &'a str,
    pub(super) name: &'a str,
    pub(super) grpc_target: &'a str,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) default_worktree_root: Option<&'a str>,
    pub(super) group_id: Option<&'a str>,
    pub(super) now: i64,
}

pub(super) struct AgentNodeUpdateRecord<'a> {
    pub(super) node_id: &'a str,
    pub(super) name: &'a str,
    pub(super) grpc_target: &'a str,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) default_worktree_root: Option<&'a str>,
    pub(super) group_id: Option<&'a str>,
    pub(super) now: i64,
}

pub(super) fn decode_agent_node_record(
    row: &SqliteRow,
    caps: AgentNodeSchemaCaps,
) -> AgentNodeRecord {
    AgentNodeRecord {
        id: row.get("id"),
        name: row.get("name"),
        grpc_target: row
            .try_get::<Option<String>, _>("grpc_target")
            .ok()
            .flatten(),
        tls_server_name: row
            .try_get::<Option<String>, _>("tls_server_name")
            .ok()
            .flatten(),
        default_worktree_root: if caps.has_default_worktree_root_column {
            row.try_get::<Option<String>, _>("default_worktree_root")
                .ok()
                .flatten()
        } else {
            None
        },
        group_id: if caps.has_group_id_column {
            row.try_get::<Option<String>, _>("group_id").ok().flatten()
        } else {
            None
        },
        last_seen_at: if caps.has_last_seen_at_column {
            row.try_get::<Option<i64>, _>("last_seen_at").ok().flatten()
        } else {
            None
        },
        is_main: false,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub(super) async fn insert_agent_node_record(
    db: &SqlitePool,
    caps: AgentNodeSchemaCaps,
    record: AgentNodeInsertRecord<'_>,
) -> anyhow::Result<AgentNodeRecord> {
    if record.group_id.is_some() && !caps.has_group_id_column {
        anyhow::bail!(
            "agent_nodes.group_id column is required to persist node group assignments on a legacy schema"
        );
    }

    if caps.has_default_worktree_root_column && caps.has_group_id_column {
        sqlx::query(
            r#"
            INSERT INTO agent_nodes (
                id,
                name,
                grpc_target,
                tls_server_name,
                default_worktree_root,
                group_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(record.id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.default_worktree_root)
        .bind(record.group_id)
        .bind(record.now)
        .bind(record.now)
        .execute(db)
        .await?;
    } else if caps.has_default_worktree_root_column {
        sqlx::query(
            r#"
            INSERT INTO agent_nodes (
                id,
                name,
                grpc_target,
                tls_server_name,
                default_worktree_root,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(record.id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.default_worktree_root)
        .bind(record.now)
        .bind(record.now)
        .execute(db)
        .await?;
    } else if caps.has_group_id_column {
        if record.default_worktree_root.is_some() {
            anyhow::bail!(
                "agent_nodes.default_worktree_root column is required to persist node worktree defaults on a legacy schema"
            );
        }
        sqlx::query(
            r#"
            INSERT INTO agent_nodes (
                id,
                name,
                grpc_target,
                tls_server_name,
                group_id,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(record.id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.group_id)
        .bind(record.now)
        .bind(record.now)
        .execute(db)
        .await?;
    } else {
        if record.default_worktree_root.is_some() {
            anyhow::bail!(
                "agent_nodes.default_worktree_root column is required to persist node worktree defaults on a legacy schema"
            );
        }
        sqlx::query(
            r#"
            INSERT INTO agent_nodes (
                id,
                name,
                grpc_target,
                tls_server_name,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(record.id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.now)
        .bind(record.now)
        .execute(db)
        .await?;
    }

    Ok(AgentNodeRecord {
        id: record.id.to_string(),
        name: record.name.to_string(),
        grpc_target: Some(record.grpc_target.to_string()),
        tls_server_name: record.tls_server_name.map(str::to_string),
        default_worktree_root: record.default_worktree_root.map(str::to_string),
        group_id: record.group_id.map(str::to_string),
        last_seen_at: None,
        is_main: false,
        created_at: record.now,
        updated_at: record.now,
    })
}

pub(super) async fn update_agent_node_record(
    db: &SqlitePool,
    caps: AgentNodeSchemaCaps,
    record: AgentNodeUpdateRecord<'_>,
) -> anyhow::Result<AgentNodeRecord> {
    if record.group_id.is_some() && !caps.has_group_id_column {
        anyhow::bail!(
            "agent_nodes.group_id column is required to persist node group assignments on a legacy schema"
        );
    }

    let result = if caps.has_default_worktree_root_column && caps.has_group_id_column {
        sqlx::query(
            r#"
            UPDATE agent_nodes
            SET name = ?2,
                grpc_target = ?3,
                tls_server_name = ?4,
                default_worktree_root = ?5,
                group_id = ?6,
                updated_at = ?7
            WHERE id = ?1
            "#,
        )
        .bind(record.node_id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.default_worktree_root)
        .bind(record.group_id)
        .bind(record.now)
        .execute(db)
        .await?
    } else if caps.has_default_worktree_root_column {
        sqlx::query(
            r#"
            UPDATE agent_nodes
            SET name = ?2,
                grpc_target = ?3,
                tls_server_name = ?4,
                default_worktree_root = ?5,
                updated_at = ?6
            WHERE id = ?1
            "#,
        )
        .bind(record.node_id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.default_worktree_root)
        .bind(record.now)
        .execute(db)
        .await?
    } else if caps.has_group_id_column {
        if record.default_worktree_root.is_some() {
            anyhow::bail!(
                "agent_nodes.default_worktree_root column is required to persist node worktree defaults on a legacy schema"
            );
        }
        sqlx::query(
            r#"
            UPDATE agent_nodes
            SET name = ?2,
                grpc_target = ?3,
                tls_server_name = ?4,
                group_id = ?5,
                updated_at = ?6
            WHERE id = ?1
            "#,
        )
        .bind(record.node_id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.group_id)
        .bind(record.now)
        .execute(db)
        .await?
    } else {
        if record.default_worktree_root.is_some() {
            anyhow::bail!(
                "agent_nodes.default_worktree_root column is required to persist node worktree defaults on a legacy schema"
            );
        }
        sqlx::query(
            r#"
            UPDATE agent_nodes
            SET name = ?2,
                grpc_target = ?3,
                tls_server_name = ?4,
                updated_at = ?5
            WHERE id = ?1
            "#,
        )
        .bind(record.node_id)
        .bind(record.name)
        .bind(record.grpc_target)
        .bind(record.tls_server_name)
        .bind(record.now)
        .execute(db)
        .await?
    };

    if result.rows_affected() == 0 {
        anyhow::bail!("agent node '{}' not found", record.node_id);
    }

    let row = get_agent_node_row(db, caps, record.node_id).await?;
    Ok(decode_agent_node_record(&row, caps))
}

pub(super) async fn list_agent_node_rows(
    db: &SqlitePool,
    caps: AgentNodeSchemaCaps,
) -> anyhow::Result<Vec<SqliteRow>> {
    if !caps.has_agent_nodes_table {
        return Ok(Vec::new());
    }
    if caps.has_default_worktree_root_column {
        let select_last_seen = if caps.has_last_seen_at_column {
            ", last_seen_at"
        } else {
            ""
        };
        let select_group_id = if caps.has_group_id_column {
            ", group_id"
        } else {
            ""
        };
        let sql = format!(
            "
            SELECT id, name, grpc_target, tls_server_name, default_worktree_root{select_group_id}{select_last_seen}, created_at, updated_at
            FROM agent_nodes
            ORDER BY created_at DESC
            "
        );
        return sqlx::query(&sql).fetch_all(db).await.map_err(Into::into);
    }

    let select_last_seen = if caps.has_last_seen_at_column {
        ", last_seen_at"
    } else {
        ""
    };
    let select_group_id = if caps.has_group_id_column {
        ", group_id"
    } else {
        ""
    };
    let sql = format!(
        "
        SELECT id, name, grpc_target, tls_server_name{select_group_id}{select_last_seen}, created_at, updated_at
        FROM agent_nodes
        ORDER BY created_at DESC
        "
    );
    sqlx::query(&sql).fetch_all(db).await.map_err(Into::into)
}

pub(super) async fn get_agent_node_row(
    db: &SqlitePool,
    caps: AgentNodeSchemaCaps,
    node_id: &str,
) -> anyhow::Result<SqliteRow> {
    if caps.has_default_worktree_root_column {
        let select_last_seen = if caps.has_last_seen_at_column {
            ", last_seen_at"
        } else {
            ""
        };
        let select_group_id = if caps.has_group_id_column {
            ", group_id"
        } else {
            ""
        };
        let sql = format!(
            "
            SELECT id, name, grpc_target, tls_server_name, default_worktree_root{select_group_id}{select_last_seen}, created_at, updated_at
            FROM agent_nodes
            WHERE id = ?1
            "
        );
        return sqlx::query(&sql)
            .bind(node_id)
            .fetch_one(db)
            .await
            .map_err(Into::into);
    }

    let select_last_seen = if caps.has_last_seen_at_column {
        ", last_seen_at"
    } else {
        ""
    };
    let select_group_id = if caps.has_group_id_column {
        ", group_id"
    } else {
        ""
    };
    let sql = format!(
        "
        SELECT id, name, grpc_target, tls_server_name{select_group_id}{select_last_seen}, created_at, updated_at
        FROM agent_nodes
        WHERE id = ?1
        "
    );
    sqlx::query(&sql)
        .bind(node_id)
        .fetch_one(db)
        .await
        .map_err(Into::into)
}

pub(super) async fn touch_agent_node_last_seen(
    db: &SqlitePool,
    caps: AgentNodeSchemaCaps,
    node_id: &str,
    now: i64,
) -> anyhow::Result<bool> {
    if !caps.has_agent_nodes_table || !caps.has_last_seen_at_column {
        return Ok(false);
    }
    let result = sqlx::query(
        r#"
        UPDATE agent_nodes
        SET last_seen_at = ?2
        WHERE id = ?1
        "#,
    )
    .bind(node_id)
    .bind(now)
    .execute(db)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(super) async fn delete_agent_node_record(
    db: &SqlitePool,
    node_id: &str,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM agent_nodes
        WHERE id = ?1
        "#,
    )
    .bind(node_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}
