use std::{fmt, path::Path, time::Duration};

use agenthub_object_store::{
    AgentHubObjectStore, MultipartUploadPart, ObjectStoreBackend, ObjectStoreSettings,
    PresignedMultipartUploadPart, PresignedObjectWrite, StoredObject,
};
use anyhow::Context;
use chrono::Utc;
use sha2::{Digest, Sha256};
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
        let id = id.trim();
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
    pub expected_size_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ObjectUploadSessionPrepareRequest {
    pub actor_id: String,
    pub owner_scope: ObjectUploadOwnerScope,
    pub file_name: String,
    pub content_type: String,
    pub kind: ObjectUploadKind,
    pub expected_size_bytes: u64,
    pub expected_sha256: Option<String>,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectUploadSessionDirectWrite {
    pub session_id: String,
    pub object_key: String,
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectUploadSessionMultipartUpload {
    pub session_id: String,
    pub object_key: String,
    pub upload_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectUploadSessionMultipartPartWrite {
    pub session_id: String,
    pub object_key: String,
    pub upload_id: String,
    pub part_number: u32,
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectUploadSessionMultipartCompletedPart {
    pub part_number: u32,
    pub etag: String,
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
        let file_name = normalize_upload_file_name(&request.file_name)?;
        let content_type = normalize_upload_content_type(&request.content_type)?;
        let expected_size_bytes = request.expected_size_bytes;
        let expected_sha256 = normalize_expected_sha256(request.expected_sha256.as_deref())?;
        let upload_id = Uuid::now_v7().to_string();
        let owner_scope = request.owner_scope.to_string();
        let (object, public_url) = self
            .store_upload_bytes(
                &upload_id,
                &owner_scope,
                &file_name,
                &content_type,
                request.kind,
                request.bytes,
            )
            .await?;
        if let Err(err) =
            verify_stored_object(&object, expected_size_bytes, expected_sha256.as_deref())
        {
            if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after upload verification failure"
                );
            }
            return Err(err);
        }
        let now = Utc::now().timestamp();
        let size_bytes = i64::try_from(object.size_bytes)
            .context("uploaded object is too large for SQLite metadata size_bytes")?;
        let upload = agenthub_db::NewObjectUpload {
            id: &upload_id,
            owner_scope: &owner_scope,
            backend: object_store_backend_name(self.store.backend()),
            object_key: &object.key,
            original_filename: &file_name,
            content_type: &content_type,
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

    pub async fn prepare_upload_session(
        &self,
        request: ObjectUploadSessionPrepareRequest,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionRecord> {
        let file_name = normalize_upload_file_name(&request.file_name)?;
        let content_type = normalize_upload_content_type(&request.content_type)?;
        anyhow::ensure!(
            request.expected_size_bytes > 0,
            "expected_size_bytes must be greater than zero"
        );
        anyhow::ensure!(
            request.ttl_seconds > 0,
            "ttl_seconds must be greater than zero"
        );
        let expected_size_bytes = i64::try_from(request.expected_size_bytes)
            .context("upload session expected_size_bytes is too large for SQLite")?;
        let expected_sha256 = normalize_expected_sha256(request.expected_sha256.as_deref())?;
        let session_id = Uuid::now_v7().to_string();
        let owner_scope = request.owner_scope.to_string();
        let object_key = self.planned_upload_key(
            &session_id,
            &owner_scope,
            &file_name,
            &content_type,
            request.kind,
        )?;
        let now = Utc::now().timestamp();
        let session = agenthub_db::NewObjectUploadSession {
            id: &session_id,
            owner_scope: &owner_scope,
            backend: object_store_backend_name(self.store.backend()),
            object_key: &object_key,
            original_filename: &file_name,
            content_type: &content_type,
            object_kind: object_upload_kind_name(request.kind),
            expected_size_bytes,
            expected_sha256: expected_sha256.as_deref(),
            created_by_actor_id: &request.actor_id,
            status: "prepared",
            created_at: now,
            expires_at: now + request.ttl_seconds,
            completed_at: None,
            canceled_at: None,
            published_upload_id: None,
        };
        agenthub_db::insert_object_upload_session(&self.db, &session)
            .await
            .context("failed to prepare upload session")
    }

    pub async fn cancel_upload_session(
        &self,
        session_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionRecord> {
        agenthub_db::cancel_object_upload_session(&self.db, session_id, Utc::now().timestamp())
            .await
    }

    pub async fn cancel_upload_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionRecord> {
        let session = agenthub_db::get_object_upload_session(&self.db, session_id).await?;
        let expected_owner_scope = owner_scope.to_string();
        anyhow::ensure!(
            session.owner_scope == expected_owner_scope,
            "object upload session does not belong to owner scope"
        );
        self.cancel_upload_session(session_id).await
    }

    pub async fn complete_upload_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
        bytes: Vec<u8>,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let session = agenthub_db::get_object_upload_session(&self.db, session_id).await?;
        let expected_owner_scope = owner_scope.to_string();
        anyhow::ensure!(
            session.owner_scope == expected_owner_scope,
            "object upload session does not belong to owner scope"
        );
        anyhow::ensure!(
            session.status == "prepared",
            "object upload session is not completable"
        );
        let now = Utc::now().timestamp();
        anyhow::ensure!(session.expires_at >= now, "object upload session expired");

        let object = self
            .store
            .put_stored_key_bytes(&session.object_key, bytes)
            .await?;
        let expected_size_bytes = u64::try_from(session.expected_size_bytes)
            .context("upload session expected_size_bytes is negative")?;
        if let Err(err) = verify_stored_object(
            &object,
            Some(expected_size_bytes),
            session.expected_sha256.as_deref(),
        ) {
            if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after upload session verification failure"
                );
            }
            return Err(err);
        }

        let upload_id = Uuid::now_v7().to_string();
        let public_url = if session.object_kind == "image" {
            self.store.public_url_for_key(&object.key)
        } else {
            None
        };
        let upload = agenthub_db::NewObjectUpload {
            id: &upload_id,
            owner_scope: &session.owner_scope,
            backend: &session.backend,
            object_key: &object.key,
            original_filename: &session.original_filename,
            content_type: &session.content_type,
            size_bytes: session.expected_size_bytes,
            sha256: &object.sha256,
            public_url: public_url.as_deref(),
            created_by_actor_id: &session.created_by_actor_id,
            publish_state: "published",
            created_at: now,
            published_at: Some(now),
            cleanup_after: None,
        };

        match agenthub_db::publish_object_upload_session(&self.db, session_id, &upload, now).await {
            Ok(record) => Ok(record),
            Err(err) => {
                if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                    tracing::warn!(
                        object_key = %object.key,
                        error = %cleanup_err,
                        "failed to delete object after upload session publish failure"
                    );
                }
                Err(err).context("failed to publish upload session metadata")
            }
        }
    }

    pub async fn upload_session_part_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
        part_number: u32,
        bytes: Vec<u8>,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionPartRecord> {
        anyhow::ensure!(part_number > 0, "part_number must be greater than zero");
        anyhow::ensure!(!bytes.is_empty(), "upload session part bytes are required");
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        let object_key = upload_session_part_key(&session.object_key, part_number)?;
        let object = self.store.put_stored_key_bytes(&object_key, bytes).await?;
        let size_bytes = i64::try_from(object.size_bytes)
            .context("upload session part is too large for SQLite metadata size_bytes")?;
        let part_number = i64::from(part_number);
        let now = Utc::now().timestamp();
        let part = agenthub_db::NewObjectUploadSessionPart {
            session_id: &session.id,
            part_number,
            object_key: &object.key,
            size_bytes,
            sha256: &object.sha256,
            uploaded_at: now,
        };
        agenthub_db::upsert_object_upload_session_part(&self.db, &part)
            .await
            .context("failed to record upload session part")
    }

    pub async fn prepare_direct_upload_session_write_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
        requested_expires_in_seconds: Option<u64>,
    ) -> anyhow::Result<ObjectUploadSessionDirectWrite> {
        const DEFAULT_DIRECT_WRITE_EXPIRES_IN_SECONDS: u64 = 15 * 60;
        const MAX_DIRECT_WRITE_EXPIRES_IN_SECONDS: u64 = 60 * 60;

        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        let now = Utc::now().timestamp();
        let remaining_session_seconds = u64::try_from(session.expires_at - now)
            .context("upload session expiration is not in the future")?;
        anyhow::ensure!(
            remaining_session_seconds > 0,
            "object upload session expired"
        );
        let requested = requested_expires_in_seconds
            .unwrap_or(DEFAULT_DIRECT_WRITE_EXPIRES_IN_SECONDS)
            .min(MAX_DIRECT_WRITE_EXPIRES_IN_SECONDS);
        anyhow::ensure!(
            requested > 0,
            "expires_in_seconds must be greater than zero"
        );
        let expires_in_seconds = requested.min(remaining_session_seconds);
        anyhow::ensure!(
            expires_in_seconds > 0,
            "direct upload session write URL would expire immediately"
        );

        let signed = self
            .store
            .presign_stored_key_write(&session.object_key, Duration::from_secs(expires_in_seconds))
            .await?;
        Ok(direct_write_response(&session, signed, now))
    }

    pub async fn initiate_multipart_upload_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
    ) -> anyhow::Result<ObjectUploadSessionMultipartUpload> {
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        let multipart = self
            .store
            .initiate_stored_key_multipart_upload(&session.object_key, Some(&session.content_type))
            .await?;
        Ok(ObjectUploadSessionMultipartUpload {
            session_id: session.id,
            object_key: multipart.key,
            upload_id: multipart.upload_id,
        })
    }

    pub async fn prepare_multipart_upload_session_part_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
        upload_id: &str,
        part_number: u32,
        requested_expires_in_seconds: Option<u64>,
    ) -> anyhow::Result<ObjectUploadSessionMultipartPartWrite> {
        const DEFAULT_MULTIPART_PART_EXPIRES_IN_SECONDS: u64 = 15 * 60;
        const MAX_MULTIPART_PART_EXPIRES_IN_SECONDS: u64 = 60 * 60;

        anyhow::ensure!(!upload_id.trim().is_empty(), "upload_id is required");
        anyhow::ensure!(part_number > 0, "part_number must be greater than zero");
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        let now = Utc::now().timestamp();
        let remaining_session_seconds = u64::try_from(session.expires_at - now)
            .context("upload session expiration is not in the future")?;
        anyhow::ensure!(
            remaining_session_seconds > 0,
            "object upload session expired"
        );
        let requested = requested_expires_in_seconds
            .unwrap_or(DEFAULT_MULTIPART_PART_EXPIRES_IN_SECONDS)
            .min(MAX_MULTIPART_PART_EXPIRES_IN_SECONDS);
        anyhow::ensure!(
            requested > 0,
            "expires_in_seconds must be greater than zero"
        );
        let expires_in_seconds = requested.min(remaining_session_seconds);
        anyhow::ensure!(
            expires_in_seconds > 0,
            "multipart upload part URL would expire immediately"
        );
        let signed = self
            .store
            .presign_stored_key_multipart_upload_part(
                &session.object_key,
                upload_id,
                part_number,
                Duration::from_secs(expires_in_seconds),
            )
            .await?;
        Ok(multipart_part_write_response(
            &session, upload_id, signed, now,
        ))
    }

    pub async fn complete_upload_session_parts_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        let parts = agenthub_db::list_object_upload_session_parts(&self.db, &session.id)
            .await
            .context("failed to list upload session parts")?;
        anyhow::ensure!(!parts.is_empty(), "upload session has no uploaded parts");

        let mut expected_part_number = 1_i64;
        let mut total_size_bytes = 0_u64;
        let mut writer = self.store.stored_key_writer(&session.object_key).await?;
        for part in &parts {
            anyhow::ensure!(
                part.part_number == expected_part_number,
                "upload session parts must be contiguous starting at 1"
            );
            expected_part_number += 1;
            let bytes = self.store.read_stored_key_bytes(&part.object_key).await?;
            let size_bytes = i64::try_from(bytes.len())
                .context("upload session part is too large for SQLite metadata size_bytes")?;
            anyhow::ensure!(
                size_bytes == part.size_bytes,
                "upload session part size mismatch"
            );
            let sha256 = hex_sha256(&bytes);
            anyhow::ensure!(sha256 == part.sha256, "upload session part sha256 mismatch");
            total_size_bytes = total_size_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("uploaded object size overflow"))?;
            writer.write_chunk(bytes).await?;
        }

        let expected_size_bytes = u64::try_from(session.expected_size_bytes)
            .context("upload session expected_size_bytes is negative")?;
        anyhow::ensure!(
            total_size_bytes == expected_size_bytes,
            "uploaded object size mismatch: expected {}, got {}",
            session.expected_size_bytes,
            total_size_bytes
        );

        let object = writer.finish().await?;
        if let Err(err) = verify_stored_object(
            &object,
            Some(expected_size_bytes),
            session.expected_sha256.as_deref(),
        ) {
            if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after upload session parts verification failure"
                );
            }
            return Err(err);
        }
        self.publish_completed_session_object(&session, object, &parts)
            .await
    }

    pub async fn complete_direct_upload_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        let object = self.store.inspect_stored_key(&session.object_key).await?;
        let expected_size_bytes = u64::try_from(session.expected_size_bytes)
            .context("upload session expected_size_bytes is negative")?;
        if let Err(err) = verify_stored_object(
            &object,
            Some(expected_size_bytes),
            session.expected_sha256.as_deref(),
        ) {
            if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after direct upload session verification failure"
                );
            }
            return Err(err);
        }
        self.publish_completed_session_object(&session, object, &[])
            .await
    }

    pub async fn complete_multipart_upload_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
        upload_id: &str,
        parts: Vec<ObjectUploadSessionMultipartCompletedPart>,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        anyhow::ensure!(!upload_id.trim().is_empty(), "upload_id is required");
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        anyhow::ensure!(!parts.is_empty(), "multipart upload parts are required");
        let mut expected_part_number = 1_u32;
        let mut store_parts = Vec::with_capacity(parts.len());
        for part in parts {
            anyhow::ensure!(
                part.part_number == expected_part_number,
                "multipart upload parts must be contiguous starting at 1"
            );
            anyhow::ensure!(
                !part.etag.trim().is_empty(),
                "multipart upload part etag is required"
            );
            expected_part_number = expected_part_number
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("multipart upload part number overflow"))?;
            store_parts.push(MultipartUploadPart {
                part_number: part.part_number,
                etag: part.etag,
            });
        }
        self.store
            .complete_stored_key_multipart_upload(&session.object_key, upload_id, store_parts)
            .await?;

        let object = self.store.inspect_stored_key(&session.object_key).await?;
        let expected_size_bytes = u64::try_from(session.expected_size_bytes)
            .context("upload session expected_size_bytes is negative")?;
        if let Err(err) = verify_stored_object(
            &object,
            Some(expected_size_bytes),
            session.expected_sha256.as_deref(),
        ) {
            if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after multipart upload session verification failure"
                );
            }
            return Err(err);
        }
        self.publish_completed_session_object(&session, object, &[])
            .await
    }

    pub async fn abort_multipart_upload_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
        upload_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionRecord> {
        anyhow::ensure!(!upload_id.trim().is_empty(), "upload_id is required");
        let session = self
            .load_prepared_session_for_scope(owner_scope, session_id)
            .await?;
        self.store
            .abort_stored_key_multipart_upload(&session.object_key, upload_id)
            .await?;
        self.cancel_upload_session(&session.id).await
    }

    pub async fn cleanup_expired_upload_sessions(
        &self,
        batch_size: u32,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionCleanupResult> {
        agenthub_db::cleanup_expired_object_upload_sessions(
            &self.db,
            Utc::now().timestamp(),
            batch_size,
        )
        .await
    }

    async fn load_prepared_session_for_scope(
        &self,
        owner_scope: ObjectUploadOwnerScope,
        session_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadSessionRecord> {
        let session = agenthub_db::get_object_upload_session(&self.db, session_id).await?;
        let expected_owner_scope = owner_scope.to_string();
        anyhow::ensure!(
            session.owner_scope == expected_owner_scope,
            "object upload session does not belong to owner scope"
        );
        anyhow::ensure!(
            session.status == "prepared",
            "object upload session is not completable"
        );
        let now = Utc::now().timestamp();
        anyhow::ensure!(session.expires_at >= now, "object upload session expired");
        Ok(session)
    }

    async fn publish_completed_session_object(
        &self,
        session: &agenthub_db::ObjectUploadSessionRecord,
        object: StoredObject,
        parts: &[agenthub_db::ObjectUploadSessionPartRecord],
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let now = Utc::now().timestamp();
        let upload_id = Uuid::now_v7().to_string();
        let public_url = if session.object_kind == "image" {
            self.store.public_url_for_key(&object.key)
        } else {
            None
        };
        let upload = agenthub_db::NewObjectUpload {
            id: &upload_id,
            owner_scope: &session.owner_scope,
            backend: &session.backend,
            object_key: &object.key,
            original_filename: &session.original_filename,
            content_type: &session.content_type,
            size_bytes: session.expected_size_bytes,
            sha256: &object.sha256,
            public_url: public_url.as_deref(),
            created_by_actor_id: &session.created_by_actor_id,
            publish_state: "published",
            created_at: now,
            published_at: Some(now),
            cleanup_after: None,
        };

        match agenthub_db::publish_object_upload_session(&self.db, &session.id, &upload, now).await
        {
            Ok(record) => {
                for part in parts {
                    if let Err(err) = self.store.delete_stored_key(&part.object_key).await {
                        tracing::warn!(
                            object_key = %part.object_key,
                            error = %err,
                            "failed to delete upload session part after publish"
                        );
                    }
                }
                Ok(record)
            }
            Err(err) => {
                if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                    tracing::warn!(
                        object_key = %object.key,
                        error = %cleanup_err,
                        "failed to delete object after upload session publish failure"
                    );
                }
                Err(err).context("failed to publish upload session metadata")
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

    fn planned_upload_key(
        &self,
        upload_id: &str,
        owner_scope: &str,
        file_name: &str,
        content_type: &str,
        kind: ObjectUploadKind,
    ) -> anyhow::Result<String> {
        match kind {
            ObjectUploadKind::Object => self
                .store
                .scoped_key(&format!("uploads/{owner_scope}/{upload_id}/{file_name}")),
            ObjectUploadKind::Image => {
                let (key, _) = self
                    .store
                    .scoped_image_key(owner_scope, upload_id, content_type)?;
                self.store.scoped_key(&key)
            }
        }
    }
}

fn upload_session_part_key(session_object_key: &str, part_number: u32) -> anyhow::Result<String> {
    let session_object_key = agenthub_object_store::normalize_object_key(session_object_key)?;
    Ok(format!("{session_object_key}.parts/{part_number:08}"))
}

fn direct_write_response(
    session: &agenthub_db::ObjectUploadSessionRecord,
    signed: PresignedObjectWrite,
    now: i64,
) -> ObjectUploadSessionDirectWrite {
    ObjectUploadSessionDirectWrite {
        session_id: session.id.clone(),
        object_key: session.object_key.clone(),
        method: signed.method,
        uri: signed.uri,
        headers: signed.headers,
        expires_at: now + i64::try_from(signed.expires_in_seconds).unwrap_or(i64::MAX),
    }
}

fn multipart_part_write_response(
    session: &agenthub_db::ObjectUploadSessionRecord,
    upload_id: &str,
    signed: PresignedMultipartUploadPart,
    now: i64,
) -> ObjectUploadSessionMultipartPartWrite {
    ObjectUploadSessionMultipartPartWrite {
        session_id: session.id.clone(),
        object_key: session.object_key.clone(),
        upload_id: upload_id.to_string(),
        part_number: signed.part_number,
        method: signed.method,
        uri: signed.uri,
        headers: signed.headers,
        expires_at: now + i64::try_from(signed.expires_in_seconds).unwrap_or(i64::MAX),
    }
}

fn verify_stored_object(
    object: &StoredObject,
    expected_size_bytes: Option<u64>,
    expected_sha256: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(expected_size_bytes) = expected_size_bytes {
        anyhow::ensure!(
            object.size_bytes == expected_size_bytes,
            "uploaded object size mismatch: expected {expected_size_bytes}, got {}",
            object.size_bytes
        );
    }
    if let Some(expected_sha256) = normalize_expected_sha256(expected_sha256)? {
        anyhow::ensure!(
            object.sha256 == expected_sha256,
            "uploaded object sha256 mismatch"
        );
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_digest(digest.as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn normalize_expected_sha256(expected_sha256: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(expected_sha256) = expected_sha256 else {
        return Ok(None);
    };
    let expected_sha256 = expected_sha256.trim().to_ascii_lowercase();
    anyhow::ensure!(
        expected_sha256.len() == 64 && expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "expected_sha256 must be a lowercase or uppercase SHA-256 hex digest"
    );
    Ok(Some(expected_sha256))
}

fn object_store_backend_name(backend: ObjectStoreBackend) -> &'static str {
    match backend {
        ObjectStoreBackend::Fs => "fs",
        ObjectStoreBackend::S3 => "s3",
    }
}

fn object_upload_kind_name(kind: ObjectUploadKind) -> &'static str {
    match kind {
        ObjectUploadKind::Object => "object",
        ObjectUploadKind::Image => "image",
    }
}

fn normalize_upload_file_name(file_name: &str) -> anyhow::Result<String> {
    let file_name = file_name.trim();
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        anyhow::bail!("file_name must be a single non-empty path segment");
    }
    if file_name.contains('/') || file_name.contains('\\') {
        anyhow::bail!("file_name must be a single path segment");
    }
    Ok(file_name.to_string())
}

fn normalize_upload_content_type(content_type: &str) -> anyhow::Result<String> {
    let content_type = content_type.trim();
    anyhow::ensure!(!content_type.is_empty(), "content_type is required");
    Ok(content_type.to_ascii_lowercase())
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
        None if backend == ObjectStoreBackend::Fs => Some(default_object_store_fs_root()?),
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

fn default_object_store_fs_root() -> anyhow::Result<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let current_dir = std::env::current_dir().context("resolve current directory")?;
    Ok(default_object_store_fs_root_from_home(
        Path::new(&home),
        &current_dir,
    ))
}

fn default_object_store_fs_root_from_home(home: &Path, current_dir: &Path) -> String {
    let root = home.join(".agenthub/objects");
    if root.is_absolute() {
        root.to_string_lossy().to_string()
    } else {
        current_dir.join(root).to_string_lossy().to_string()
    }
}

fn read_secret_env(name: &str, config_key: &str) -> anyhow::Result<String> {
    std::env::var(name)
        .with_context(|| format!("{config_key} references missing environment variable {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

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
            ObjectUploadOwnerScope::parse("agents/ agent-1 ")
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

    #[test]
    fn upload_file_name_rejects_nested_or_empty_values() {
        assert_eq!(
            normalize_upload_file_name(" report.json ").expect("normalize file name"),
            "report.json"
        );
        for value in [
            "",
            " ",
            ".",
            "..",
            "nested/report.json",
            "nested\\report.json",
        ] {
            assert!(
                normalize_upload_file_name(value).is_err(),
                "file name should be rejected: {value:?}"
            );
        }
    }

    #[test]
    fn default_object_store_root_absolutizes_relative_home() {
        let root = default_object_store_fs_root_from_home(
            Path::new("target/test-home"),
            Path::new("/workspace/agenthub"),
        );
        assert_eq!(
            root,
            "/workspace/agenthub/target/test-home/.agenthub/objects"
        );
    }

    #[test]
    fn expected_sha256_normalization_rejects_malformed_values() {
        assert_eq!(
            normalize_expected_sha256(Some(
                " 3A6EB0790F39AC87C94F3856B2DD2C5D110E6811602261A9A923D3BB23ADC8B7 "
            ))
            .expect("normalize expected sha256"),
            Some("3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7".to_string())
        );
        for value in [
            "",
            "abc",
            "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b",
            "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8xz",
        ] {
            assert!(
                normalize_expected_sha256(Some(value)).is_err(),
                "checksum should be rejected: {value:?}"
            );
        }
    }

    #[tokio::test]
    async fn stored_object_verification_rejects_size_or_checksum_mismatch() {
        let object = StoredObject {
            key: "uploads/teams/team-1/upload-1/report.txt".to_string(),
            size_bytes: 4,
            sha256: "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7".to_string(),
        };
        let request = ObjectUploadRequest {
            actor_id: "user-1".to_string(),
            owner_scope: ObjectUploadOwnerScope::Team("team-1".to_string()),
            file_name: "report.txt".to_string(),
            content_type: "text/plain".to_string(),
            kind: ObjectUploadKind::Object,
            expected_size_bytes: Some(5),
            expected_sha256: None,
            bytes: Vec::new(),
        };
        assert!(
            verify_stored_object(
                &object,
                request.expected_size_bytes,
                request.expected_sha256.as_deref()
            )
            .is_err()
        );

        let request = ObjectUploadRequest {
            expected_size_bytes: Some(4),
            expected_sha256: Some(
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ),
            ..request
        };
        assert!(
            verify_stored_object(
                &object,
                request.expected_size_bytes,
                request.expected_sha256.as_deref()
            )
            .is_err()
        );

        let request = ObjectUploadRequest {
            expected_sha256: Some(object.sha256.to_ascii_uppercase()),
            ..request
        };
        verify_stored_object(
            &object,
            request.expected_size_bytes,
            request.expected_sha256.as_deref(),
        )
        .expect("uppercase checksum should normalize");
    }

    #[tokio::test]
    async fn prepare_upload_session_persists_planned_object_key_and_cancels_once() {
        let db = test_upload_db().await;
        let dir = unique_temp_dir("upload-session-object");
        std::fs::create_dir_all(&dir).expect("create object store root");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        let service = ObjectUploadService::new(db.clone(), store);
        let expected_sha256 = "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7";

        let session = service
            .prepare_upload_session(ObjectUploadSessionPrepareRequest {
                actor_id: "user:user-1".to_string(),
                owner_scope: ObjectUploadOwnerScope::Team("team-1".to_string()),
                file_name: " report.txt ".to_string(),
                content_type: "text/plain".to_string(),
                kind: ObjectUploadKind::Object,
                expected_size_bytes: 42,
                expected_sha256: Some(expected_sha256.to_ascii_uppercase()),
                ttl_seconds: 900,
            })
            .await
            .expect("prepare upload session");

        assert_eq!(session.owner_scope, "teams/team-1");
        assert_eq!(session.backend, "fs");
        assert_eq!(session.original_filename, "report.txt");
        assert_eq!(session.object_kind, "object");
        assert_eq!(session.expected_size_bytes, 42);
        assert_eq!(session.expected_sha256.as_deref(), Some(expected_sha256));
        assert_eq!(session.status, "prepared");
        assert!(session.expires_at > session.created_at);
        assert!(
            session
                .object_key
                .starts_with("agenthub/local/uploads/teams/team-1/")
        );
        assert!(session.object_key.ends_with("/report.txt"));

        let canceled = service
            .cancel_upload_session(&session.id)
            .await
            .expect("cancel upload session");
        assert_eq!(canceled.status, "canceled");
        assert!(canceled.canceled_at.is_some());
        let err = service
            .cancel_upload_session(&session.id)
            .await
            .expect_err("second cancel should fail");
        assert!(err.to_string().contains("not cancelable"));

        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn prepare_upload_session_uses_image_key_rules() {
        let db = test_upload_db().await;
        let dir = unique_temp_dir("upload-session-image");
        std::fs::create_dir_all(&dir).expect("create object store root");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        let service = ObjectUploadService::new(db.clone(), store);

        let session = service
            .prepare_upload_session(ObjectUploadSessionPrepareRequest {
                actor_id: "user:user-1".to_string(),
                owner_scope: ObjectUploadOwnerScope::Agent("agent-1".to_string()),
                file_name: "ignored-name.png".to_string(),
                content_type: " IMAGE/PNG ".to_string(),
                kind: ObjectUploadKind::Image,
                expected_size_bytes: 4,
                expected_sha256: None,
                ttl_seconds: 60,
            })
            .await
            .expect("prepare image upload session");

        assert_eq!(session.content_type, "image/png");
        assert_eq!(session.object_kind, "image");
        assert!(
            session
                .object_key
                .starts_with("agenthub/local/images/agents/agent-1/")
        );
        assert!(session.object_key.ends_with(".png"));

        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn complete_upload_session_writes_planned_key_and_publishes_metadata() {
        let db = test_upload_db().await;
        let dir = unique_temp_dir("upload-session-complete");
        std::fs::create_dir_all(&dir).expect("create object store root");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            public_base_url: Some("https://img.example.test".to_string()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        let service = ObjectUploadService::new(db.clone(), store);
        let bytes = vec![1_u8, 2, 3, 4];
        let expected_sha256 = hex_sha256_for_test(&bytes);

        let session = service
            .prepare_upload_session(ObjectUploadSessionPrepareRequest {
                actor_id: "user:user-1".to_string(),
                owner_scope: ObjectUploadOwnerScope::Agent("agent-1".to_string()),
                file_name: "screenshot.png".to_string(),
                content_type: "image/png".to_string(),
                kind: ObjectUploadKind::Image,
                expected_size_bytes: bytes.len() as u64,
                expected_sha256: Some(expected_sha256.clone()),
                ttl_seconds: 60,
            })
            .await
            .expect("prepare upload session");

        let upload = service
            .complete_upload_session_for_scope(
                ObjectUploadOwnerScope::Agent("agent-1".to_string()),
                &session.id,
                bytes,
            )
            .await
            .expect("complete upload session");

        assert_eq!(upload.owner_scope, "agents/agent-1");
        assert_eq!(upload.object_key, session.object_key);
        assert_eq!(upload.content_type, "image/png");
        assert_eq!(upload.size_bytes, 4);
        assert_eq!(upload.sha256, expected_sha256);
        assert_eq!(upload.publish_state, "published");
        assert_eq!(
            upload.public_url.as_deref(),
            Some(format!("https://img.example.test/{}", upload.object_key).as_str())
        );
        let completed = agenthub_db::get_object_upload_session(&db, &session.id)
            .await
            .expect("load completed session");
        assert_eq!(completed.status, "completed");
        assert_eq!(
            completed.published_upload_id.as_deref(),
            Some(upload.id.as_str())
        );

        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn upload_session_parts_complete_resumable_proxy_upload() {
        let db = test_upload_db().await;
        let dir = unique_temp_dir("upload-session-parts-complete");
        std::fs::create_dir_all(&dir).expect("create object store root");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        let service = ObjectUploadService::new(db.clone(), store.clone());
        let bytes = b"hello world".to_vec();
        let expected_sha256 = hex_sha256_for_test(&bytes);

        let session = service
            .prepare_upload_session(ObjectUploadSessionPrepareRequest {
                actor_id: "user:user-1".to_string(),
                owner_scope: ObjectUploadOwnerScope::Team("team-1".to_string()),
                file_name: "report.txt".to_string(),
                content_type: "text/plain".to_string(),
                kind: ObjectUploadKind::Object,
                expected_size_bytes: bytes.len() as u64,
                expected_sha256: Some(expected_sha256.clone()),
                ttl_seconds: 60,
            })
            .await
            .expect("prepare upload session");

        let first_part = service
            .upload_session_part_for_scope(
                ObjectUploadOwnerScope::Team("team-1".to_string()),
                &session.id,
                1,
                b"hello ".to_vec(),
            )
            .await
            .expect("upload first part");
        let replaced_part = service
            .upload_session_part_for_scope(
                ObjectUploadOwnerScope::Team("team-1".to_string()),
                &session.id,
                1,
                b"hello ".to_vec(),
            )
            .await
            .expect("retry first part");
        assert_eq!(replaced_part.object_key, first_part.object_key);
        let second_part = service
            .upload_session_part_for_scope(
                ObjectUploadOwnerScope::Team("team-1".to_string()),
                &session.id,
                2,
                b"world".to_vec(),
            )
            .await
            .expect("upload second part");

        let upload = service
            .complete_upload_session_parts_for_scope(
                ObjectUploadOwnerScope::Team("team-1".to_string()),
                &session.id,
            )
            .await
            .expect("complete upload session from parts");

        assert_eq!(upload.owner_scope, "teams/team-1");
        assert_eq!(upload.object_key, session.object_key);
        assert_eq!(upload.size_bytes, bytes.len() as i64);
        assert_eq!(upload.sha256, expected_sha256);
        assert_eq!(
            store
                .read_stored_key_bytes(&upload.object_key)
                .await
                .unwrap(),
            bytes
        );
        assert!(
            !store
                .exists_stored_key(&first_part.object_key)
                .await
                .unwrap()
        );
        assert!(
            !store
                .exists_stored_key(&second_part.object_key)
                .await
                .unwrap()
        );
        let completed = agenthub_db::get_object_upload_session(&db, &session.id)
            .await
            .expect("load completed session");
        assert_eq!(completed.status, "completed");

        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn complete_direct_upload_session_verifies_planned_key_and_publishes_metadata() {
        let db = test_upload_db().await;
        let dir = unique_temp_dir("upload-session-direct-complete");
        std::fs::create_dir_all(&dir).expect("create object store root");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        let service = ObjectUploadService::new(db.clone(), store.clone());
        let bytes = b"direct object".to_vec();
        let expected_sha256 = hex_sha256_for_test(&bytes);

        let session = service
            .prepare_upload_session(ObjectUploadSessionPrepareRequest {
                actor_id: "user:user-1".to_string(),
                owner_scope: ObjectUploadOwnerScope::Team("team-1".to_string()),
                file_name: "report.txt".to_string(),
                content_type: "text/plain".to_string(),
                kind: ObjectUploadKind::Object,
                expected_size_bytes: bytes.len() as u64,
                expected_sha256: Some(expected_sha256.clone()),
                ttl_seconds: 60,
            })
            .await
            .expect("prepare upload session");
        store
            .put_stored_key_bytes(&session.object_key, bytes)
            .await
            .expect("write object through direct transport");

        let upload = service
            .complete_direct_upload_session_for_scope(
                ObjectUploadOwnerScope::Team("team-1".to_string()),
                &session.id,
            )
            .await
            .expect("complete direct upload session");

        assert_eq!(upload.owner_scope, "teams/team-1");
        assert_eq!(upload.object_key, session.object_key);
        assert_eq!(upload.size_bytes, session.expected_size_bytes);
        assert_eq!(upload.sha256, expected_sha256);
        let completed = agenthub_db::get_object_upload_session(&db, &session.id)
            .await
            .expect("load completed session");
        assert_eq!(completed.status, "completed");
        assert_eq!(
            completed.published_upload_id.as_deref(),
            Some(upload.id.as_str())
        );

        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cleanup_expired_upload_sessions_marks_prepared_sessions_expired() {
        let db = test_upload_db().await;
        let dir = unique_temp_dir("upload-session-expiry-cleanup");
        std::fs::create_dir_all(&dir).expect("create object store root");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        let service = ObjectUploadService::new(db.clone(), store);
        agenthub_db::insert_object_upload_session(
            &db,
            &agenthub_db::NewObjectUploadSession {
                id: "expired-session",
                owner_scope: "teams/team-1",
                backend: "fs",
                object_key: "agenthub/local/uploads/teams/team-1/expired-session/report.txt",
                original_filename: "report.txt",
                content_type: "text/plain",
                object_kind: "object",
                expected_size_bytes: 4,
                expected_sha256: None,
                created_by_actor_id: "user:user-1",
                status: "prepared",
                created_at: 1,
                expires_at: 2,
                completed_at: None,
                canceled_at: None,
                published_upload_id: None,
            },
        )
        .await
        .expect("insert expired upload session");

        let result = service
            .cleanup_expired_upload_sessions(100)
            .await
            .expect("cleanup expired upload sessions");

        assert_eq!(result.expired_sessions, 1);
        let session = agenthub_db::get_object_upload_session(&db, "expired-session")
            .await
            .expect("load expired upload session");
        assert_eq!(session.status, "expired");
        assert!(session.canceled_at.is_some());

        db.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn test_upload_db() -> sqlx::SqlitePool {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory db");
        sqlx::query(
            r#"
            CREATE TABLE object_uploads (
                id TEXT PRIMARY KEY,
                owner_scope TEXT NOT NULL,
                backend TEXT NOT NULL,
                object_key TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                sha256 TEXT NOT NULL,
                public_url TEXT,
                created_by_actor_id TEXT NOT NULL,
                publish_state TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                published_at INTEGER,
                cleanup_after INTEGER
            );
            "#,
        )
        .execute(&db)
        .await
        .expect("create object_uploads table");
        sqlx::query(
            r#"
            CREATE TABLE object_upload_sessions (
                id TEXT PRIMARY KEY,
                owner_scope TEXT NOT NULL,
                backend TEXT NOT NULL,
                object_key TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                object_kind TEXT NOT NULL,
                expected_size_bytes INTEGER NOT NULL,
                expected_sha256 TEXT,
                created_by_actor_id TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                completed_at INTEGER,
                canceled_at INTEGER,
                published_upload_id TEXT
            );
            "#,
        )
        .execute(&db)
        .await
        .expect("create object_upload_sessions table");
        sqlx::query(
            r#"
            CREATE TABLE object_upload_session_parts (
                session_id TEXT NOT NULL,
                part_number INTEGER NOT NULL,
                object_key TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                sha256 TEXT NOT NULL,
                uploaded_at INTEGER NOT NULL,
                PRIMARY KEY(session_id, part_number)
            );
            "#,
        )
        .execute(&db)
        .await
        .expect("create object_upload_session_parts table");
        db
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("agenthub-{label}-{}", Uuid::new_v4()))
    }

    fn hex_sha256_for_test(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};

        let digest = Sha256::digest(bytes);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
