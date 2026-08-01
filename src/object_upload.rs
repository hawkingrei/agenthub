use std::{
    collections::HashMap,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
    time::{Duration, Instant},
};

use agenthub_object_store::{
    AgentHubObjectStore, ObjectStoreBackend, ObjectStoreSettings, StoredObject,
};
use anyhow::{Context, anyhow};
use bytes::Bytes;
use chrono::Utc;
use futures::Stream;
use reqwest::{StatusCode, Url, header};
use sqlx::SqlitePool;
use tokio::{
    net::lookup_host,
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    time::sleep,
};
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
    pub retry_attempts: u8,
    pub retry_backoff: Duration,
    pub max_concurrent_per_host: u16,
    pub allow_private_networks: bool,
    pub allowed_hosts: Vec<String>,
    pub denied_hosts: Vec<String>,
}

impl Default for ObjectDownloadSettings {
    fn default() -> Self {
        Self {
            max_bytes: 512 * 1024 * 1024,
            max_redirects: 5,
            timeout: Duration::from_secs(120),
            retry_attempts: 3,
            retry_backoff: Duration::from_millis(250),
            max_concurrent_per_host: 4,
            allow_private_networks: false,
            allowed_hosts: Vec::new(),
            denied_hosts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectUploadService {
    db: SqlitePool,
    store: AgentHubObjectStore,
    download_http: reqwest::Client,
    download_settings: ObjectDownloadSettings,
    download_host_limiters: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
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
            download_host_limiters: Arc::new(Mutex::new(HashMap::new())),
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
            retry_attempts: config.object_store_download_retry_attempts(),
            retry_backoff: Duration::from_millis(
                config.object_store_download_retry_backoff_millis(),
            ),
            max_concurrent_per_host: config.object_store_download_max_concurrent_per_host(),
            allow_private_networks: config.object_store_download_allow_private_networks(),
            allowed_hosts: config.object_store_download_allowed_hosts(),
            denied_hosts: config.object_store_download_denied_hosts(),
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
        self.publish_verified_object(VerifiedObjectPublication {
            upload_id,
            owner_scope,
            file_name,
            content_type: request.content_type,
            object,
            public_url,
            actor_id: request.actor_id,
        })
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
        let source_host = source_url
            .host_str()
            .expect("host checked by parse_download_url")
            .to_string();
        let started_at = Instant::now();
        let upload_id = Uuid::now_v7().to_string();
        let owner_scope = request.owner_scope.to_string();
        let key = format!("uploads/{owner_scope}/{upload_id}/{file_name}");
        let object = match self.download_to_object(&key, source_url).await {
            Ok(object) => {
                tracing::info!(
                    upload_id = %upload_id,
                    owner_scope = %owner_scope,
                    source_host = %source_host,
                    object_key = %object.key,
                    size_bytes = object.size_bytes,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "object download ingestion completed"
                );
                object
            }
            Err(err) => {
                tracing::warn!(
                    upload_id = %upload_id,
                    owner_scope = %owner_scope,
                    source_host = %source_host,
                    failure_class = classify_download_error(&err),
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    error = %err,
                    "object download ingestion failed"
                );
                return Err(err).context("download object source");
            }
        };
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
        self.publish_verified_object(VerifiedObjectPublication {
            upload_id,
            owner_scope,
            file_name,
            content_type: request.content_type,
            object,
            public_url: None,
            actor_id: request.actor_id,
        })
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
            validate_download_url(&current, &self.download_settings).await?;
            let _host_permit = self.acquire_download_host_permit(&current).await?;
            let response = self.send_download_request(&current).await?;
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

    async fn acquire_download_host_permit(
        &self,
        url: &Url,
    ) -> anyhow::Result<OwnedSemaphorePermit> {
        let host = normalized_download_url_host(url)?;
        let max_concurrent_per_host = self.download_settings.max_concurrent_per_host.max(1);
        let limiter = {
            let mut limiters = self.download_host_limiters.lock().await;
            limiters
                .entry(host.clone())
                .or_insert_with(|| Arc::new(Semaphore::new(max_concurrent_per_host as usize)))
                .clone()
        };
        let started_at = Instant::now();
        let permit = limiter
            .acquire_owned()
            .await
            .with_context(|| format!("acquire download source host concurrency permit {host:?}"))?;
        let waited_ms = started_at.elapsed().as_millis() as u64;
        if waited_ms > 0 {
            tracing::debug!(
                source_host = %host,
                waited_ms,
                max_concurrent_per_host,
                "object download source host concurrency permit acquired"
            );
        }
        Ok(permit)
    }

    async fn send_download_request(&self, url: &Url) -> anyhow::Result<reqwest::Response> {
        let max_attempts = self.download_settings.retry_attempts.max(1);
        let mut attempt = 1;
        loop {
            let response = self
                .download_http
                .get(url.clone())
                .header(header::ACCEPT, "*/*")
                .send()
                .await;
            match response {
                Ok(response)
                    if should_retry_download_status(response.status())
                        && attempt < max_attempts =>
                {
                    let status = response.status();
                    self.log_download_retry(url, attempt, Some(status), None);
                    self.sleep_before_download_retry(attempt).await;
                    attempt += 1;
                }
                Ok(response) => return Ok(response),
                Err(err) if is_retryable_download_request_error(&err) && attempt < max_attempts => {
                    let error = err.to_string();
                    self.log_download_retry(url, attempt, None, Some(error.as_str()));
                    self.sleep_before_download_retry(attempt).await;
                    attempt += 1;
                }
                Err(err) => {
                    return Err(err).with_context(|| format!("request download source {url}"));
                }
            }
        }
    }

    async fn sleep_before_download_retry(&self, attempt: u8) {
        if self.download_settings.retry_backoff.is_zero() {
            return;
        }
        let delay = self
            .download_settings
            .retry_backoff
            .checked_mul(attempt as u32)
            .unwrap_or(self.download_settings.retry_backoff);
        sleep(delay).await;
    }

    fn log_download_retry(
        &self,
        url: &Url,
        attempt: u8,
        status: Option<StatusCode>,
        error: Option<&str>,
    ) {
        tracing::warn!(
            source_host = %url.host_str().unwrap_or("<missing>"),
            attempt,
            max_attempts = self.download_settings.retry_attempts.max(1),
            status = status.map(|status| status.as_u16()),
            error,
            "retrying object download source request"
        );
    }

    async fn publish_verified_object(
        &self,
        publication: VerifiedObjectPublication,
    ) -> anyhow::Result<agenthub_db::ObjectUploadRecord> {
        let object = publication.object;
        let now = Utc::now().timestamp();
        let size_bytes = i64::try_from(object.size_bytes)
            .context("uploaded object is too large for SQLite metadata size_bytes")?;
        let upload = agenthub_db::NewObjectUpload {
            id: &publication.upload_id,
            owner_scope: &publication.owner_scope,
            backend: object_store_backend_name(self.store.backend()),
            object_key: &object.key,
            original_filename: &publication.file_name,
            content_type: &publication.content_type,
            size_bytes,
            sha256: &object.sha256,
            public_url: publication.public_url.as_deref(),
            created_by_actor_id: &publication.actor_id,
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

struct VerifiedObjectPublication {
    upload_id: String,
    owner_scope: String,
    file_name: String,
    content_type: String,
    object: StoredObject,
    public_url: Option<String>,
    actor_id: String,
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

fn normalized_download_url_host(url: &Url) -> anyhow::Result<String> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("source_url must include a host"))?;
    normalize_download_host_pattern(host)
}

fn should_retry_download_status(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn is_retryable_download_request_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout()
}

fn classify_download_error(err: &anyhow::Error) -> &'static str {
    let message = err.to_string();
    if message.contains("HTTP 408") || message.contains("HTTP 429") {
        return "transient_status";
    }
    if message.contains("HTTP 5") {
        return "server_status";
    }
    if message.contains("max_bytes") || message.contains("content-length") {
        return "size_limit";
    }
    if message.contains("private or local")
        || message.contains("denied by object_store.download_denied_hosts")
        || message.contains("not allowed by object_store.download_allowed_hosts")
    {
        return "source_policy";
    }
    if message.contains("max redirects") || message.contains("redirect") {
        return "redirect";
    }
    if message.contains("resolve download source host") {
        return "dns";
    }
    "request"
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

async fn validate_download_url(url: &Url, settings: &ObjectDownloadSettings) -> anyhow::Result<()> {
    validate_download_url_shape(url)?;
    let host = url.host_str().expect("host checked above");
    validate_download_host_policy(host, &settings.allowed_hosts, &settings.denied_hosts)?;
    if !settings.allow_private_networks && host.eq_ignore_ascii_case("localhost") {
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
    if !settings.allow_private_networks {
        for address in addresses {
            anyhow::ensure!(
                is_public_download_ip(address.ip()),
                "source_url host resolves to a private or local address"
            );
        }
    }
    Ok(())
}

fn validate_download_host_policy(
    host: &str,
    allowed_hosts: &[String],
    denied_hosts: &[String],
) -> anyhow::Result<()> {
    let host = normalize_download_host_pattern(host)?;
    if denied_hosts
        .iter()
        .any(|pattern| download_host_pattern_matches(pattern, &host))
    {
        anyhow::bail!("source_url host is denied by object_store.download_denied_hosts");
    }
    if !allowed_hosts.is_empty()
        && !allowed_hosts
            .iter()
            .any(|pattern| download_host_pattern_matches(pattern, &host))
    {
        anyhow::bail!("source_url host is not allowed by object_store.download_allowed_hosts");
    }
    Ok(())
}

fn normalize_download_host_pattern(value: &str) -> anyhow::Result<String> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    anyhow::ensure!(!value.is_empty(), "download host pattern must not be empty");
    Ok(value)
}

fn download_host_pattern_matches(pattern: &str, host: &str) -> bool {
    let Ok(pattern) = normalize_download_host_pattern(pattern) else {
        return false;
    };
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return host
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.') && prefix.len() > 1);
    }
    pattern == host
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{Router, extract::State, response::IntoResponse, routing::get};
    use sqlx::SqlitePool;
    use tokio::net::TcpListener;

    #[test]
    fn download_retry_status_policy_only_retries_transient_failures() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            assert!(
                should_retry_download_status(status),
                "status should be retried: {status}"
            );
        }

        for status in [
            StatusCode::OK,
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::PERMANENT_REDIRECT,
        ] {
            assert!(
                !should_retry_download_status(status),
                "status should not be retried: {status}"
            );
        }
    }

    #[test]
    fn download_error_classification_keeps_operator_signal() {
        assert_eq!(
            classify_download_error(&anyhow!("download source returned HTTP 503")),
            "server_status"
        );
        assert_eq!(
            classify_download_error(&anyhow!(
                "download content-length exceeds configured max_bytes"
            )),
            "size_limit"
        );
        assert_eq!(
            classify_download_error(&anyhow!(
                "source_url host resolves to a private or local address"
            )),
            "source_policy"
        );
        assert_eq!(
            classify_download_error(&anyhow!("download exceeded max redirects")),
            "redirect"
        );
    }

    #[tokio::test]
    async fn download_request_retries_transient_status_before_success() {
        let (url, attempts) = spawn_retry_source(StatusCode::SERVICE_UNAVAILABLE, StatusCode::OK)
            .await
            .expect("spawn retry source");
        let service = test_download_request_service(2);

        let response = service
            .send_download_request(&url)
            .await
            .expect("request should retry and then succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn download_request_does_not_retry_permanent_status() {
        let (url, attempts) = spawn_retry_source(StatusCode::BAD_REQUEST, StatusCode::OK)
            .await
            .expect("spawn retry source");
        let service = test_download_request_service(2);

        let response = service
            .send_download_request(&url)
            .await
            .expect("permanent status should return without retry");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn download_host_concurrency_limit_is_keyed_by_normalized_host() {
        let service = test_download_service_with_settings(ObjectDownloadSettings {
            max_concurrent_per_host: 1,
            ..ObjectDownloadSettings::default()
        });
        let first_url = Url::parse("https://Downloads.Example.Test.:443/file").unwrap();
        let same_host_url = Url::parse("https://downloads.example.test:8443/other").unwrap();
        let other_host_url = Url::parse("https://other.example.test/file").unwrap();

        let first_permit = service
            .acquire_download_host_permit(&first_url)
            .await
            .expect("first same-host permit");
        tokio::time::timeout(
            Duration::from_millis(50),
            service.acquire_download_host_permit(&same_host_url),
        )
        .await
        .expect_err("same normalized host should wait for the permit");

        let other_permit = tokio::time::timeout(
            Duration::from_millis(50),
            service.acquire_download_host_permit(&other_host_url),
        )
        .await
        .expect("different host should not wait")
        .expect("different host permit");
        drop(other_permit);

        drop(first_permit);
        let released_permit = tokio::time::timeout(
            Duration::from_millis(50),
            service.acquire_download_host_permit(&same_host_url),
        )
        .await
        .expect("same host should acquire after release")
        .expect("released same-host permit");
        drop(released_permit);
    }

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
        let err = validate_download_url(&url, &ObjectDownloadSettings::default())
            .await
            .expect_err("private download sources should be rejected");
        assert!(err.to_string().contains("private or local"));

        validate_download_url(
            &url,
            &ObjectDownloadSettings {
                allow_private_networks: true,
                ..ObjectDownloadSettings::default()
            },
        )
        .await
        .expect("private download sources can be explicitly enabled");
    }

    #[test]
    fn download_host_policy_denies_blocked_hosts_before_allow_list() {
        let allowed_hosts = vec!["*.example.test".to_string()];
        let denied_hosts = vec!["blocked.example.test".to_string()];

        validate_download_host_policy("files.example.test", &allowed_hosts, &denied_hosts)
            .expect("matching wildcard host should be allowed");

        let err =
            validate_download_host_policy("blocked.example.test", &allowed_hosts, &denied_hosts)
                .expect_err("denied host should win over allow list");
        assert!(err.to_string().contains("denied"));

        let err = validate_download_host_policy("example.test", &allowed_hosts, &denied_hosts)
            .expect_err("wildcard should not match apex host");
        assert!(err.to_string().contains("not allowed"));
    }

    #[test]
    fn download_host_policy_normalizes_case_and_trailing_dot() {
        let allowed_hosts = vec!["Downloads.Example.Test.".to_string()];
        let denied_hosts = vec!["*.internal.example.test".to_string()];

        validate_download_host_policy("downloads.example.test.", &allowed_hosts, &denied_hosts)
            .expect("exact host should normalize");
        validate_download_host_policy("Sub.Internal.Example.Test", &[], &denied_hosts)
            .expect_err("wildcard deny should normalize");
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

    fn test_download_request_service(retry_attempts: u8) -> ObjectUploadService {
        test_download_service_with_settings(ObjectDownloadSettings {
            retry_attempts,
            retry_backoff: Duration::ZERO,
            ..ObjectDownloadSettings::default()
        })
    }

    fn test_download_service_with_settings(
        download_settings: ObjectDownloadSettings,
    ) -> ObjectUploadService {
        let root =
            std::env::temp_dir().join(format!("agenthub-object-download-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&root).expect("create object store tempdir");
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(root.to_string_lossy().into_owned()),
            ..ObjectStoreSettings::default()
        })
        .expect("create object store");
        ObjectUploadService::new_with_download_settings(
            SqlitePool::connect_lazy("sqlite::memory:").expect("create lazy sqlite pool"),
            store,
            download_settings,
        )
    }

    async fn spawn_retry_source(
        first_status: StatusCode,
        next_status: StatusCode,
    ) -> anyhow::Result<(Url, Arc<AtomicUsize>)> {
        let attempts = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/", get(retry_source_handler))
            .with_state(RetrySourceState {
                attempts: attempts.clone(),
                first_status,
                next_status,
            });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok((Url::parse(&format!("http://{address}/"))?, attempts))
    }

    #[derive(Clone)]
    struct RetrySourceState {
        attempts: Arc<AtomicUsize>,
        first_status: StatusCode,
        next_status: StatusCode,
    }

    async fn retry_source_handler(State(state): State<RetrySourceState>) -> impl IntoResponse {
        let attempt = state.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            state.first_status
        } else {
            state.next_status
        }
    }
}
