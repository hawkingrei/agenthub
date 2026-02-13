use chrono::Utc;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{
    TeamDefinitionConfig, TeamDefinitionRecord, TeamRunEventRecord, TeamRunRecord, TeamRunStatus,
    TeamStepRecord, TeamStepStatus,
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

    #[allow(dead_code)]
    pub async fn submit_step(
        &self,
        run_id: &str,
        step_key: &str,
        member_id: &str,
        depends_on: Vec<String>,
        input: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let step_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let status = TeamStepStatus::Submitted;
        let depends_on_json = serde_json::to_string(&depends_on)?;
        let input_json = input.as_ref().map(serde_json::to_string).transpose()?;

        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_steps (
                id, run_id, step_key, member_id, remote_task_id, status, attempt, depends_on_json, input_json
            )
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, 0, ?6, ?7)
            "#,
        )
        .bind(&step_id)
        .bind(run_id)
        .bind(step_key)
        .bind(member_id)
        .bind(team_step_status_to_str(&status))
        .bind(depends_on_json)
        .bind(input_json)
        .execute(&mut *tx)
        .await?;

        let payload = serde_json::json!({
            "step_id": step_id,
            "step_key": step_key,
            "member_id": member_id,
            "status": team_step_status_to_str(&status),
        });
        sqlx::query(
            r#"
            INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(run_id)
        .bind(&step_id)
        .bind("step_submitted")
        .bind(now)
        .bind(payload.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get_step(&step_id).await
    }

    #[allow(dead_code)]
    pub async fn get_step(&self, step_id: &str) -> anyhow::Result<TeamStepRecord> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                step_key,
                member_id,
                remote_task_id,
                status,
                attempt,
                depends_on_json,
                input_json,
                output_json,
                error_text,
                started_at,
                ended_at
            FROM team_steps
            WHERE id = ?1
            "#,
        )
        .bind(step_id)
        .fetch_one(&self.db)
        .await?;
        parse_team_step_row(&row)
    }

    pub async fn list_steps(&self, run_id: &str) -> anyhow::Result<Vec<TeamStepRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                step_key,
                member_id,
                remote_task_id,
                status,
                attempt,
                depends_on_json,
                input_json,
                output_json,
                error_text,
                started_at,
                ended_at
            FROM team_steps
            WHERE run_id = ?1
            ORDER BY attempt ASC, step_key ASC, id ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.db)
        .await?;

        let mut steps = Vec::with_capacity(rows.len());
        for row in rows {
            steps.push(parse_team_step_row(&row)?);
        }
        Ok(steps)
    }

    #[allow(dead_code)]
    pub async fn start_step(
        &self,
        step_id: &str,
        remote_task_id: Option<&str>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'working',
                remote_task_id = COALESCE(?1, remote_task_id),
                started_at = COALESCE(started_at, ?2)
            WHERE id = ?3 AND status IN ('submitted', 'input_required')
            "#,
        )
        .bind(remote_task_id)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

        let step_row = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                step_key,
                member_id,
                remote_task_id,
                status,
                attempt,
                depends_on_json,
                input_json,
                output_json,
                error_text,
                started_at,
                ended_at
            FROM team_steps
            WHERE id = ?1
            "#,
        )
        .bind(step_id)
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'working', started_at = COALESCE(started_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'input_required')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;
            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "working",
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(&step.run_id)
                .bind("run_working")
                .bind(now)
                .bind(run_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }

            let step_payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "working",
                "remote_task_id": step.remote_task_id,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_working")
            .bind(now)
            .bind(step_payload.to_string())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn complete_step(
        &self,
        step_id: &str,
        output: Option<Value>,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let output_json = output.as_ref().map(serde_json::to_string).transpose()?;
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'completed',
                output_json = ?1,
                ended_at = COALESCE(ended_at, ?2)
            WHERE id = ?3 AND status IN ('working', 'input_required')
            "#,
        )
        .bind(output_json)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

        let step_row = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                step_key,
                member_id,
                remote_task_id,
                status,
                attempt,
                depends_on_json,
                input_json,
                output_json,
                error_text,
                started_at,
                ended_at
            FROM team_steps
            WHERE id = ?1
            "#,
        )
        .bind(step_id)
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "completed",
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_completed")
            .bind(now)
            .bind(payload.to_string())
            .execute(&mut *tx)
            .await?;

            let non_completed_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM team_steps
                WHERE run_id = ?1 AND status <> 'completed'
                "#,
            )
            .bind(&step.run_id)
            .fetch_one(&mut *tx)
            .await?;

            if non_completed_count == 0 {
                let run_update = sqlx::query(
                    r#"
                    UPDATE team_runs
                    SET status = 'completed', ended_at = COALESCE(ended_at, ?1)
                    WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
                    "#,
                )
                .bind(now)
                .bind(&step.run_id)
                .execute(&mut *tx)
                .await?;

                if run_update.rows_affected() > 0 {
                    let run_payload = serde_json::json!({
                        "status": "completed",
                    });
                    sqlx::query(
                        r#"
                        INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                        VALUES (?1, NULL, ?2, ?3, ?4)
                        "#,
                    )
                    .bind(&step.run_id)
                    .bind("run_completed")
                    .bind(now)
                    .bind(run_payload.to_string())
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(step)
    }

    #[allow(dead_code)]
    pub async fn fail_step(
        &self,
        step_id: &str,
        error_text: &str,
    ) -> anyhow::Result<TeamStepRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE team_steps
            SET
                status = 'failed',
                error_text = ?1,
                ended_at = COALESCE(ended_at, ?2)
            WHERE id = ?3 AND status IN ('submitted', 'working', 'input_required')
            "#,
        )
        .bind(error_text)
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;

        let step_row = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                step_key,
                member_id,
                remote_task_id,
                status,
                attempt,
                depends_on_json,
                input_json,
                output_json,
                error_text,
                started_at,
                ended_at
            FROM team_steps
            WHERE id = ?1
            "#,
        )
        .bind(step_id)
        .fetch_one(&mut *tx)
        .await?;
        let step = parse_team_step_row(&step_row)?;

        if update.rows_affected() > 0 {
            let payload = serde_json::json!({
                "step_id": step.id,
                "step_key": step.step_key,
                "status": "failed",
                "error_text": step.error_text,
            });
            sqlx::query(
                r#"
                INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(&step.run_id)
            .bind(&step.id)
            .bind("step_failed")
            .bind(now)
            .bind(payload.to_string())
            .execute(&mut *tx)
            .await?;

            let run_update = sqlx::query(
                r#"
                UPDATE team_runs
                SET status = 'failed', ended_at = COALESCE(ended_at, ?1)
                WHERE id = ?2 AND status IN ('submitted', 'working', 'input_required')
                "#,
            )
            .bind(now)
            .bind(&step.run_id)
            .execute(&mut *tx)
            .await?;

            if run_update.rows_affected() > 0 {
                let run_payload = serde_json::json!({
                    "status": "failed",
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, NULL, ?2, ?3, ?4)
                    "#,
                )
                .bind(&step.run_id)
                .bind("run_failed")
                .bind(now)
                .bind(run_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(step)
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
            let active_steps = sqlx::query(
                r#"
                SELECT id, step_key
                FROM team_steps
                WHERE run_id = ?1 AND status NOT IN ('completed', 'failed', 'canceled')
                "#,
            )
            .bind(run_id)
            .fetch_all(&mut *tx)
            .await?;

            for step in active_steps {
                let step_id: String = step.get("id");
                let step_key: String = step.get("step_key");
                sqlx::query(
                    r#"
                    UPDATE team_steps
                    SET status = 'canceled', ended_at = COALESCE(ended_at, ?1)
                    WHERE id = ?2
                    "#,
                )
                .bind(now)
                .bind(&step_id)
                .execute(&mut *tx)
                .await?;

                let step_payload = serde_json::json!({
                    "step_id": step_id,
                    "step_key": step_key,
                    "status": "canceled",
                });
                sqlx::query(
                    r#"
                    INSERT INTO team_run_events (run_id, step_id, event_type, ts, payload_json)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                )
                .bind(run_id)
                .bind(&step_id)
                .bind("step_canceled")
                .bind(now)
                .bind(step_payload.to_string())
                .execute(&mut *tx)
                .await?;
            }

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

#[allow(dead_code)]
fn parse_team_step_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamStepRecord> {
    let status_raw: String = row.get("status");
    let depends_on_json: String = row.get("depends_on_json");
    let depends_on: Vec<String> = serde_json::from_str(&depends_on_json)?;
    let input = row
        .try_get::<Option<String>, _>("input_json")?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()?;
    let output = row
        .try_get::<Option<String>, _>("output_json")?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()?;
    Ok(TeamStepRecord {
        id: row.get("id"),
        run_id: row.get("run_id"),
        step_key: row.get("step_key"),
        member_id: row.get("member_id"),
        remote_task_id: row.try_get("remote_task_id")?,
        status: team_step_status_from_str(&status_raw),
        attempt: row.get("attempt"),
        depends_on,
        input,
        output,
        error_text: row.try_get("error_text")?,
        started_at: row.try_get("started_at")?,
        ended_at: row.try_get("ended_at")?,
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

#[allow(dead_code)]
fn team_step_status_to_str(status: &TeamStepStatus) -> &'static str {
    match status {
        TeamStepStatus::Submitted => "submitted",
        TeamStepStatus::Working => "working",
        TeamStepStatus::InputRequired => "input_required",
        TeamStepStatus::Completed => "completed",
        TeamStepStatus::Failed => "failed",
        TeamStepStatus::Canceled => "canceled",
    }
}

#[allow(dead_code)]
fn team_step_status_from_str(status: &str) -> TeamStepStatus {
    match status {
        "working" => TeamStepStatus::Working,
        "input_required" => TeamStepStatus::InputRequired,
        "completed" => TeamStepStatus::Completed,
        "failed" => TeamStepStatus::Failed,
        "canceled" => TeamStepStatus::Canceled,
        _ => TeamStepStatus::Submitted,
    }
}

#[cfg(test)]
mod tests {
    use super::TeamManager;
    use crate::team::{TeamDefinitionConfig, TeamRunStatus, TeamStepStatus};
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
            CREATE TABLE team_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_key TEXT NOT NULL,
                member_id TEXT NOT NULL,
                remote_task_id TEXT,
                status TEXT NOT NULL,
                attempt INTEGER NOT NULL DEFAULT 0,
                depends_on_json TEXT NOT NULL DEFAULT '[]',
                input_json TEXT,
                output_json TEXT,
                error_text TEXT,
                started_at INTEGER,
                ended_at INTEGER,
                UNIQUE(run_id, step_key, attempt),
                FOREIGN KEY(run_id) REFERENCES team_runs(id)
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("create team_steps");

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

    #[tokio::test]
    async fn step_lifecycle_transitions_persist_and_emit_events() {
        let db = setup_test_db().await;
        let manager = TeamManager::new(db.clone());

        let team = manager
            .create_team(TeamDefinitionConfig {
                name: "step-team".to_string(),
                description: Some("team with step lifecycle".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = manager
            .create_run(&team.id, Some("ctx-step"), json!({"payload":"start"}))
            .await
            .expect("create run");

        let step = manager
            .submit_step(
                &run.id,
                "plan_step",
                "planner",
                Vec::new(),
                Some(json!({"goal":"draft plan"})),
            )
            .await
            .expect("submit step");
        assert_eq!(step.status, TeamStepStatus::Submitted);

        let working = manager
            .start_step(&step.id, Some("remote-task-1"))
            .await
            .expect("start step");
        assert_eq!(working.status, TeamStepStatus::Working);
        assert_eq!(working.remote_task_id.as_deref(), Some("remote-task-1"));
        assert!(working.started_at.is_some());

        let run_after_start = manager.get_run(&run.id).await.expect("get run");
        assert_eq!(run_after_start.status, TeamRunStatus::Working);
        assert!(run_after_start.started_at.is_some());

        let completed = manager
            .complete_step(&step.id, Some(json!({"result":"ok"})))
            .await
            .expect("complete step");
        assert_eq!(completed.status, TeamStepStatus::Completed);
        assert_eq!(completed.output, Some(json!({"result":"ok"})));
        assert!(completed.ended_at.is_some());

        let run_after_complete = manager.get_run(&run.id).await.expect("get run");
        assert_eq!(run_after_complete.status, TeamRunStatus::Completed);
        assert!(run_after_complete.ended_at.is_some());

        let events = manager
            .list_run_events(&run.id, 100, None)
            .await
            .expect("list run events");
        let event_types: Vec<&str> = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect();
        assert_eq!(
            event_types,
            vec![
                "run_submitted",
                "step_submitted",
                "run_working",
                "step_working",
                "step_completed",
                "run_completed"
            ]
        );
    }

    #[tokio::test]
    async fn list_steps_returns_sorted_steps_for_a_run() {
        let db = setup_test_db().await;
        let manager = TeamManager::new(db.clone());

        let team = manager
            .create_team(TeamDefinitionConfig {
                name: "list-steps-team".to_string(),
                description: Some("team for step listing".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = manager
            .create_run(&team.id, Some("ctx-list"), json!({"payload":"list"}))
            .await
            .expect("create run");
        let run_2 = manager
            .create_run(&team.id, Some("ctx-list-2"), json!({"payload":"list-2"}))
            .await
            .expect("create second run");

        let _ = manager
            .submit_step(
                &run.id,
                "z-step",
                "planner",
                Vec::new(),
                Some(json!({"goal":"z"})),
            )
            .await
            .expect("submit z step");
        let _ = manager
            .submit_step(
                &run.id,
                "a-step",
                "planner",
                Vec::new(),
                Some(json!({"goal":"a"})),
            )
            .await
            .expect("submit a step");
        let _ = manager
            .submit_step(
                &run_2.id,
                "other-run-step",
                "planner",
                Vec::new(),
                Some(json!({"goal":"other"})),
            )
            .await
            .expect("submit step in other run");

        let listed = manager.list_steps(&run.id).await.expect("list steps");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].run_id, run.id);
        assert_eq!(listed[1].run_id, run.id);
        assert_eq!(
            listed
                .iter()
                .map(|step| step.step_key.as_str())
                .collect::<Vec<_>>(),
            vec!["a-step", "z-step"]
        );
    }

    #[tokio::test]
    async fn run_completes_only_after_all_steps_complete() {
        let db = setup_test_db().await;
        let manager = TeamManager::new(db.clone());

        let team = manager
            .create_team(TeamDefinitionConfig {
                name: "multi-step-team".to_string(),
                description: Some("team with two parallel steps".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"},{"member_id":"reviewer"}]}),
            })
            .await
            .expect("create team");
        let run = manager
            .create_run(&team.id, Some("ctx-multi"), json!({"payload":"start"}))
            .await
            .expect("create run");

        let step_1 = manager
            .submit_step(
                &run.id,
                "plan_step",
                "planner",
                Vec::new(),
                Some(json!({"goal":"draft"})),
            )
            .await
            .expect("submit step 1");
        let step_2 = manager
            .submit_step(
                &run.id,
                "review_step",
                "reviewer",
                vec!["plan_step".to_string()],
                Some(json!({"goal":"review"})),
            )
            .await
            .expect("submit step 2");

        let _ = manager
            .start_step(&step_1.id, Some("remote-task-1"))
            .await
            .expect("start step 1");
        let _ = manager
            .start_step(&step_2.id, Some("remote-task-2"))
            .await
            .expect("start step 2");

        let _ = manager
            .complete_step(&step_1.id, Some(json!({"result":"done-1"})))
            .await
            .expect("complete step 1");
        let run_after_first_complete = manager.get_run(&run.id).await.expect("get run");
        assert_eq!(run_after_first_complete.status, TeamRunStatus::Working);
        assert!(run_after_first_complete.ended_at.is_none());

        let _ = manager
            .complete_step(&step_2.id, Some(json!({"result":"done-2"})))
            .await
            .expect("complete step 2");
        let run_after_second_complete = manager.get_run(&run.id).await.expect("get run");
        assert_eq!(run_after_second_complete.status, TeamRunStatus::Completed);
        assert!(run_after_second_complete.ended_at.is_some());

        let events = manager
            .list_run_events(&run.id, 100, None)
            .await
            .expect("list run events");
        let run_completed_count = events
            .iter()
            .filter(|event| event.event_type == "run_completed")
            .count();
        assert_eq!(run_completed_count, 1);
        assert_eq!(
            events.last().map(|event| event.event_type.as_str()),
            Some("run_completed")
        );
    }

    #[tokio::test]
    async fn fail_step_updates_status_and_emits_event() {
        let db = setup_test_db().await;
        let manager = TeamManager::new(db.clone());

        let team = manager
            .create_team(TeamDefinitionConfig {
                name: "fail-step-team".to_string(),
                description: Some("team with failure".to_string()),
                spec: json!({"entrypoint":"planner","members":[{"member_id":"planner"}]}),
            })
            .await
            .expect("create team");
        let run = manager
            .create_run(&team.id, Some("ctx-fail"), json!({"payload":"start"}))
            .await
            .expect("create run");
        let step = manager
            .submit_step(
                &run.id,
                "failing_step",
                "planner",
                Vec::new(),
                Some(json!({"goal":"can fail"})),
            )
            .await
            .expect("submit step");

        let _ = manager
            .start_step(&step.id, Some("remote-task-fail"))
            .await
            .expect("start step");
        let failed = manager
            .fail_step(&step.id, "remote task failed")
            .await
            .expect("fail step");
        assert_eq!(failed.status, TeamStepStatus::Failed);
        assert_eq!(failed.error_text.as_deref(), Some("remote task failed"));

        let run_after_fail = manager.get_run(&run.id).await.expect("get run");
        assert_eq!(run_after_fail.status, TeamRunStatus::Failed);
        assert!(run_after_fail.ended_at.is_some());

        let events = manager
            .list_run_events(&run.id, 100, None)
            .await
            .expect("list run events");
        let event_types: Vec<&str> = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect();
        assert_eq!(
            event_types,
            vec![
                "run_submitted",
                "step_submitted",
                "run_working",
                "step_working",
                "step_failed",
                "run_failed"
            ]
        );
    }
}
