use chrono::Utc;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{
    TeamDefinitionConfig, TeamDefinitionRecord, TeamRunEventRecord, TeamRunRecord, TeamRunStatus,
};

#[derive(Clone)]
pub struct TeamManager {
    db: SqlitePool,
}

impl TeamManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn create_team(
        &self,
        config: TeamDefinitionConfig,
    ) -> anyhow::Result<TeamDefinitionRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let spec_json = serde_json::to_string(&config.spec)?;
        sqlx::query(
            r#"
            INSERT INTO team_definitions (id, name, description, spec_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&id)
        .bind(&config.name)
        .bind(&config.description)
        .bind(spec_json)
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await?;

        Ok(TeamDefinitionRecord {
            id,
            name: config.name,
            description: config.description,
            spec: config.spec,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn list_teams(&self) -> anyhow::Result<Vec<TeamDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, created_at, updated_at
            FROM team_definitions
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db)
        .await?;

        let mut teams = Vec::with_capacity(rows.len());
        for row in rows {
            teams.push(parse_team_definition_row(&row)?);
        }
        Ok(teams)
    }

    pub async fn get_team(&self, team_id: &str) -> anyhow::Result<TeamDefinitionRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, name, description, spec_json, created_at, updated_at
            FROM team_definitions
            WHERE id = ?1
            "#,
        )
        .bind(team_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_definition_row(&row)
    }

    pub async fn create_run(
        &self,
        team_id: &str,
        context_id: Option<&str>,
        input: Value,
    ) -> anyhow::Result<TeamRunRecord> {
        let run_id = Uuid::new_v4().to_string();
        let resolved_context_id = if let Some(context_id) = context_id {
            let context_id = context_id.trim();
            if context_id.is_empty() {
                Uuid::new_v4().to_string()
            } else {
                context_id.to_string()
            }
        } else {
            Uuid::new_v4().to_string()
        };
        let now = Utc::now().timestamp();
        let status = TeamRunStatus::Submitted;
        let input_json = serde_json::to_string(&input)?;

        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_runs (id, team_id, context_id, status, input_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&run_id)
        .bind(team_id)
        .bind(&resolved_context_id)
        .bind(team_run_status_to_str(&status))
        .bind(input_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let payload = serde_json::json!({
            "team_id": team_id,
            "context_id": resolved_context_id,
            "status": team_run_status_to_str(&status),
        });
        sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, NULL, ?2, ?3, ?4)
            "#,
        )
        .bind(&run_id)
        .bind("run_submitted")
        .bind(now)
        .bind(payload.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(TeamRunRecord {
            id: run_id,
            team_id: team_id.to_string(),
            context_id: resolved_context_id,
            status,
            input,
            created_at: now,
            started_at: None,
            ended_at: None,
        })
    }

    pub async fn get_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, team_id, context_id, status, input_json, created_at, started_at, ended_at
            FROM team_runs
            WHERE id = ?1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_run_row(&row)
    }

    pub async fn cancel_run(&self, run_id: &str) -> anyhow::Result<TeamRunRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(
            r#"
            UPDATE team_runs
            SET status = 'canceled', ended_at = COALESCE(ended_at, ?1)
            WHERE id = ?2 AND status NOT IN ('completed', 'failed', 'canceled')
            "#,
        )
        .bind(now)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() > 0 {
            let payload = serde_json::json!({ "status": "canceled" });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, NULL, ?2, ?3, ?4)
                "#,
            )
            .bind(run_id)
            .bind("run_canceled")
            .bind(now)
            .bind(payload.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        self.get_run(run_id).await
    }

    pub async fn list_run_events(
        &self,
        run_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamRunEventRecord>> {
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, run_id, step_id, event_type, ts, payload_json
                FROM team_run_events
                WHERE run_id = ?1 AND id < ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(run_id)
            .bind(before_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, run_id, step_id, event_type, ts, payload_json
                FROM team_run_events
                WHERE run_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(run_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(parse_run_event_row(&row)?);
        }
        events.reverse();
        Ok(events)
    }
}

fn parse_team_definition_row(
    row: &sqlx::sqlite::SqliteRow,
) -> anyhow::Result<TeamDefinitionRecord> {
    let spec_json: String = row.get("spec_json");
    let spec: Value = serde_json::from_str(&spec_json)?;
    Ok(TeamDefinitionRecord {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        spec,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn parse_team_run_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamRunRecord> {
    let input_json: String = row.get("input_json");
    let input: Value = serde_json::from_str(&input_json)?;
    let status_raw: String = row.get("status");
    Ok(TeamRunRecord {
        id: row.get("id"),
        team_id: row.get("team_id"),
        context_id: row.get("context_id"),
        status: team_run_status_from_str(&status_raw),
        input,
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
    })
}

fn parse_run_event_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamRunEventRecord> {
    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json)?;
    Ok(TeamRunEventRecord {
        event_id: row.get("id"),
        run_id: row.get("run_id"),
        step_id: row.get("step_id"),
        event_type: row.get("event_type"),
        ts: row.get("ts"),
        payload,
    })
}

fn team_run_status_to_str(status: &TeamRunStatus) -> &'static str {
    match status {
        TeamRunStatus::Submitted => "submitted",
        TeamRunStatus::Working => "working",
        TeamRunStatus::InputRequired => "input_required",
        TeamRunStatus::Completed => "completed",
        TeamRunStatus::Failed => "failed",
        TeamRunStatus::Canceled => "canceled",
    }
}

fn team_run_status_from_str(status: &str) -> TeamRunStatus {
    match status {
        "working" => TeamRunStatus::Working,
        "input_required" => TeamRunStatus::InputRequired,
        "completed" => TeamRunStatus::Completed,
        "failed" => TeamRunStatus::Failed,
        "canceled" => TeamRunStatus::Canceled,
        _ => TeamRunStatus::Submitted,
    }
}

#[cfg(test)]
mod tests {
    use super::TeamManager;
    use crate::team::TeamDefinitionConfig;
    use serde_json::json;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::{Row, SqlitePool};

    async fn setup_test_db() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect sqlite");

        sqlx::query(
            r#"
            CREATE TABLE team_definitions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                spec_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_definitions");

        sqlx::query(
            r#"
            CREATE TABLE team_runs (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                context_id TEXT NOT NULL,
                status TEXT NOT NULL,
                input_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                ended_at INTEGER,
                FOREIGN KEY(team_id) REFERENCES team_definitions(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_runs");

        sqlx::query(
            r#"
            CREATE TABLE team_run_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                step_id TEXT,
                event_type TEXT NOT NULL,
                ts INTEGER NOT NULL,
                payload_json TEXT NOT NULL,
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_run_events");

        pool
    }

    #[tokio::test]
    async fn create_team_and_run_records_submission_event() {
        let db = setup_test_db().await;
        let manager = TeamManager::new(db.clone());

        let team = manager
            .create_team(TeamDefinitionConfig {
                name: "review-team".to_string(),
                description: Some("team for review tasks".to_string()),
                spec: json!({"entrypoint":"triage","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        assert_eq!(team.name, "review-team");

        let run = manager
            .create_run(&team.id, None, json!({"prompt":"check plan"}))
            .await
            .expect("create run");
        assert_eq!(run.status, crate::team::TeamRunStatus::Submitted);

        let row = sqlx::query(
            "SELECT event_type, run_id FROM team_run_events WHERE run_id = ?1 ORDER BY id ASC LIMIT 1",
        )
        .bind(&run.id)
        .fetch_one(&db)
        .await
        .expect("read run event");
        let event_type: String = row.get("event_type");
        let run_id: String = row.get("run_id");
        assert_eq!(event_type, "run_submitted");
        assert_eq!(run_id, run.id);
    }

    #[tokio::test]
    async fn cancel_run_updates_status_and_emits_event() {
        let db = setup_test_db().await;
        let manager = TeamManager::new(db.clone());

        let team = manager
            .create_team(TeamDefinitionConfig {
                name: "cancel-team".to_string(),
                description: None,
                spec: json!({"entrypoint":"main","members":[]}),
            })
            .await
            .expect("create team");
        let run = manager
            .create_run(&team.id, Some("ctx-1"), json!({"payload":1}))
            .await
            .expect("create run");

        let canceled = manager.cancel_run(&run.id).await.expect("cancel run");
        assert_eq!(canceled.status, crate::team::TeamRunStatus::Canceled);
        assert!(canceled.ended_at.is_some());

        let events = manager
            .list_run_events(&run.id, 100, None)
            .await
            .expect("list run events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "run_submitted");
        assert_eq!(events[1].event_type, "run_canceled");
    }
}
