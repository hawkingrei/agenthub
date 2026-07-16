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
