use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::Path,
    pin::Pin,
    task::{Context as TaskContext, Poll},
    time::Duration,
};

use agenthub_object_store::{
    AgentHubObjectStore, ObjectStoreBackend, ObjectStoreSettings, StoredObject,
};
use anyhow::{Context, anyhow};
use bytes::Bytes;
use chrono::Utc;
use futures::Stream;
use reqwest::{Url, header};
use sqlx::SqlitePool;
use tokio::net::lookup_host;
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
pub struct ObjectDownloadRequest {
    pub actor_id: String,
    pub owner_scope: ObjectUploadOwnerScope,
    pub file_name: String,
    pub content_type: String,
    pub source_url: String,
    pub expected_size_bytes: Option<u64>,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDownloadSettings {
    pub max_bytes: u64,
    pub max_redirects: u8,
    pub timeout: Duration,
    pub allow_private_networks: bool,
}

impl Default for ObjectDownloadSettings {
    fn default() -> Self {
        Self {
            max_bytes: 512 * 1024 * 1024,
            max_redirects: 5,
            timeout: Duration::from_secs(120),
            allow_private_networks: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectUploadService {
    db: SqlitePool,
    store: AgentHubObjectStore,
    download_http: reqwest::Client,
    download_settings: ObjectDownloadSettings,
}

impl ObjectUploadService {
    pub fn new(db: SqlitePool, store: AgentHubObjectStore) -> Self {
        Self::new_with_download_settings(db, store, ObjectDownloadSettings::default())
    }

    pub fn new_with_download_settings(
        db: SqlitePool,
        store: AgentHubObjectStore,
        download_settings: ObjectDownloadSettings,
    ) -> Self {
        let download_http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(download_settings.timeout)
            .build()
            .expect("object download HTTP client settings should be valid");
        Self {
            db,
            store,
            download_http,
            download_settings,
        }
    }

    pub fn from_config(
        db: SqlitePool,
        config: &agenthub_config::AppConfig,
    ) -> anyhow::Result<Self> {
        let settings = object_store_settings_from_config(config)?;
        let download_settings = ObjectDownloadSettings {
            max_bytes: config.object_store_download_max_bytes(),
            max_redirects: config.object_store_download_max_redirects(),
            timeout: Duration::from_secs(config.object_store_download_timeout_seconds()),
            allow_private_networks: config.object_store_download_allow_private_networks(),
        };
        Ok(Self::new_with_download_settings(
            db,
            AgentHubObjectStore::from_settings(settings)?,
            download_settings,
        ))
    }

    pub async fn upload(
        &self,
        request: ObjectUploadRequest,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let file_name = normalize_upload_file_name(&request.file_name)?;
        let expected_size_bytes = request.expected_size_bytes;
        let expected_sha256 = normalize_expected_sha256(request.expected_sha256.as_deref())?;
        let upload_id = Uuid::now_v7().to_string();
        let owner_scope = request.owner_scope.to_string();
        let (object, public_url) = self
            .store_upload_bytes(
                &upload_id,
                &owner_scope,
                &file_name,
                &request.content_type,
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
        self.publish_verified_object(
            &upload_id,
            &owner_scope,
            &file_name,
            &request.content_type,
            object,
            public_url,
            &request.actor_id,
        )
        .await
    }

    pub async fn download(
        &self,
        request: ObjectDownloadRequest,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let file_name = normalize_upload_file_name(&request.file_name)?;
        let expected_size_bytes = request.expected_size_bytes;
        let expected_sha256 = normalize_expected_sha256(request.expected_sha256.as_deref())?;
        if let Some(expected_size_bytes) = expected_size_bytes {
            anyhow::ensure!(
                expected_size_bytes <= self.download_settings.max_bytes,
                "download expected_size_bytes exceeds configured max_bytes"
            );
        }
        let source_url = parse_download_url(&request.source_url)?;
        let upload_id = Uuid::now_v7().to_string();
        let owner_scope = request.owner_scope.to_string();
        let key = format!("uploads/{owner_scope}/{upload_id}/{file_name}");
        let object = self
            .download_to_object(&key, source_url)
            .await
            .context("download object source")?;
        if let Err(err) =
            verify_stored_object(&object, expected_size_bytes, expected_sha256.as_deref())
        {
            if let Err(cleanup_err) = self.store.delete_stored_object(&object).await {
                tracing::warn!(
                    object_key = %object.key,
                    error = %cleanup_err,
                    "failed to delete object after download verification failure"
                );
            }
            return Err(err);
        }
        self.publish_verified_object(
            &upload_id,
            &owner_scope,
            &file_name,
            &request.content_type,
            object,
            None,
            &request.actor_id,
        )
        .await
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

    async fn download_to_object(&self, key: &str, source_url: Url) -> anyhow::Result<StoredObject> {
        let mut current = source_url;
        for redirect_count in 0..=self.download_settings.max_redirects {
            validate_download_url(&current, self.download_settings.allow_private_networks).await?;
            let response = self
                .download_http
                .get(current.clone())
                .header(header::ACCEPT, "*/*")
                .send()
                .await
                .with_context(|| format!("request download source {current}"))?;
            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .ok_or_else(|| anyhow!("download redirect missing Location header"))?
                    .to_str()
                    .context("download redirect Location header is not valid UTF-8")?;
                current = current
                    .join(location)
                    .context("download redirect Location header is not a valid URL")?;
                anyhow::ensure!(
                    redirect_count < self.download_settings.max_redirects,
                    "download exceeded max redirects"
                );
                continue;
            }
            anyhow::ensure!(
                response.status().is_success(),
                "download source returned HTTP {}",
                response.status()
            );
            if let Some(content_length) = response.content_length() {
                anyhow::ensure!(
                    content_length <= self.download_settings.max_bytes,
                    "download content-length exceeds configured max_bytes"
                );
            }
            let stream = LimitedDownloadStream::new(
                response.bytes_stream(),
                self.download_settings.max_bytes,
            );
            return self.store.put_byte_stream(key, stream).await;
        }
        Err(anyhow!("download exceeded max redirects"))
    }

    async fn publish_verified_object(
        &self,
        upload_id: &str,
        owner_scope: &str,
        file_name: &str,
        content_type: &str,
        object: StoredObject,
        public_url: Option<String>,
        actor_id: &str,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let now = Utc::now().timestamp();
        let size_bytes = i64::try_from(object.size_bytes)
            .context("uploaded object is too large for SQLite metadata size_bytes")?;
        let upload = agenthub_db::NewObjectUpload {
            id: upload_id,
            owner_scope,
            backend: object_store_backend_name(self.store.backend()),
            object_key: &object.key,
            original_filename: file_name,
            content_type,
            size_bytes,
            sha256: &object.sha256,
            public_url: public_url.as_deref(),
            created_by_actor_id: actor_id,
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
}

struct LimitedDownloadStream {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    max_bytes: u64,
    seen_bytes: u64,
}

impl LimitedDownloadStream {
    fn new(
        inner: impl Stream<Item = reqwest::Result<Bytes>> + Send + 'static,
        max_bytes: u64,
    ) -> Self {
        Self {
            inner: Box::pin(inner),
            max_bytes,
            seen_bytes: 0,
        }
    }
}

impl Stream for LimitedDownloadStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                self.seen_bytes = match self.seen_bytes.checked_add(chunk.len() as u64) {
                    Some(value) => value,
                    None => {
                        return Poll::Ready(Some(Err(std::io::Error::other(
                            "download exceeded configured max_bytes",
                        ))));
                    }
                };
                if self.seen_bytes > self.max_bytes {
                    return Poll::Ready(Some(Err(std::io::Error::other(
                        "download exceeded configured max_bytes",
                    ))));
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(std::io::Error::other(err)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn parse_download_url(value: &str) -> anyhow::Result<Url> {
    let url = Url::parse(value.trim()).context("source_url must be a valid URL")?;
    validate_download_url_shape(&url)?;
    Ok(url)
}

fn validate_download_url_shape(url: &Url) -> anyhow::Result<()> {
    anyhow::ensure!(
        url.scheme() == "http" || url.scheme() == "https",
        "source_url must use http or https"
    );
    anyhow::ensure!(url.host_str().is_some(), "source_url must include a host");
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "source_url must not include credentials"
    );
    Ok(())
}

async fn validate_download_url(url: &Url, allow_private_networks: bool) -> anyhow::Result<()> {
    validate_download_url_shape(url)?;
    let host = url.host_str().expect("host checked above");
    if !allow_private_networks && host.eq_ignore_ascii_case("localhost") {
        anyhow::bail!("source_url host resolves to a private or local address");
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("source_url must include a valid port"))?;
    let addresses = lookup_host((host, port))
        .await
        .with_context(|| format!("resolve download source host {host:?}"))?
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !addresses.is_empty(),
        "source_url host did not resolve to any address"
    );
    if !allow_private_networks {
        for address in addresses {
            anyhow::ensure!(
                is_public_download_ip(address.ip()),
                "source_url host resolves to a private or local address"
            );
        }
    }
    Ok(())
}

fn is_public_download_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || ip.is_unspecified()
        || ip.octets()[0] == 0
        || ip.octets()[0] == 100 && (ip.octets()[1] & 0b1100_0000) == 0b0100_0000)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
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
    async fn download_url_guard_rejects_private_sources_by_default() {
        let url = parse_download_url("http://127.0.0.1:8080/file.txt").unwrap();
        let err = validate_download_url(&url, false)
            .await
            .expect_err("private download sources should be rejected");
        assert!(err.to_string().contains("private or local"));

        validate_download_url(&url, true)
            .await
            .expect("private download sources can be explicitly enabled");
    }

    #[test]
    fn download_url_shape_rejects_unsupported_sources() {
        for source_url in [
            "file:///tmp/report.txt",
            "ftp://example.com/report.txt",
            "https://user:pass@example.com/report.txt",
            "https://",
        ] {
            assert!(
                parse_download_url(source_url).is_err(),
                "source URL should be rejected: {source_url}"
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
}
