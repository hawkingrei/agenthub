use std::fmt;

use agenthub_object_store::{
    AgentHubObjectStore, ObjectStoreBackend, ObjectStoreSettings, StoredObject,
};
use anyhow::Context;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectUploadKind {
    Object,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectUploadOwnerScope {
    Team(String),
    Task(String),
    Agent(String),
}

impl ObjectUploadOwnerScope {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let trimmed = value.trim();
        let Some((kind, id)) = trimmed.split_once('/') else {
            anyhow::bail!(
                "owner scope must be teams/<team_id>, tasks/<task_id>, or agents/<agent_id>"
            );
        };
        if id.is_empty() || id.contains('/') || id.contains('\\') || id == "." || id == ".." {
            anyhow::bail!("owner scope id must be a single non-empty path segment");
        }
        match kind {
            "teams" => Ok(Self::Team(id.to_string())),
            "tasks" => Ok(Self::Task(id.to_string())),
            "agents" => Ok(Self::Agent(id.to_string())),
            _ => anyhow::bail!(
                "owner scope must be teams/<team_id>, tasks/<task_id>, or agents/<agent_id>"
            ),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Team(value) | Self::Task(value) | Self::Agent(value) => value,
        }
    }

    fn prefix(&self) -> &'static str {
        match self {
            Self::Team(_) => "teams",
            Self::Task(_) => "tasks",
            Self::Agent(_) => "agents",
        }
    }
}

impl fmt::Display for ObjectUploadOwnerScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.prefix(), self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ObjectUploadRequest {
    pub actor_id: String,
    pub owner_scope: ObjectUploadOwnerScope,
    pub file_name: String,
    pub content_type: String,
    pub kind: ObjectUploadKind,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ObjectUploadService {
    db: SqlitePool,
    store: AgentHubObjectStore,
}

impl ObjectUploadService {
    pub fn new(db: SqlitePool, store: AgentHubObjectStore) -> Self {
        Self { db, store }
    }

    pub fn from_config(
        db: SqlitePool,
        config: &agenthub_config::AppConfig,
    ) -> anyhow::Result<Self> {
        let settings = object_store_settings_from_config(config)?;
        Ok(Self::new(db, AgentHubObjectStore::from_settings(settings)?))
    }

    pub async fn upload(
        &self,
        request: ObjectUploadRequest,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let upload_id = Uuid::now_v7().to_string();
        let owner_scope = request.owner_scope.to_string();
        let (object, public_url) = self
            .store_upload_bytes(
                &upload_id,
                &owner_scope,
                &request.file_name,
                &request.content_type,
                request.kind,
                request.bytes,
            )
            .await?;
        let now = Utc::now().timestamp();
        let size_bytes = i64::try_from(object.size_bytes)
            .context("uploaded object is too large for SQLite metadata size_bytes")?;
        let upload = agenthub_db::NewObjectUpload {
            id: &upload_id,
            owner_scope: &owner_scope,
            backend: object_store_backend_name(self.store.backend()),
            object_key: &object.key,
            original_filename: &request.file_name,
            content_type: &request.content_type,
            size_bytes,
            sha256: &object.sha256,
            public_url: public_url.as_deref(),
            created_by_actor_id: &request.actor_id,
            publish_state: "published",
            created_at: now,
            published_at: Some(now),
            cleanup_after: None,
        };
        match agenthub_db::insert_object_upload(&self.db, &upload).await {
            Ok(record) => Ok(record),
            Err(err) => {
                if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
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

    async fn store_upload_bytes(
        &self,
        upload_id: &str,
        owner_scope: &str,
        file_name: &str,
        content_type: &str,
        kind: ObjectUploadKind,
        bytes: Vec<u8>,
    ) -> anyhow::Result<(StoredObject, Option<String>)> {
        match kind {
            ObjectUploadKind::Object => {
                let key = format!("uploads/{owner_scope}/{upload_id}/{file_name}");
                let object = self.store.put_bytes(&key, bytes).await?;
                Ok((object, None))
            }
            ObjectUploadKind::Image => {
                let image = self
                    .store
                    .put_image_bytes(owner_scope, upload_id, content_type, bytes)
                    .await?;
                Ok((image.object, image.public_url))
            }
        }
    }
}

fn object_store_backend_name(backend: ObjectStoreBackend) -> &'static str {
    match backend {
        ObjectStoreBackend::Fs => "fs",
        ObjectStoreBackend::S3 => "s3",
    }
}

pub fn object_store_settings_from_config(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_scope_parses_supported_scopes() {
        assert_eq!(
            ObjectUploadOwnerScope::parse("teams/team-1")
                .expect("parse team scope")
                .to_string(),
            "teams/team-1"
        );
        assert_eq!(
            ObjectUploadOwnerScope::parse("tasks/task-1")
                .expect("parse task scope")
                .to_string(),
            "tasks/task-1"
        );
        assert_eq!(
            ObjectUploadOwnerScope::parse("agents/agent-1")
                .expect("parse agent scope")
                .to_string(),
            "agents/agent-1"
        );
    }

    #[test]
    fn owner_scope_rejects_ambiguous_or_nested_values() {
        for value in ["team-1", "channels/main", "teams/", "teams/a/b", "teams/.."] {
            assert!(
                ObjectUploadOwnerScope::parse(value).is_err(),
                "scope should be rejected: {value}"
            );
        }
    }
}
