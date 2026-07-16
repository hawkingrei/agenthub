use std::path::Path;

use agenthub_object_store::{
    AgentHubObjectStore, ObjectStoreBackend, ObjectStoreSettings, StoredObject,
};
use anyhow::Context;
use chrono::Utc;
use uuid::Uuid;

use super::ActorUploadKind;

pub(super) async fn run_actor_upload(
    actor_id: String,
    owner_scope: String,
    file_path: String,
    content_type: Option<String>,
    display_name: Option<String>,
    kind: ActorUploadKind,
) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
    let config = agenthub_config::AppConfig::load_with_info()?.0;
    let settings = object_store_settings_from_config(&config)?;
    let store = AgentHubObjectStore::from_settings(settings)?;
    let file_name = upload_display_name(&file_path, display_name)?;
    let content_type = infer_content_type(&file_path, content_type)?;
    let bytes = tokio::fs::read(&file_path)
        .await
        .with_context(|| format!("read upload file {file_path:?}"))?;
    let upload_id = Uuid::now_v7().to_string();
    let (object, public_url) = store_actor_upload(
        &store,
        &upload_id,
        &owner_scope,
        &file_name,
        &content_type,
        kind,
        bytes,
    )
    .await?;
    let now = Utc::now().timestamp();
    let db = agenthub_db::init_db().await?;
    let size_bytes = i64::try_from(object.size_bytes)
        .context("uploaded object is too large for SQLite metadata size_bytes")?;
    let upload = agenthub_db::NewObjectUpload {
        id: &upload_id,
        owner_scope: &owner_scope,
        backend: object_store_backend_name(store.backend()),
        object_key: &object.key,
        original_filename: &file_name,
        content_type: &content_type,
        size_bytes,
        sha256: &object.sha256,
        public_url: public_url.as_deref(),
        created_by_actor_id: &actor_id,
        publish_state: "published",
        created_at: now,
        published_at: Some(now),
        cleanup_after: None,
    };
    match agenthub_db::insert_object_upload(&db, &upload).await {
        Ok(record) => Ok(record),
        Err(err) => {
            if let Err(cleanup_err) = store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after upload metadata insert failure"
                );
            }
            Err(err).context("failed to publish upload metadata")
        }
    }
}

fn object_store_backend_name(backend: ObjectStoreBackend) -> &'static str {
    match backend {
        ObjectStoreBackend::Fs => "fs",
        ObjectStoreBackend::S3 => "s3",
    }
}

fn object_store_settings_from_config(
    config: &agenthub_config::AppConfig,
) -> anyhow::Result<ObjectStoreSettings> {
    let backend = match config.object_store_backend().as_str() {
        "fs" => ObjectStoreBackend::Fs,
        "s3" => ObjectStoreBackend::S3,
        other => anyhow::bail!("unsupported object_store.backend: {other}"),
    };
    let root = match config.object_store_root() {
        Some(root) => Some(root),
        None if backend == ObjectStoreBackend::Fs => Some(
            agenthub_config::path_utils::expand_tilde("~/.agenthub/objects"),
        ),
        None => None,
    };
    let access_key_id = config
        .object_store_access_key_id_env()
        .map(|name| read_secret_env(&name, "object_store.access_key_id_env"))
        .transpose()?;
    let secret_access_key = config
        .object_store_secret_access_key_env()
        .map(|name| read_secret_env(&name, "object_store.secret_access_key_env"))
        .transpose()?;

    Ok(ObjectStoreSettings {
        backend,
        root,
        public_base_url: config.object_store_public_base_url(),
        bucket: config.object_store_bucket(),
        endpoint: config.object_store_endpoint(),
        region: config.object_store_region(),
        access_key_id,
        secret_access_key,
        prefix: config.object_store_prefix(),
    })
}

fn read_secret_env(name: &str, config_key: &str) -> anyhow::Result<String> {
    std::env::var(name)
        .with_context(|| format!("{config_key} references missing environment variable {name}"))
}

fn normalize_upload_segment(value: &str, field: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    anyhow::ensure!(!trimmed.is_empty(), "{field} cannot be empty");
    anyhow::ensure!(
        trimmed != "." && trimmed != "..",
        "{field} cannot be '.' or '..'"
    );
    anyhow::ensure!(
        !trimmed.contains('/') && !trimmed.contains('\\'),
        "{field} must be a single path segment"
    );
    Ok(trimmed.to_string())
}

fn upload_display_name(file_path: &str, display_name: Option<String>) -> anyhow::Result<String> {
    match display_name {
        Some(name) => normalize_upload_segment(&name, "--name"),
        None => {
            let file_name = Path::new(file_path)
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| anyhow::anyhow!("upload file path must have a UTF-8 file name"))?;
            normalize_upload_segment(file_name, "file name")
        }
    }
}

fn infer_content_type(file_path: &str, content_type: Option<String>) -> anyhow::Result<String> {
    match content_type {
        Some(value) => {
            let trimmed = value.trim();
            anyhow::ensure!(!trimmed.is_empty(), "--content-type cannot be empty");
            Ok(trimmed.to_ascii_lowercase())
        }
        None => Ok(mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .essence_str()
            .to_string()),
    }
}

async fn store_actor_upload(
    store: &AgentHubObjectStore,
    upload_id: &str,
    owner_scope: &str,
    file_name: &str,
    content_type: &str,
    kind: ActorUploadKind,
    bytes: Vec<u8>,
) -> anyhow::Result<(StoredObject, Option<String>)> {
    match kind {
        ActorUploadKind::Object => {
            let key = format!("uploads/{owner_scope}/{upload_id}/{file_name}");
            let object = store.put_bytes(&key, bytes).await?;
            Ok((object, None))
        }
        ActorUploadKind::Image => {
            let image = store
                .put_image_bytes(owner_scope, upload_id, content_type, bytes)
                .await?;
            Ok((image.object, image.public_url))
        }
    }
}
