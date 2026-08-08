use anyhow::Context;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ObjectUploadRecord {
    pub id: String,
    pub owner_scope: String,
    pub backend: String,
    pub object_key: String,
    pub original_filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub public_url: Option<String>,
    pub created_by_actor_id: String,
    pub publish_state: String,
    pub created_at: i64,
    pub published_at: Option<i64>,
    pub cleanup_after: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewObjectUpload<'a> {
    pub id: &'a str,
    pub owner_scope: &'a str,
    pub backend: &'a str,
    pub object_key: &'a str,
    pub original_filename: &'a str,
    pub content_type: &'a str,
    pub size_bytes: i64,
    pub sha256: &'a str,
    pub public_url: Option<&'a str>,
    pub created_by_actor_id: &'a str,
    pub publish_state: &'a str,
    pub created_at: i64,
    pub published_at: Option<i64>,
    pub cleanup_after: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ObjectUploadSessionRecord {
    pub id: String,
    pub owner_scope: String,
    pub backend: String,
    pub object_key: String,
    pub original_filename: String,
    pub content_type: String,
    pub object_kind: String,
    pub expected_size_bytes: i64,
    pub expected_sha256: Option<String>,
    pub created_by_actor_id: String,
    pub status: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub completed_at: Option<i64>,
    pub canceled_at: Option<i64>,
    pub published_upload_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ObjectUploadSessionPartRecord {
    pub session_id: String,
    pub part_number: i64,
    pub object_key: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub uploaded_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewObjectUploadSession<'a> {
    pub id: &'a str,
    pub owner_scope: &'a str,
    pub backend: &'a str,
    pub object_key: &'a str,
    pub original_filename: &'a str,
    pub content_type: &'a str,
    pub object_kind: &'a str,
    pub expected_size_bytes: i64,
    pub expected_sha256: Option<&'a str>,
    pub created_by_actor_id: &'a str,
    pub status: &'a str,
    pub created_at: i64,
    pub expires_at: i64,
    pub completed_at: Option<i64>,
    pub canceled_at: Option<i64>,
    pub published_upload_id: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewObjectUploadSessionPart<'a> {
    pub session_id: &'a str,
    pub part_number: i64,
    pub object_key: &'a str,
    pub size_bytes: i64,
    pub sha256: &'a str,
    pub uploaded_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectUploadSessionCleanupResult {
    pub cutoff_ts: i64,
    pub expired_sessions: u64,
    pub expire_batches: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectDownloadCleanupOutcome {
    NotAttempted,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectDownloadMetricDelta<'a> {
    pub backend: &'a str,
    pub succeeded: bool,
    pub downloaded_bytes: i64,
    pub latency_ms: i64,
    pub failure_class: Option<&'a str>,
    pub cleanup_outcome: ObjectDownloadCleanupOutcome,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ObjectDownloadMetricsRecord {
    pub backend: String,
    pub attempts_total: i64,
    pub successes_total: i64,
    pub failures_total: i64,
    pub downloaded_bytes_total: i64,
    pub latency_ms_total: i64,
    pub latency_ms_max: i64,
    pub cleanup_attempts_total: i64,
    pub cleanup_successes_total: i64,
    pub cleanup_failures_total: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct ObjectDownloadFailureMetricRecord {
    pub backend: String,
    pub failure_class: String,
    pub failures_total: i64,
    pub last_failure_at: i64,
}

pub async fn record_object_download_metric(
    pool: &SqlitePool,
    metric: ObjectDownloadMetricDelta<'_>,
) -> anyhow::Result<()> {
    let backend = metric.backend.trim();
    anyhow::ensure!(
        !backend.is_empty(),
        "object download metric backend is required"
    );
    anyhow::ensure!(
        matches!(backend, "fs" | "s3"),
        "unsupported object download metric backend"
    );
    anyhow::ensure!(
        metric.downloaded_bytes >= 0,
        "object download metric bytes must be non-negative"
    );
    anyhow::ensure!(
        metric.latency_ms >= 0,
        "object download metric latency must be non-negative"
    );
    let failure_class = metric
        .failure_class
        .map(str::trim)
        .filter(|value| !value.is_empty());
    anyhow::ensure!(
        metric.succeeded == failure_class.is_none(),
        "successful object downloads must not have a failure class and failed downloads must have one"
    );
    anyhow::ensure!(
        failure_class.is_none_or(is_object_download_failure_class),
        "unsupported object download failure class"
    );

    let successes = i64::from(metric.succeeded);
    let failures = i64::from(!metric.succeeded);
    let (cleanup_attempts, cleanup_successes, cleanup_failures) = match metric.cleanup_outcome {
        ObjectDownloadCleanupOutcome::NotAttempted => (0_i64, 0_i64, 0_i64),
        ObjectDownloadCleanupOutcome::Succeeded => (1_i64, 1_i64, 0_i64),
        ObjectDownloadCleanupOutcome::Failed => (1_i64, 0_i64, 1_i64),
    };

    let mut tx = pool
        .begin()
        .await
        .context("begin object download metric update")?;
    sqlx::query(
        r#"
        INSERT INTO object_download_metrics (
            backend,
            attempts_total,
            successes_total,
            failures_total,
            downloaded_bytes_total,
            latency_ms_total,
            latency_ms_max,
            cleanup_attempts_total,
            cleanup_successes_total,
            cleanup_failures_total,
            updated_at
        )
        VALUES (?1, 1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(backend) DO UPDATE SET
            attempts_total = attempts_total + 1,
            successes_total = successes_total + excluded.successes_total,
            failures_total = failures_total + excluded.failures_total,
            downloaded_bytes_total = downloaded_bytes_total + excluded.downloaded_bytes_total,
            latency_ms_total = latency_ms_total + excluded.latency_ms_total,
            latency_ms_max = MAX(latency_ms_max, excluded.latency_ms_max),
            cleanup_attempts_total = cleanup_attempts_total + excluded.cleanup_attempts_total,
            cleanup_successes_total = cleanup_successes_total + excluded.cleanup_successes_total,
            cleanup_failures_total = cleanup_failures_total + excluded.cleanup_failures_total,
            updated_at = MAX(updated_at, excluded.updated_at)
        "#,
    )
    .bind(backend)
    .bind(successes)
    .bind(failures)
    .bind(metric.downloaded_bytes)
    .bind(metric.latency_ms)
    .bind(cleanup_attempts)
    .bind(cleanup_successes)
    .bind(cleanup_failures)
    .bind(metric.recorded_at)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("update object download metrics for backend {backend:?}"))?;

    if let Some(failure_class) = failure_class {
        sqlx::query(
            r#"
            INSERT INTO object_download_failure_metrics (
                backend,
                failure_class,
                failures_total,
                last_failure_at
            )
            VALUES (?1, ?2, 1, ?3)
            ON CONFLICT(backend, failure_class) DO UPDATE SET
                failures_total = failures_total + 1,
                last_failure_at = MAX(last_failure_at, excluded.last_failure_at)
            "#,
        )
        .bind(backend)
        .bind(failure_class)
        .bind(metric.recorded_at)
        .execute(&mut *tx)
        .await
        .with_context(|| {
            format!(
                "update object download failure metric for backend {backend:?} class {failure_class:?}"
            )
        })?;
    }

    tx.commit()
        .await
        .context("commit object download metric update")?;
    Ok(())
}

pub async fn get_object_download_metrics(
    pool: &SqlitePool,
    backend: &str,
) -> anyhow::Result<Option<ObjectDownloadMetricsRecord>> {
    let row = sqlx::query(
        r#"
        SELECT
            backend,
            attempts_total,
            successes_total,
            failures_total,
            downloaded_bytes_total,
            latency_ms_total,
            latency_ms_max,
            cleanup_attempts_total,
            cleanup_successes_total,
            cleanup_failures_total,
            updated_at
        FROM object_download_metrics
        WHERE backend = ?1
        "#,
    )
    .bind(backend.trim())
    .fetch_optional(pool)
    .await
    .with_context(|| format!("load object download metrics for backend {backend:?}"))?;

    Ok(row.map(|row| ObjectDownloadMetricsRecord {
        backend: row.get("backend"),
        attempts_total: row.get("attempts_total"),
        successes_total: row.get("successes_total"),
        failures_total: row.get("failures_total"),
        downloaded_bytes_total: row.get("downloaded_bytes_total"),
        latency_ms_total: row.get("latency_ms_total"),
        latency_ms_max: row.get("latency_ms_max"),
        cleanup_attempts_total: row.get("cleanup_attempts_total"),
        cleanup_successes_total: row.get("cleanup_successes_total"),
        cleanup_failures_total: row.get("cleanup_failures_total"),
        updated_at: row.get("updated_at"),
    }))
}

pub async fn list_object_download_failure_metrics(
    pool: &SqlitePool,
    backend: &str,
) -> anyhow::Result<Vec<ObjectDownloadFailureMetricRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT backend, failure_class, failures_total, last_failure_at
        FROM object_download_failure_metrics
        WHERE backend = ?1
        ORDER BY failure_class ASC
        "#,
    )
    .bind(backend.trim())
    .fetch_all(pool)
    .await
    .with_context(|| format!("list object download failure metrics for backend {backend:?}"))?;

    Ok(rows
        .iter()
        .map(|row| ObjectDownloadFailureMetricRecord {
            backend: row.get("backend"),
            failure_class: row.get("failure_class"),
            failures_total: row.get("failures_total"),
            last_failure_at: row.get("last_failure_at"),
        })
        .collect())
}

fn is_object_download_failure_class(value: &str) -> bool {
    matches!(
        value,
        "request_validation"
            | "transient_status"
            | "server_status"
            | "size_limit"
            | "source_policy"
            | "redirect"
            | "dns"
            | "source_stream"
            | "object_store"
            | "request"
            | "verification"
            | "metadata_publish"
    )
}

pub async fn insert_object_upload(
    pool: &SqlitePool,
    upload: &NewObjectUpload<'_>,
) -> anyhow::Result<ObjectUploadRecord> {
    sqlx::query(
        r#"
        INSERT INTO object_uploads (
            id,
            owner_scope,
            backend,
            object_key,
            original_filename,
            content_type,
            size_bytes,
            sha256,
            public_url,
            created_by_actor_id,
            publish_state,
            created_at,
            published_at,
            cleanup_after
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(upload.id)
    .bind(upload.owner_scope)
    .bind(upload.backend)
    .bind(upload.object_key)
    .bind(upload.original_filename)
    .bind(upload.content_type)
    .bind(upload.size_bytes)
    .bind(upload.sha256)
    .bind(upload.public_url)
    .bind(upload.created_by_actor_id)
    .bind(upload.publish_state)
    .bind(upload.created_at)
    .bind(upload.published_at)
    .bind(upload.cleanup_after)
    .execute(pool)
    .await
    .with_context(|| format!("insert object upload {}", upload.id))?;

    Ok(ObjectUploadRecord {
        id: upload.id.to_string(),
        owner_scope: upload.owner_scope.to_string(),
        backend: upload.backend.to_string(),
        object_key: upload.object_key.to_string(),
        original_filename: upload.original_filename.to_string(),
        content_type: upload.content_type.to_string(),
        size_bytes: upload.size_bytes,
        sha256: upload.sha256.to_string(),
        public_url: upload.public_url.map(str::to_string),
        created_by_actor_id: upload.created_by_actor_id.to_string(),
        publish_state: upload.publish_state.to_string(),
        created_at: upload.created_at,
        published_at: upload.published_at,
        cleanup_after: upload.cleanup_after,
    })
}

pub async fn get_object_upload(
    pool: &SqlitePool,
    upload_id: &str,
) -> anyhow::Result<ObjectUploadRecord> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            owner_scope,
            backend,
            object_key,
            original_filename,
            content_type,
            size_bytes,
            sha256,
            public_url,
            created_by_actor_id,
            publish_state,
            created_at,
            published_at,
            cleanup_after
        FROM object_uploads
        WHERE id = ?1
        "#,
    )
    .bind(upload_id)
    .fetch_one(pool)
    .await
    .with_context(|| format!("load object upload {upload_id}"))?;

    Ok(ObjectUploadRecord {
        id: row.get("id"),
        owner_scope: row.get("owner_scope"),
        backend: row.get("backend"),
        object_key: row.get("object_key"),
        original_filename: row.get("original_filename"),
        content_type: row.get("content_type"),
        size_bytes: row.get("size_bytes"),
        sha256: row.get("sha256"),
        public_url: row.get("public_url"),
        created_by_actor_id: row.get("created_by_actor_id"),
        publish_state: row.get("publish_state"),
        created_at: row.get("created_at"),
        published_at: row.get("published_at"),
        cleanup_after: row.get("cleanup_after"),
    })
}

pub async fn insert_object_upload_session(
    pool: &SqlitePool,
    session: &NewObjectUploadSession<'_>,
) -> anyhow::Result<ObjectUploadSessionRecord> {
    sqlx::query(
        r#"
        INSERT INTO object_upload_sessions (
            id,
            owner_scope,
            backend,
            object_key,
            original_filename,
            content_type,
            object_kind,
            expected_size_bytes,
            expected_sha256,
            created_by_actor_id,
            status,
            created_at,
            expires_at,
            completed_at,
            canceled_at,
            published_upload_id
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        "#,
    )
    .bind(session.id)
    .bind(session.owner_scope)
    .bind(session.backend)
    .bind(session.object_key)
    .bind(session.original_filename)
    .bind(session.content_type)
    .bind(session.object_kind)
    .bind(session.expected_size_bytes)
    .bind(session.expected_sha256)
    .bind(session.created_by_actor_id)
    .bind(session.status)
    .bind(session.created_at)
    .bind(session.expires_at)
    .bind(session.completed_at)
    .bind(session.canceled_at)
    .bind(session.published_upload_id)
    .execute(pool)
    .await
    .with_context(|| format!("insert object upload session {}", session.id))?;

    Ok(ObjectUploadSessionRecord {
        id: session.id.to_string(),
        owner_scope: session.owner_scope.to_string(),
        backend: session.backend.to_string(),
        object_key: session.object_key.to_string(),
        original_filename: session.original_filename.to_string(),
        content_type: session.content_type.to_string(),
        object_kind: session.object_kind.to_string(),
        expected_size_bytes: session.expected_size_bytes,
        expected_sha256: session.expected_sha256.map(str::to_string),
        created_by_actor_id: session.created_by_actor_id.to_string(),
        status: session.status.to_string(),
        created_at: session.created_at,
        expires_at: session.expires_at,
        completed_at: session.completed_at,
        canceled_at: session.canceled_at,
        published_upload_id: session.published_upload_id.map(str::to_string),
    })
}

pub async fn get_object_upload_session(
    pool: &SqlitePool,
    session_id: &str,
) -> anyhow::Result<ObjectUploadSessionRecord> {
    let row = sqlx::query(
        r#"
        SELECT
            id,
            owner_scope,
            backend,
            object_key,
            original_filename,
            content_type,
            object_kind,
            expected_size_bytes,
            expected_sha256,
            created_by_actor_id,
            status,
            created_at,
            expires_at,
            completed_at,
            canceled_at,
            published_upload_id
        FROM object_upload_sessions
        WHERE id = ?1
        "#,
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .with_context(|| format!("load object upload session {session_id}"))?;

    Ok(object_upload_session_from_row(&row))
}

pub async fn upsert_object_upload_session_part(
    pool: &SqlitePool,
    part: &NewObjectUploadSessionPart<'_>,
) -> anyhow::Result<ObjectUploadSessionPartRecord> {
    sqlx::query(
        r#"
        INSERT INTO object_upload_session_parts (
            session_id,
            part_number,
            object_key,
            size_bytes,
            sha256,
            uploaded_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(session_id, part_number) DO UPDATE SET
            object_key = excluded.object_key,
            size_bytes = excluded.size_bytes,
            sha256 = excluded.sha256,
            uploaded_at = excluded.uploaded_at
        "#,
    )
    .bind(part.session_id)
    .bind(part.part_number)
    .bind(part.object_key)
    .bind(part.size_bytes)
    .bind(part.sha256)
    .bind(part.uploaded_at)
    .execute(pool)
    .await
    .with_context(|| {
        format!(
            "upsert object upload session part {}:{}",
            part.session_id, part.part_number
        )
    })?;

    let row = sqlx::query(
        r#"
        SELECT session_id, part_number, object_key, size_bytes, sha256, uploaded_at
        FROM object_upload_session_parts
        WHERE session_id = ?1 AND part_number = ?2
        "#,
    )
    .bind(part.session_id)
    .bind(part.part_number)
    .fetch_one(pool)
    .await
    .with_context(|| {
        format!(
            "load object upload session part {}:{}",
            part.session_id, part.part_number
        )
    })?;

    Ok(object_upload_session_part_from_row(&row))
}

pub async fn list_object_upload_session_parts(
    pool: &SqlitePool,
    session_id: &str,
) -> anyhow::Result<Vec<ObjectUploadSessionPartRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT session_id, part_number, object_key, size_bytes, sha256, uploaded_at
        FROM object_upload_session_parts
        WHERE session_id = ?1
        ORDER BY part_number ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("list object upload session parts {session_id}"))?;

    Ok(rows
        .iter()
        .map(object_upload_session_part_from_row)
        .collect())
}

pub async fn cancel_object_upload_session(
    pool: &SqlitePool,
    session_id: &str,
    now: i64,
) -> anyhow::Result<ObjectUploadSessionRecord> {
    let result = sqlx::query(
        r#"
        UPDATE object_upload_sessions
        SET status = 'canceled', canceled_at = ?2
        WHERE id = ?1 AND status = 'prepared'
        "#,
    )
    .bind(session_id)
    .bind(now)
    .execute(pool)
    .await
    .with_context(|| format!("cancel object upload session {session_id}"))?;

    anyhow::ensure!(
        result.rows_affected() == 1,
        "object upload session is not cancelable"
    );
    get_object_upload_session(pool, session_id).await
}

pub async fn publish_object_upload_session(
    pool: &SqlitePool,
    session_id: &str,
    upload: &NewObjectUpload<'_>,
    now: i64,
) -> anyhow::Result<ObjectUploadRecord> {
    let mut tx = pool
        .begin()
        .await
        .with_context(|| format!("begin publish object upload session {session_id}"))?;

    sqlx::query(
        r#"
        INSERT INTO object_uploads (
            id,
            owner_scope,
            backend,
            object_key,
            original_filename,
            content_type,
            size_bytes,
            sha256,
            public_url,
            created_by_actor_id,
            publish_state,
            created_at,
            published_at,
            cleanup_after
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(upload.id)
    .bind(upload.owner_scope)
    .bind(upload.backend)
    .bind(upload.object_key)
    .bind(upload.original_filename)
    .bind(upload.content_type)
    .bind(upload.size_bytes)
    .bind(upload.sha256)
    .bind(upload.public_url)
    .bind(upload.created_by_actor_id)
    .bind(upload.publish_state)
    .bind(upload.created_at)
    .bind(upload.published_at)
    .bind(upload.cleanup_after)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("insert object upload {}", upload.id))?;

    let result = sqlx::query(
        r#"
        UPDATE object_upload_sessions
        SET status = 'completed', completed_at = ?2, published_upload_id = ?3
        WHERE id = ?1 AND status = 'prepared' AND owner_scope = ?4
        "#,
    )
    .bind(session_id)
    .bind(now)
    .bind(upload.id)
    .bind(upload.owner_scope)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("complete object upload session {session_id}"))?;

    anyhow::ensure!(
        result.rows_affected() == 1,
        "object upload session is not completable"
    );

    tx.commit()
        .await
        .with_context(|| format!("commit object upload session {session_id}"))?;

    get_object_upload(pool, upload.id).await
}

pub async fn cleanup_expired_object_upload_sessions(
    pool: &SqlitePool,
    now: i64,
    batch_size: u32,
) -> anyhow::Result<ObjectUploadSessionCleanupResult> {
    let batch_size = i64::from(batch_size.max(1));
    let mut expired_sessions = 0_u64;
    let mut expire_batches = 0_u64;

    loop {
        let expired = sqlx::query(
            r#"
            UPDATE object_upload_sessions
            SET status = 'expired', canceled_at = ?1
            WHERE id IN (
                SELECT id
                FROM object_upload_sessions
                WHERE status = 'prepared' AND expires_at < ?1
                ORDER BY expires_at, id
                LIMIT ?2
            )
            "#,
        )
        .bind(now)
        .bind(batch_size)
        .execute(pool)
        .await
        .with_context(|| format!("cleanup expired object upload sessions before {now}"))?
        .rows_affected();

        if expired == 0 {
            break;
        }
        expired_sessions = expired_sessions.saturating_add(expired);
        expire_batches = expire_batches.saturating_add(1);
    }

    Ok(ObjectUploadSessionCleanupResult {
        cutoff_ts: now,
        expired_sessions,
        expire_batches,
    })
}

fn object_upload_session_from_row(row: &sqlx::sqlite::SqliteRow) -> ObjectUploadSessionRecord {
    ObjectUploadSessionRecord {
        id: row.get("id"),
        owner_scope: row.get("owner_scope"),
        backend: row.get("backend"),
        object_key: row.get("object_key"),
        original_filename: row.get("original_filename"),
        content_type: row.get("content_type"),
        object_kind: row.get("object_kind"),
        expected_size_bytes: row.get("expected_size_bytes"),
        expected_sha256: row.get("expected_sha256"),
        created_by_actor_id: row.get("created_by_actor_id"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        completed_at: row.get("completed_at"),
        canceled_at: row.get("canceled_at"),
        published_upload_id: row.get("published_upload_id"),
    }
}

fn object_upload_session_part_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> ObjectUploadSessionPartRecord {
    ObjectUploadSessionPartRecord {
        session_id: row.get("session_id"),
        part_number: row.get("part_number"),
        object_key: row.get("object_key"),
        size_bytes: row.get("size_bytes"),
        sha256: row.get("sha256"),
        uploaded_at: row.get("uploaded_at"),
    }
}
