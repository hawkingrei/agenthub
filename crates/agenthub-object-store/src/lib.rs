use std::path::Path;

use anyhow::{Context, anyhow};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use opendal::{Operator, services::Fs};
use sha2::{Digest, Sha256};

#[cfg(feature = "s3")]
use opendal::services::S3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectStoreBackend {
    Fs,
    S3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreSettings {
    pub backend: ObjectStoreBackend,
    pub root: Option<String>,
    pub public_base_url: Option<String>,
    pub bucket: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub prefix: Option<String>,
}

impl Default for ObjectStoreSettings {
    fn default() -> Self {
        Self {
            backend: ObjectStoreBackend::Fs,
            root: None,
            public_base_url: None,
            bucket: None,
            endpoint: None,
            region: None,
            access_key_id: None,
            secret_access_key: None,
            prefix: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObject {
    pub key: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedImageObject {
    pub object: StoredObject,
    pub content_type: String,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentHubObjectStore {
    operator: Operator,
    prefix: Option<String>,
    public_base_url: Option<String>,
    backend: ObjectStoreBackend,
}

impl AgentHubObjectStore {
    pub fn from_settings(settings: ObjectStoreSettings) -> anyhow::Result<Self> {
        let prefix = normalize_prefix(settings.prefix.as_deref())?;
        let public_base_url = normalize_public_base_url(settings.public_base_url.as_deref());
        match settings.backend {
            ObjectStoreBackend::Fs => {
                let root = settings
                    .root
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow!("object store fs root is required"))?;
                let root_path = Path::new(root);
                if !root_path.is_absolute() {
                    anyhow::bail!("object store fs root must be absolute: {root}");
                }
                std::fs::create_dir_all(root_path)
                    .with_context(|| format!("create object store fs root {root:?}"))?;

                let builder = Fs::default().root(root);
                let operator = Operator::new(builder)?;
                Ok(Self {
                    operator,
                    prefix,
                    public_base_url,
                    backend: ObjectStoreBackend::Fs,
                })
            }
            ObjectStoreBackend::S3 => Self::from_s3_settings(settings, prefix, public_base_url),
        }
    }

    pub fn backend(&self) -> ObjectStoreBackend {
        self.backend
    }

    pub async fn put_bytes(
        &self,
        key: &str,
        bytes: impl Into<Vec<u8>>,
    ) -> anyhow::Result<StoredObject> {
        let bytes = bytes.into();
        let size_bytes = bytes.len() as u64;
        let sha256 = hex_sha256(&bytes);
        let normalized_key = self.scoped_key(key)?;
        self.operator
            .write(&normalized_key, bytes)
            .await
            .with_context(|| format!("write object {normalized_key:?}"))?;
        Ok(StoredObject {
            key: normalized_key,
            size_bytes,
            sha256,
        })
    }

    pub async fn put_byte_stream<S, E>(
        &self,
        key: &str,
        mut chunks: S,
    ) -> anyhow::Result<StoredObject>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: std::error::Error + Send + Sync + 'static,
    {
        let normalized_key = self.scoped_key(key)?;
        let mut writer = self
            .operator
            .writer(&normalized_key)
            .await
            .with_context(|| format!("create object writer {normalized_key:?}"))?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;

        while let Some(chunk) = chunks.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    abort_writer(&mut writer, &normalized_key).await;
                    return Err(err)
                        .with_context(|| format!("read object chunk {normalized_key:?}"));
                }
            };
            size_bytes = size_bytes
                .checked_add(chunk.len() as u64)
                .context("streamed object is too large")?;
            hasher.update(&chunk);
            if let Err(err) = writer.write(chunk).await {
                abort_writer(&mut writer, &normalized_key).await;
                return Err(err).with_context(|| format!("write object chunk {normalized_key:?}"));
            }
        }

        if let Err(err) = writer.close().await {
            abort_writer(&mut writer, &normalized_key).await;
            return Err(err).with_context(|| format!("close object writer {normalized_key:?}"));
        }

        Ok(StoredObject {
            key: normalized_key,
            size_bytes,
            sha256: hex_sha256_digest(hasher.finalize().as_slice()),
        })
    }

    pub async fn read_bytes(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let normalized_key = self.scoped_key(key)?;
        let bytes = self
            .operator
            .read(&normalized_key)
            .await
            .with_context(|| format!("read object {normalized_key:?}"))?;
        Ok(bytes.to_vec())
    }

    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let normalized_key = self.scoped_key(key)?;
        self.operator
            .delete(&normalized_key)
            .await
            .with_context(|| format!("delete object {normalized_key:?}"))?;
        Ok(())
    }

    pub async fn delete_stored_object(&self, object: &StoredObject) -> anyhow::Result<()> {
        self.operator
            .delete(&object.key)
            .await
            .with_context(|| format!("delete object {:?}", object.key))?;
        Ok(())
    }

    pub async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let normalized_key = self.scoped_key(key)?;
        self.operator
            .exists(&normalized_key)
            .await
            .with_context(|| format!("stat object {normalized_key:?}"))
    }

    pub fn scoped_key(&self, key: &str) -> anyhow::Result<String> {
        let key = normalize_object_key(key)?;
        Ok(match self.prefix.as_deref() {
            Some(prefix) => format!("{prefix}/{key}"),
            None => key,
        })
    }

    pub async fn put_image_bytes(
        &self,
        scope: &str,
        image_id: &str,
        content_type: &str,
        bytes: impl Into<Vec<u8>>,
    ) -> anyhow::Result<HostedImageObject> {
        let content_type = normalize_image_content_type(content_type)?;
        let scope = normalize_object_key(scope)?;
        let image_id = normalize_object_segment(image_id, "image_id")?;
        let extension = image_extension(&content_type)?;
        let key = format!("images/{scope}/{image_id}.{extension}");
        let object = self.put_bytes(&key, bytes).await?;
        let public_url = self.public_url_for_key(&object.key);
        Ok(HostedImageObject {
            object,
            content_type,
            public_url,
        })
    }

    pub fn public_url_for_key(&self, key: &str) -> Option<String> {
        self.public_base_url
            .as_ref()
            .map(|base_url| format!("{base_url}/{}", key.trim_start_matches('/')))
    }

    #[cfg(feature = "s3")]
    fn from_s3_settings(
        settings: ObjectStoreSettings,
        prefix: Option<String>,
        public_base_url: Option<String>,
    ) -> anyhow::Result<Self> {
        opendal::install_default();

        let bucket = settings
            .bucket
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("object store s3 bucket is required"))?;

        let mut builder = S3::default().bucket(bucket);
        if let Some(root) = settings
            .root
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            builder = builder.root(root);
        }
        if let Some(endpoint) = settings
            .endpoint
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            builder = builder.endpoint(endpoint);
        }
        if let Some(region) = settings
            .region
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            builder = builder.region(region);
        }
        if let Some(access_key_id) = settings
            .access_key_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            builder = builder.access_key_id(access_key_id);
        }
        if let Some(secret_access_key) = settings
            .secret_access_key
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            builder = builder.secret_access_key(secret_access_key);
        }

        let operator = Operator::new(builder)?;
        Ok(Self {
            operator,
            prefix,
            public_base_url,
            backend: ObjectStoreBackend::S3,
        })
    }

    #[cfg(not(feature = "s3"))]
    fn from_s3_settings(
        _settings: ObjectStoreSettings,
        _prefix: Option<String>,
        _public_base_url: Option<String>,
    ) -> anyhow::Result<Self> {
        anyhow::bail!("object store s3 backend requires the agenthub-object-store s3 feature")
    }
}

pub fn normalize_object_key(key: &str) -> anyhow::Result<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        anyhow::bail!("object key is required");
    }
    if trimmed.starts_with('/') {
        anyhow::bail!("object key must be relative");
    }
    if trimmed.contains('\\') {
        anyhow::bail!("object key must use forward slashes");
    }

    let mut parts = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." | ".." => anyhow::bail!("object key contains an invalid segment"),
            _ => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

pub fn normalize_image_content_type(content_type: &str) -> anyhow::Result<String> {
    let content_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match content_type.as_str() {
        "image/png" | "image/jpeg" | "image/webp" | "image/gif" => Ok(content_type),
        _ => anyhow::bail!("unsupported hosted image content type: {content_type}"),
    }
}

fn image_extension(content_type: &str) -> anyhow::Result<&'static str> {
    match content_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        "image/gif" => Ok("gif"),
        _ => anyhow::bail!("unsupported hosted image content type: {content_type}"),
    }
}

fn normalize_object_segment(segment: &str, field_name: &str) -> anyhow::Result<String> {
    let segment = normalize_object_key(segment)?;
    if segment.contains('/') {
        anyhow::bail!("{field_name} must be a single path segment");
    }
    Ok(segment)
}

fn normalize_prefix(prefix: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(prefix) = prefix.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let trimmed = prefix.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(normalize_object_key(trimmed)?))
}

fn normalize_public_base_url(public_base_url: Option<&str>) -> Option<String> {
    public_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex_sha256_digest(Sha256::digest(bytes).as_slice())
}

fn hex_sha256_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

async fn abort_writer(writer: &mut opendal::Writer, key: &str) {
    if let Err(err) = writer.abort().await {
        log::warn!("failed to abort object writer {key:?}: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "s3")]
    fn s3_fixture_settings_from_env() -> Option<ObjectStoreSettings> {
        let endpoint = std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_ENDPOINT").ok()?;
        let bucket = std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_BUCKET").ok()?;
        let access_key_id = std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_ACCESS_KEY_ID").ok()?;
        let secret_access_key =
            std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_SECRET_ACCESS_KEY").ok()?;
        let region = std::env::var("AGENTHUB_OBJECT_STORE_S3_TEST_REGION")
            .unwrap_or_else(|_| "us-east-1".to_string());
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Some(ObjectStoreSettings {
            backend: ObjectStoreBackend::S3,
            bucket: Some(bucket),
            endpoint: Some(endpoint),
            region: Some(region),
            access_key_id: Some(access_key_id),
            secret_access_key: Some(secret_access_key),
            prefix: Some(format!("agenthub-object-store-ci/{nonce}")),
            public_base_url: Some("https://img.example.test/objects".to_string()),
            ..ObjectStoreSettings::default()
        })
    }

    #[test]
    fn normalize_object_key_rejects_path_escape() {
        for key in [
            "",
            " ",
            "/absolute",
            "../escape",
            "a/../b",
            "a//b",
            "a\\b",
            "./a",
        ] {
            assert!(
                normalize_object_key(key).is_err(),
                "key should fail: {key:?}"
            );
        }
    }

    #[test]
    fn normalize_object_key_keeps_scoped_relative_keys() {
        assert_eq!(
            normalize_object_key(" team/run/artifact.json ").unwrap(),
            "team/run/artifact.json"
        );
    }

    #[test]
    fn normalize_image_content_type_allows_raster_images_only() {
        assert_eq!(
            normalize_image_content_type(" IMAGE/PNG; charset=binary ").unwrap(),
            "image/png"
        );
        assert!(normalize_image_content_type("image/svg+xml").is_err());
        assert!(normalize_image_content_type("text/html").is_err());
    }

    #[tokio::test]
    async fn fs_store_writes_reads_and_deletes_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("teams/team-1".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let stored = store
            .put_bytes("runs/run-1/artifact.json", br#"{"ok":true}"#.to_vec())
            .await
            .unwrap();
        assert_eq!(stored.key, "teams/team-1/runs/run-1/artifact.json");
        assert_eq!(stored.size_bytes, 11);
        assert_eq!(
            stored.sha256,
            "4062edaf750fb8074e7e83e0c9028c94e32468a8b6f1614774328ef045150f93"
        );
        assert!(store.exists("runs/run-1/artifact.json").await.unwrap());
        assert_eq!(
            store.read_bytes("runs/run-1/artifact.json").await.unwrap(),
            br#"{"ok":true}"#.to_vec()
        );

        store.delete("runs/run-1/artifact.json").await.unwrap();
        assert!(!store.exists("runs/run-1/artifact.json").await.unwrap());
    }

    #[tokio::test]
    async fn fs_store_streams_chunks_and_hashes_object() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let chunks = futures_util::stream::iter([
            Ok::<_, std::io::Error>(Bytes::from_static(b"streamed ")),
            Ok::<_, std::io::Error>(Bytes::from_static(b"download")),
        ]);
        let stored = store
            .put_byte_stream("uploads/teams/team-1/download.txt", chunks)
            .await
            .unwrap();

        assert_eq!(stored.size_bytes, 17);
        assert_eq!(stored.sha256, hex_sha256(b"streamed download"));
        assert_eq!(
            store
                .read_bytes("uploads/teams/team-1/download.txt")
                .await
                .unwrap(),
            b"streamed download"
        );
    }

    #[tokio::test]
    async fn delete_stored_object_deletes_prefixed_key_without_double_prefixing() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("tenant-a".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let stored = store
            .put_bytes("uploads/team-1/report.json", br#"{"ok":true}"#.to_vec())
            .await
            .unwrap();
        assert_eq!(stored.key, "tenant-a/uploads/team-1/report.json");

        store.delete_stored_object(&stored).await.unwrap();

        assert!(!store.exists("uploads/team-1/report.json").await.unwrap());
    }

    #[tokio::test]
    async fn put_image_bytes_scopes_image_keys_and_public_url() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("agenthub/local".to_string()),
            public_base_url: Some("https://img.example.test/".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let hosted = store
            .put_image_bytes("teams/team-1", "avatar-1", "image/png", vec![1, 2, 3, 4])
            .await
            .unwrap();

        assert_eq!(
            hosted.object.key,
            "agenthub/local/images/teams/team-1/avatar-1.png"
        );
        assert_eq!(hosted.content_type, "image/png");
        assert_eq!(
            hosted.public_url.as_deref(),
            Some("https://img.example.test/agenthub/local/images/teams/team-1/avatar-1.png")
        );
        assert!(
            store
                .exists("images/teams/team-1/avatar-1.png")
                .await
                .unwrap()
        );
        assert!(
            store
                .put_image_bytes("teams/team-1", "nested/avatar-1", "image/png", vec![1])
                .await
                .is_err()
        );
    }

    #[cfg(feature = "s3")]
    #[tokio::test]
    async fn s3_compatible_store_writes_reads_and_deletes_bytes() {
        let Some(settings) = s3_fixture_settings_from_env() else {
            eprintln!("skipping s3 fixture: AGENTHUB_OBJECT_STORE_S3_TEST_* env is not set");
            return;
        };
        let store = AgentHubObjectStore::from_settings(settings).unwrap();

        let stored = store
            .put_bytes(
                "uploads/teams/team-1/report.json",
                br#"{"ok":true}"#.to_vec(),
            )
            .await
            .unwrap();
        assert!(stored.key.starts_with("agenthub-object-store-ci/"));
        assert!(stored.key.ends_with("/uploads/teams/team-1/report.json"));
        assert_eq!(stored.size_bytes, 11);
        assert_eq!(
            stored.sha256,
            "4062edaf750fb8074e7e83e0c9028c94e32468a8b6f1614774328ef045150f93"
        );
        assert!(
            store
                .exists("uploads/teams/team-1/report.json")
                .await
                .unwrap()
        );
        assert_eq!(
            store
                .read_bytes("uploads/teams/team-1/report.json")
                .await
                .unwrap(),
            br#"{"ok":true}"#.to_vec()
        );

        let image = store
            .put_image_bytes("agents/agent-1", "image-1", "image/png", vec![1, 2, 3, 4])
            .await
            .unwrap();
        assert!(
            image
                .object
                .key
                .contains("/images/agents/agent-1/image-1.png")
        );
        assert_eq!(image.content_type, "image/png");
        assert_eq!(
            image.public_url,
            Some(format!(
                "https://img.example.test/objects/{}",
                image.object.key
            ))
        );

        store.delete_stored_object(&stored).await.unwrap();
        store.delete_stored_object(&image.object).await.unwrap();
        assert!(
            !store
                .exists("uploads/teams/team-1/report.json")
                .await
                .unwrap()
        );
    }
}
