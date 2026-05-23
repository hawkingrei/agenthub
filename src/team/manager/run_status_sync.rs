use std::collections::HashMap;

use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::{
    CONTINUITY_MAX_SUMMARY_CHARS, TeamRunRecord, TeamRunStatus, TeamTaskStatus,
    build_continuity_snapshot, codec, team_task_status_to_str,
};
use agenthub_text::truncate_chars;

pub(super) async fn load_run_summary(
    db: &SqlitePool,
    run_id: &str,
    status: &TeamRunStatus,
) -> anyhow::Result<Option<String>> {
    let row = sqlx::query(
        r#"
        SELECT output_json, error_text
        FROM team_steps
        WHERE run_id = ?1
          AND (output_json IS NOT NULL OR error_text IS NOT NULL)
        ORDER BY COALESCE(ended_at, started_at, 0) DESC, attempt DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .fetch_optional(db)
    .await?;

    if let Some(row) = row {
        let output_json = row.try_get::<Option<String>, _>("output_json")?;
        let error_text = row.try_get::<Option<String>, _>("error_text")?;
        if let Some(summary) =
            summarize_run_summary_fields(output_json.as_deref(), error_text.as_deref())
        {
            return Ok(Some(summary));
        }
    }

    Ok(fallback_run_summary(status))
}

pub(super) async fn load_run_summaries(
    db: &SqlitePool,
    runs: &[TeamRunRecord],
) -> anyhow::Result<HashMap<String, Option<String>>> {
    let mut summaries = HashMap::with_capacity(runs.len());
    if runs.is_empty() {
        return Ok(summaries);
    }

    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT run_id, output_json, error_text
        FROM team_steps
        WHERE run_id IN (
        "#,
    );
    {
        let mut separated = builder.separated(", ");
        for run in runs {
            separated.push_bind(&run.id);
        }
    }
    builder.push(
        r#")
          AND (output_json IS NOT NULL OR error_text IS NOT NULL)
        ORDER BY run_id ASC, COALESCE(ended_at, started_at, 0) DESC, attempt DESC, id DESC
        "#,
    );

    let rows = builder.build().fetch_all(db).await?;
    for row in rows {
        let run_id = row.try_get::<String, _>("run_id")?;
        let output_json = row.try_get::<Option<String>, _>("output_json")?;
        let error_text = row.try_get::<Option<String>, _>("error_text")?;
        summaries.entry(run_id).or_insert_with(|| {
            summarize_run_summary_fields(output_json.as_deref(), error_text.as_deref())
        });
    }

    for run in runs {
        summaries
            .entry(run.id.clone())
            .or_insert_with(|| fallback_run_summary(&run.status));
    }
    Ok(summaries)
}

fn summarize_run_summary_fields(
    output_json: Option<&str>,
    error_text: Option<&str>,
) -> Option<String> {
    if let Some(output_json) = output_json
        && let Ok(output) = serde_json::from_str::<Value>(output_json)
    {
        let summary = build_continuity_snapshot(Some(&output)).summary_text;
        if !summary.trim().is_empty() {
            return Some(summary);
        }
    }
    if let Some(error_text) = error_text {
        let trimmed = error_text.trim();
        if !trimmed.is_empty() {
            return Some(truncate_chars(trimmed, CONTINUITY_MAX_SUMMARY_CHARS));
        }
    }
    None
}

pub(super) fn fallback_run_summary(status: &TeamRunStatus) -> Option<String> {
    let fallback = match status {
        TeamRunStatus::Completed => Some("Completed without a structured summary."),
        TeamRunStatus::Failed => Some("Run failed before a structured summary was recorded."),
        TeamRunStatus::Canceled => Some("Run was canceled before completion."),
        TeamRunStatus::Submitted | TeamRunStatus::Working | TeamRunStatus::InputRequired => None,
    };
    fallback.map(str::to_string)
}

pub(super) async fn load_linked_task_ids_for_runs(
    db: &SqlitePool,
    run_ids: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    let mut linked_task_ids = HashMap::new();
    if run_ids.is_empty() {
        return Ok(linked_task_ids);
    }

    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT id, trim(COALESCE(json_extract(input_json, '$.task_id'), '')) AS task_id
        FROM team_runs
        WHERE id IN (
        "#,
    );
    {
        let mut separated = builder.separated(", ");
        for run_id in run_ids {
            separated.push_bind(run_id);
        }
    }
    builder.push(")");

    let rows = builder.build().fetch_all(db).await?;
    for row in rows {
        let run_id = row.try_get::<String, _>("id")?;
        let task_id = row.try_get::<String, _>("task_id")?;
        let task_id = task_id.trim();
        if !task_id.is_empty() {
            linked_task_ids.insert(run_id, task_id.to_string());
        }
    }
    Ok(linked_task_ids)
}

pub(super) fn extract_linked_task_id_from_run_input(input: &Value) -> Option<&str> {
    input
        .as_object()
        .and_then(|obj| obj.get("task_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn compute_next_task_execution_context(
    current_status: &TeamTaskStatus,
    current_context: &Value,
    next_status: &TeamTaskStatus,
) -> Option<Value> {
    if *next_status != TeamTaskStatus::InProgress || *current_status == TeamTaskStatus::InProgress {
        return None;
    }

    let mut next_context = current_context.clone();
    let next_attempt_number = current_context
        .pointer("/execution/attempt_number")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        + 1;

    let context_obj = next_context
        .as_object_mut()
        .expect("team task context should always be a JSON object");
    let execution = context_obj
        .entry("execution".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let execution_obj = execution
        .as_object_mut()
        .expect("task execution context should always be a JSON object");
    execution_obj.insert(
        "attempt_number".to_string(),
        Value::Number(next_attempt_number.into()),
    );
    Some(next_context)
}

pub(super) async fn load_run_status_sync_meta_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> anyhow::Result<(String, Value)> {
    let run_meta_row = sqlx::query(
        r#"
        SELECT team_id, input_json
        FROM team_runs
        WHERE id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;
    let team_id: String = run_meta_row.get("team_id");
    let run_input_json: String = run_meta_row.get("input_json");
    let run_input: Value =
        serde_json::from_str(&run_input_json).unwrap_or_else(|_| serde_json::json!({}));
    Ok((team_id, run_input))
}

pub(super) async fn sync_linked_task_status_tx(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    team_id: &str,
    input: &Value,
    status: TeamTaskStatus,
    now: i64,
    preserve_waiting: bool,
) -> anyhow::Result<()> {
    let Some(task_id) = extract_linked_task_id_from_run_input(input) else {
        return Ok(());
    };

    let current_row = sqlx::query(
        r#"
        SELECT status, context_json
        FROM team_tasks
        WHERE id = ?1 AND team_id = ?2
        "#,
    )
    .bind(task_id)
    .bind(team_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(current_row) = current_row else {
        return Ok(());
    };

    let current_status_raw: String = current_row.get("status");
    let current_status = codec::team_task_status_from_str(&current_status_raw);
    let current_context_json: String = current_row.get("context_json");
    let current_context: Value =
        serde_json::from_str(&current_context_json).unwrap_or_else(|_| serde_json::json!({}));

    let effective_status = if preserve_waiting && current_status == TeamTaskStatus::Waiting {
        current_status.clone()
    } else {
        status
    };
    let next_context =
        compute_next_task_execution_context(&current_status, &current_context, &effective_status);
    if current_status == effective_status && next_context.is_none() {
        return Ok(());
    }

    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE team_tasks SET ");
    let mut first = true;
    if current_status != effective_status {
        builder.push("status = ");
        builder.push_bind(team_task_status_to_str(&effective_status));
        first = false;
    }
    if let Some(next_context) = next_context.as_ref() {
        builder.push(", ");
        builder.push("context_json = ");
        builder.push_bind(next_context.to_string());
    }
    if !first {
        builder.push(", ");
    }
    builder.push("updated_at = ");
    builder.push_bind(now);
    builder.push(" WHERE id = ");
    builder.push_bind(task_id);
    builder.push(" AND team_id = ");
    builder.push_bind(team_id);
    builder.build().execute(&mut **tx).await?;
    Ok(())
}
