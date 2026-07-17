use std::path::Path;

use crate::object_upload::{
    ObjectUploadKind, ObjectUploadOwnerScope, ObjectUploadRequest, ObjectUploadService,
};

const MAX_UPLOAD_FILE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
enum UploadError {
    #[error("read upload metadata {path:?}: {source}")]
    ReadMetadata {
        path: String,
        source: std::io::Error,
    },
    #[error("upload file is too large: {size_bytes} bytes exceeds {limit_bytes} bytes")]
    FileTooLarge { size_bytes: u64, limit_bytes: u64 },
    #[error("read upload file {path:?}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
}

pub(super) async fn run_actor_upload(
    actor_id: String,
    owner_scope: ObjectUploadOwnerScope,
    file_path: String,
    content_type: Option<String>,
    display_name: Option<String>,
    kind: ObjectUploadKind,
) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
    let config = agenthub_config::AppConfig::load_with_info()?.0;
    let file_name = upload_display_name(&file_path, display_name)?;
    let content_type = infer_content_type(&file_path, content_type)?;
    let bytes = read_upload_file(&file_path).await?;
    let db = agenthub_db::init_db().await?;
    let service = ObjectUploadService::from_config(db, &config)?;
    service
        .upload(ObjectUploadRequest {
            actor_id,
            owner_scope,
            file_name,
            content_type,
            kind,
            bytes,
        })
        .await
}

async fn read_upload_file(file_path: &str) -> Result<Vec<u8>, UploadError> {
    let metadata =
        tokio::fs::metadata(file_path)
            .await
            .map_err(|source| UploadError::ReadMetadata {
                path: file_path.to_string(),
                source,
            })?;
    let size_bytes = metadata.len();
    if size_bytes > MAX_UPLOAD_FILE_BYTES {
        return Err(UploadError::FileTooLarge {
            size_bytes,
            limit_bytes: MAX_UPLOAD_FILE_BYTES,
        });
    }
    tokio::fs::read(file_path)
        .await
        .map_err(|source| UploadError::ReadFile {
            path: file_path.to_string(),
            source,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_upload_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("agenthub-upload-{name}-{}", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn read_upload_file_reads_file_under_limit() {
        let path = temp_upload_path("small");
        tokio::fs::write(&path, b"hello")
            .await
            .expect("write upload fixture");

        let bytes = read_upload_file(path.to_str().expect("utf-8 path"))
            .await
            .expect("read upload file");

        assert_eq!(bytes, b"hello");
        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn read_upload_file_rejects_file_over_limit() {
        let path = temp_upload_path("large");
        let file = std::fs::File::create(&path).expect("create upload fixture");
        file.set_len(MAX_UPLOAD_FILE_BYTES + 1)
            .expect("size upload fixture");
        drop(file);

        let err = read_upload_file(path.to_str().expect("utf-8 path"))
            .await
            .expect_err("reject oversized upload");

        match err {
            UploadError::FileTooLarge {
                size_bytes,
                limit_bytes,
            } => {
                assert_eq!(size_bytes, MAX_UPLOAD_FILE_BYTES + 1);
                assert_eq!(limit_bytes, MAX_UPLOAD_FILE_BYTES);
            }
            other => panic!("expected file-too-large error, got {other:?}"),
        }
        let _ = tokio::fs::remove_file(path).await;
    }
}
