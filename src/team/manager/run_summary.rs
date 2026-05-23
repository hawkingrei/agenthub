use std::collections::HashMap;

use serde_json::Value;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use super::step_continuity::build_continuity_snapshot;
use super::{CONTINUITY_MAX_SUMMARY_CHARS, TeamRunRecord, TeamRunStatus};
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
