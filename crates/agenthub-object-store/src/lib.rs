use std::{ops::Range, path::Path, time::Duration};

use anyhow::{Context, anyhow};
use opendal::{Operator, Writer, services::Fs};
use sha2::{Digest, Sha256};

#[cfg(feature = "s3")]
use {
    aws_credential_types::Credentials,
    aws_sigv4::{
        http_request::{
            PayloadChecksumKind, PercentEncodingMode, SessionTokenMode, SignableBody,
            SignableRequest, SignatureLocation, SigningSettings, UriPathNormalizationMode, sign,
        },
        sign::v4,
    },
    aws_smithy_runtime_api::client::identity::Identity,
    http::{Request, header},
    opendal::services::S3,
    percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode},
    quick_xml::{de::from_str as xml_from_str, se::to_string as xml_to_string},
    reqwest::{Client, Method},
    serde::{Deserialize, Serialize},
    std::time::SystemTime,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresignedObjectWrite {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartUpload {
    pub key: String,
    pub upload_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartUploadPart {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresignedMultipartUploadPart {
    pub part_number: u32,
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub expires_in_seconds: u64,
}

pub struct AgentHubObjectWriter {
    key: String,
    writer: Writer,
    size_bytes: u64,
    hasher: Sha256,
}

impl AgentHubObjectWriter {
    pub async fn write_chunk(&mut self, chunk: Vec<u8>) -> anyhow::Result<()> {
        if chunk.is_empty() {
            return Ok(());
        }
        self.size_bytes = self
            .size_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| anyhow!("object size overflow"))?;
        self.hasher.update(&chunk);
        self.writer
            .write(chunk)
            .await
            .with_context(|| format!("write object chunk {:?}", self.key))?;
        Ok(())
    }

    pub async fn finish(mut self) -> anyhow::Result<StoredObject> {
        self.writer
            .close()
            .await
            .with_context(|| format!("close object writer {:?}", self.key))?;
        Ok(StoredObject {
            key: self.key,
            size_bytes: self.size_bytes,
            sha256: hex_digest(self.hasher.finalize().as_slice()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct AgentHubObjectStore {
    operator: Operator,
    prefix: Option<String>,
    public_base_url: Option<String>,
    backend: ObjectStoreBackend,
    #[cfg(feature = "s3")]
    s3_multipart: Option<S3MultipartClient>,
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
                    #[cfg(feature = "s3")]
                    s3_multipart: None,
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

    pub async fn put_stored_key_bytes(
        &self,
        key: &str,
        bytes: impl Into<Vec<u8>>,
    ) -> anyhow::Result<StoredObject> {
        let bytes = bytes.into();
        let size_bytes = bytes.len() as u64;
        let sha256 = hex_sha256(&bytes);
        let normalized_key = normalize_object_key(key)?;
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

    pub async fn put_stored_key_chunks<I>(
        &self,
        key: &str,
        chunks: I,
    ) -> anyhow::Result<StoredObject>
    where
        I: IntoIterator<Item = Vec<u8>>,
    {
        let mut writer = self.stored_key_writer(key).await?;
        for chunk in chunks {
            writer.write_chunk(chunk).await?;
        }
        writer.finish().await
    }

    pub async fn stored_key_writer(&self, key: &str) -> anyhow::Result<AgentHubObjectWriter> {
        let normalized_key = normalize_object_key(key)?;
        let writer = self
            .operator
            .writer(&normalized_key)
            .await
            .with_context(|| format!("open object writer {normalized_key:?}"))?;
        Ok(AgentHubObjectWriter {
            key: normalized_key,
            writer,
            size_bytes: 0,
            hasher: Sha256::new(),
        })
    }

    pub async fn presign_stored_key_write(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> anyhow::Result<PresignedObjectWrite> {
        anyhow::ensure!(
            self.backend == ObjectStoreBackend::S3,
            "presigned object writes require the s3 backend"
        );
        anyhow::ensure!(
            !expires_in.is_zero(),
            "presigned object write expiration must be greater than zero"
        );
        let normalized_key = normalize_object_key(key)?;
        let request = self
            .operator
            .presign_write(&normalized_key, expires_in)
            .await
            .with_context(|| format!("presign object write {normalized_key:?}"))?;
        let headers = request
            .header()
            .iter()
            .map(|(name, value)| {
                let value = value
                    .to_str()
                    .with_context(|| format!("presigned header {name} is not valid UTF-8"))?;
                Ok((name.to_string(), value.to_string()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(PresignedObjectWrite {
            method: request.method().as_str().to_string(),
            uri: request.uri().to_string(),
            headers,
            expires_in_seconds: expires_in.as_secs(),
        })
    }

    pub async fn initiate_stored_key_multipart_upload(
        &self,
        key: &str,
        content_type: Option<&str>,
    ) -> anyhow::Result<MultipartUpload> {
        let normalized_key = normalize_object_key(key)?;
        self.s3_multipart_client()?
            .initiate_multipart_upload(&normalized_key, content_type)
            .await
    }

    pub async fn presign_stored_key_multipart_upload_part(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
        expires_in: Duration,
    ) -> anyhow::Result<PresignedMultipartUploadPart> {
        let normalized_key = normalize_object_key(key)?;
        self.s3_multipart_client()?.presign_upload_part(
            &normalized_key,
            upload_id,
            part_number,
            expires_in,
        )
    }

    pub async fn complete_stored_key_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
        parts: Vec<MultipartUploadPart>,
    ) -> anyhow::Result<()> {
        let normalized_key = normalize_object_key(key)?;
        self.s3_multipart_client()?
            .complete_multipart_upload(&normalized_key, upload_id, parts)
            .await
    }

    pub async fn abort_stored_key_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> anyhow::Result<()> {
        let normalized_key = normalize_object_key(key)?;
        self.s3_multipart_client()?
            .abort_multipart_upload(&normalized_key, upload_id)
            .await
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

    pub async fn read_stored_key_bytes(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let normalized_key = normalize_object_key(key)?;
        let bytes = self
            .operator
            .read(&normalized_key)
            .await
            .with_context(|| format!("read object {normalized_key:?}"))?;
        Ok(bytes.to_vec())
    }

    pub async fn inspect_stored_key(&self, key: &str) -> anyhow::Result<StoredObject> {
        const READ_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

        let normalized_key = normalize_object_key(key)?;
        let metadata = self
            .operator
            .stat(&normalized_key)
            .await
            .with_context(|| format!("stat object {normalized_key:?}"))?;
        let size_bytes = metadata.content_length();
        let reader = self
            .operator
            .reader(&normalized_key)
            .await
            .with_context(|| format!("open object reader {normalized_key:?}"))?;

        let mut hasher = Sha256::new();
        let mut offset = 0_u64;
        while offset < size_bytes {
            let end = offset.saturating_add(READ_CHUNK_BYTES).min(size_bytes);
            let range: Range<u64> = offset..end;
            let chunk = reader
                .read(range)
                .await
                .with_context(|| format!("read object chunk {normalized_key:?}"))?;
            for bytes in chunk {
                hasher.update(&bytes);
            }
            offset = end;
        }

        Ok(StoredObject {
            key: normalized_key,
            size_bytes,
            sha256: hex_digest(hasher.finalize().as_slice()),
        })
    }

    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let normalized_key = self.scoped_key(key)?;
        self.operator
            .delete(&normalized_key)
            .await
            .with_context(|| format!("delete object {normalized_key:?}"))?;
        Ok(())
    }

    pub async fn delete_stored_key(&self, key: &str) -> anyhow::Result<()> {
        let normalized_key = normalize_object_key(key)?;
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

    pub async fn exists_stored_key(&self, key: &str) -> anyhow::Result<bool> {
        let normalized_key = normalize_object_key(key)?;
        self.operator
            .exists(&normalized_key)
            .await
            .with_context(|| format!("stat object {normalized_key:?}"))
    }

    #[cfg(feature = "s3")]
    fn s3_multipart_client(&self) -> anyhow::Result<&S3MultipartClient> {
        anyhow::ensure!(
            self.backend == ObjectStoreBackend::S3,
            "s3 multipart uploads require the s3 backend"
        );
        self.s3_multipart.as_ref().ok_or_else(|| {
            anyhow!(
                "s3 multipart uploads require explicit bucket, endpoint, region, access key, and secret key settings"
            )
        })
    }

    #[cfg(not(feature = "s3"))]
    fn s3_multipart_client(&self) -> anyhow::Result<&S3MultipartClient> {
        anyhow::bail!("s3 multipart uploads require the agenthub-object-store s3 feature")
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
        let (key, content_type) = self.scoped_image_key(scope, image_id, content_type)?;
        let object = self.put_bytes(&key, bytes).await?;
        let public_url = self.public_url_for_key(&object.key);
        Ok(HostedImageObject {
            object,
            content_type,
            public_url,
        })
    }

    pub fn scoped_image_key(
        &self,
        scope: &str,
        image_id: &str,
        content_type: &str,
    ) -> anyhow::Result<(String, String)> {
        let content_type = normalize_image_content_type(content_type)?;
        let scope = normalize_object_key(scope)?;
        let image_id = normalize_object_segment(image_id, "image_id")?;
        let extension = image_extension(&content_type)?;
        Ok((
            format!("images/{scope}/{image_id}.{extension}"),
            content_type,
        ))
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

        let multipart = S3MultipartClient::from_settings(&settings)?;
        let operator = Operator::new(builder)?;
        Ok(Self {
            operator,
            prefix,
            public_base_url,
            backend: ObjectStoreBackend::S3,
            s3_multipart: multipart,
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

#[cfg(not(feature = "s3"))]
struct S3MultipartClient;

#[cfg(not(feature = "s3"))]
impl S3MultipartClient {
    async fn initiate_multipart_upload(
        &self,
        _key: &str,
        _content_type: Option<&str>,
    ) -> anyhow::Result<MultipartUpload> {
        anyhow::bail!("s3 multipart uploads require the agenthub-object-store s3 feature")
    }

    fn presign_upload_part(
        &self,
        _key: &str,
        _upload_id: &str,
        _part_number: u32,
        _expires_in: Duration,
    ) -> anyhow::Result<PresignedMultipartUploadPart> {
        anyhow::bail!("s3 multipart uploads require the agenthub-object-store s3 feature")
    }

    async fn complete_multipart_upload(
        &self,
        _key: &str,
        _upload_id: &str,
        _parts: Vec<MultipartUploadPart>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("s3 multipart uploads require the agenthub-object-store s3 feature")
    }

    async fn abort_multipart_upload(&self, _key: &str, _upload_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("s3 multipart uploads require the agenthub-object-store s3 feature")
    }
}

#[cfg(feature = "s3")]
#[derive(Debug, Clone)]
struct S3MultipartClient {
    endpoint: String,
    root: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    client: Client,
}

#[cfg(feature = "s3")]
#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct InitiateMultipartUploadResult {
    upload_id: String,
}

#[cfg(feature = "s3")]
impl Default for InitiateMultipartUploadResult {
    fn default() -> Self {
        Self {
            upload_id: String::new(),
        }
    }
}

#[cfg(feature = "s3")]
#[derive(Debug, Serialize)]
#[serde(rename = "CompleteMultipartUpload", rename_all = "PascalCase")]
struct CompleteMultipartUploadRequest {
    part: Vec<CompleteMultipartUploadRequestPart>,
}

#[cfg(feature = "s3")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct CompleteMultipartUploadRequestPart {
    #[serde(rename = "PartNumber")]
    part_number: u32,
    #[serde(rename = "ETag")]
    etag: String,
}

#[cfg(feature = "s3")]
impl S3MultipartClient {
    fn from_settings(settings: &ObjectStoreSettings) -> anyhow::Result<Option<Self>> {
        let Some(bucket) = settings
            .bucket
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let Some(region) = settings
            .region
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let Some(access_key_id) = settings
            .access_key_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let Some(secret_access_key) = settings
            .secret_access_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let endpoint = s3_path_style_endpoint(settings.endpoint.as_deref(), bucket, region)?;
        Ok(Some(Self {
            endpoint,
            root: normalize_s3_root(settings.root.as_deref()),
            region: region.to_string(),
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            client: Client::new(),
        }))
    }

    async fn initiate_multipart_upload(
        &self,
        key: &str,
        content_type: Option<&str>,
    ) -> anyhow::Result<MultipartUpload> {
        let url = format!("{}{}?uploads", self.endpoint, self.encoded_path(key)?);
        let mut headers = Vec::new();
        if let Some(content_type) = content_type
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            headers.push((
                header::CONTENT_TYPE.as_str().to_string(),
                content_type.to_string(),
            ));
        }
        let response = self
            .send_signed(Method::POST, &url, headers, Vec::new())
            .await
            .with_context(|| format!("initiate s3 multipart upload {key:?}"))?;
        let status = response.status();
        let body = response.text().await.context("read s3 initiate response")?;
        anyhow::ensure!(
            status.is_success(),
            "s3 initiate multipart upload failed with status {status}: {body}"
        );
        let result: InitiateMultipartUploadResult =
            xml_from_str(&body).context("decode s3 initiate multipart upload response")?;
        anyhow::ensure!(
            !result.upload_id.is_empty(),
            "s3 initiate multipart upload response did not include UploadId"
        );
        Ok(MultipartUpload {
            key: key.to_string(),
            upload_id: result.upload_id,
        })
    }

    fn presign_upload_part(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
        expires_in: Duration,
    ) -> anyhow::Result<PresignedMultipartUploadPart> {
        anyhow::ensure!(part_number > 0, "part_number must be greater than zero");
        anyhow::ensure!(
            !expires_in.is_zero(),
            "presigned multipart upload part expiration must be greater than zero"
        );
        let url = format!(
            "{}{}?partNumber={part_number}&uploadId={}",
            self.endpoint,
            self.encoded_path(key)?,
            percent_encode_query_value(upload_id)
        );
        let mut request = Request::builder()
            .method("PUT")
            .uri(&url)
            .body(())
            .context("build s3 upload part presign request")?;
        self.sign_request(
            &mut request,
            SignableBody::UnsignedPayload,
            Some(expires_in),
        )?;
        Ok(PresignedMultipartUploadPart {
            part_number,
            method: "PUT".to_string(),
            uri: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|(name, value)| {
                    let value = value
                        .to_str()
                        .with_context(|| format!("signed header {name} is not valid UTF-8"))?;
                    Ok((name.to_string(), value.to_string()))
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
            expires_in_seconds: expires_in.as_secs(),
        })
    }

    async fn complete_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
        parts: Vec<MultipartUploadPart>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!parts.is_empty(), "s3 multipart upload parts are required");
        let request = CompleteMultipartUploadRequest {
            part: parts
                .into_iter()
                .map(|part| CompleteMultipartUploadRequestPart {
                    part_number: part.part_number,
                    etag: part.etag,
                })
                .collect(),
        };
        let body =
            xml_to_string(&request).context("encode s3 complete multipart upload request")?;
        let url = format!(
            "{}{}?uploadId={}",
            self.endpoint,
            self.encoded_path(key)?,
            percent_encode_query_value(upload_id)
        );
        let headers = vec![
            (
                header::CONTENT_TYPE.as_str().to_string(),
                "application/xml".to_string(),
            ),
            (
                header::CONTENT_LENGTH.as_str().to_string(),
                body.len().to_string(),
            ),
        ];
        let response = self
            .send_signed(Method::POST, &url, headers, body.into_bytes())
            .await
            .with_context(|| format!("complete s3 multipart upload {key:?}"))?;
        let status = response.status();
        let body = response.text().await.context("read s3 complete response")?;
        anyhow::ensure!(
            status.is_success(),
            "s3 complete multipart upload failed with status {status}: {body}"
        );
        Ok(())
    }

    async fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> anyhow::Result<()> {
        let url = format!(
            "{}{}?uploadId={}",
            self.endpoint,
            self.encoded_path(key)?,
            percent_encode_query_value(upload_id)
        );
        let response = self
            .send_signed(Method::DELETE, &url, Vec::new(), Vec::new())
            .await
            .with_context(|| format!("abort s3 multipart upload {key:?}"))?;
        let status = response.status();
        let body = response.text().await.context("read s3 abort response")?;
        anyhow::ensure!(
            status.is_success(),
            "s3 abort multipart upload failed with status {status}: {body}"
        );
        Ok(())
    }

    async fn send_signed(
        &self,
        method: Method,
        url: &str,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> anyhow::Result<reqwest::Response> {
        let mut request = Request::builder()
            .method(method.as_str())
            .uri(url)
            .body(body.clone())
            .context("build s3 signed request")?;
        for (name, value) in &headers {
            request.headers_mut().insert(
                http::header::HeaderName::from_bytes(name.as_bytes())
                    .context("build s3 header name")?,
                http::HeaderValue::from_str(value).context("build s3 header value")?,
            );
        }
        let sha256 = hex_sha256(&body);
        self.sign_request(&mut request, SignableBody::Precomputed(sha256), None)?;

        let mut builder = self.client.request(method, url);
        for (name, value) in request.headers() {
            builder = builder.header(name.as_str(), value);
        }
        builder
            .body(body)
            .send()
            .await
            .context("send s3 signed request")
    }

    fn sign_request<B>(
        &self,
        request: &mut Request<B>,
        body: SignableBody<'_>,
        expires_in: Option<Duration>,
    ) -> anyhow::Result<()> {
        let identity: Identity = Credentials::new(
            self.access_key_id.clone(),
            self.secret_access_key.clone(),
            None,
            None,
            "agenthub-object-store",
        )
        .into();
        let mut signing_settings = SigningSettings::default();
        signing_settings.percent_encoding_mode = PercentEncodingMode::Single;
        signing_settings.payload_checksum_kind = if expires_in.is_some() {
            PayloadChecksumKind::NoHeader
        } else {
            PayloadChecksumKind::XAmzSha256
        };
        signing_settings.signature_location = if expires_in.is_some() {
            SignatureLocation::QueryParams
        } else {
            SignatureLocation::Headers
        };
        signing_settings.expires_in = expires_in;
        signing_settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;
        signing_settings.session_token_mode = SessionTokenMode::Include;
        let signing_params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name("s3")
            .time(SystemTime::now())
            .settings(signing_settings)
            .build()
            .context("build s3 signing params")?
            .into();
        let headers = request
            .headers()
            .iter()
            .map(|(name, value)| {
                let value = value
                    .to_str()
                    .with_context(|| format!("s3 header {name} is not valid UTF-8"))?;
                Ok((name.as_str(), value))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let signable_request = SignableRequest::new(
            request.method().as_str(),
            request.uri().to_string(),
            headers.into_iter(),
            body,
        )
        .context("build s3 signable request")?;
        let (instructions, _) = sign(signable_request, &signing_params)
            .context("sign s3 request")?
            .into_parts();
        instructions.apply_to_request_http1x(request);
        Ok(())
    }

    fn encoded_path(&self, key: &str) -> anyhow::Result<String> {
        let key = normalize_object_key(key)?;
        let path = if self.root == "/" {
            format!("/{key}")
        } else {
            format!("{}{key}", self.root)
        };
        Ok(percent_encode_path(&path))
    }
}

#[cfg(feature = "s3")]
const S3_PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[cfg(feature = "s3")]
fn s3_path_style_endpoint(
    endpoint: Option<&str>,
    bucket: &str,
    region: &str,
) -> anyhow::Result<String> {
    let mut endpoint = endpoint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "https://s3.amazonaws.com".to_string());
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        endpoint = format!("https://{endpoint}");
    }
    endpoint = endpoint.replace(&format!("//{bucket}."), "//");
    endpoint = endpoint.trim_end_matches('/').to_string();
    if endpoint == "https://s3.amazonaws.com" {
        endpoint = format!("https://s3.{region}.amazonaws.com");
    }
    Ok(format!("{endpoint}/{bucket}"))
}

#[cfg(feature = "s3")]
fn normalize_s3_root(root: Option<&str>) -> String {
    let mut root = root
        .unwrap_or_default()
        .split('/')
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if !root.starts_with('/') {
        root.insert(0, '/');
    }
    if !root.ends_with('/') {
        root.push('/');
    }
    root
}

#[cfg(feature = "s3")]
fn percent_encode_path(path: &str) -> String {
    utf8_percent_encode(path, S3_PATH_ENCODE_SET).to_string()
}

#[cfg(feature = "s3")]
fn percent_encode_query_value(value: &str) -> String {
    utf8_percent_encode(value, S3_PATH_ENCODE_SET).to_string()
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
    async fn put_stored_key_bytes_writes_planned_prefixed_key_without_double_prefixing() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("tenant-a".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let planned_key = store
            .scoped_key("uploads/teams/team-1/upload-1/report.json")
            .unwrap();
        assert_eq!(
            planned_key,
            "tenant-a/uploads/teams/team-1/upload-1/report.json"
        );

        let stored = store
            .put_stored_key_bytes(&planned_key, br#"{"ok":true}"#.to_vec())
            .await
            .unwrap();

        assert_eq!(stored.key, planned_key);
        assert!(
            store
                .exists("uploads/teams/team-1/upload-1/report.json")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn stored_key_writer_writes_chunks_and_reports_digest() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("tenant-a".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();
        let planned_key = store
            .scoped_key("uploads/teams/team-1/upload-1/report.txt")
            .unwrap();

        let mut writer = store.stored_key_writer(&planned_key).await.unwrap();
        writer.write_chunk(b"hello ".to_vec()).await.unwrap();
        writer.write_chunk(Vec::new()).await.unwrap();
        writer.write_chunk(b"world".to_vec()).await.unwrap();
        let stored = writer.finish().await.unwrap();

        assert_eq!(stored.key, planned_key);
        assert_eq!(stored.size_bytes, 11);
        assert_eq!(
            stored.sha256,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        assert_eq!(
            store.read_stored_key_bytes(&stored.key).await.unwrap(),
            b"hello world".to_vec()
        );
    }

    #[tokio::test]
    async fn fs_store_rejects_presigned_stored_key_writes() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("tenant-a".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let err = store
            .presign_stored_key_write(
                "tenant-a/uploads/teams/team-1/upload-1/report.txt",
                Duration::from_secs(60),
            )
            .await
            .expect_err("fs should not expose direct presigned writes");
        assert!(
            err.to_string()
                .contains("presigned object writes require the s3 backend")
        );
    }

    #[tokio::test]
    async fn fs_store_rejects_s3_multipart_uploads() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("tenant-a".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();

        let err = store
            .initiate_stored_key_multipart_upload(
                "tenant-a/uploads/teams/team-1/upload-1/report.txt",
                Some("text/plain"),
            )
            .await
            .expect_err("fs should not expose s3 multipart uploads");
        assert!(
            err.to_string().contains("s3 multipart uploads require"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn inspect_stored_key_streams_size_and_digest() {
        let dir = tempfile::tempdir().unwrap();
        let store = AgentHubObjectStore::from_settings(ObjectStoreSettings {
            backend: ObjectStoreBackend::Fs,
            root: Some(dir.path().to_string_lossy().to_string()),
            prefix: Some("tenant-a".to_string()),
            ..ObjectStoreSettings::default()
        })
        .unwrap();
        let planned_key = store
            .scoped_key("uploads/teams/team-1/upload-1/report.txt")
            .unwrap();

        store
            .put_stored_key_chunks(&planned_key, vec![b"hello ".to_vec(), b"world".to_vec()])
            .await
            .unwrap();

        let object = store.inspect_stored_key(&planned_key).await.unwrap();
        assert_eq!(object.key, planned_key);
        assert_eq!(object.size_bytes, 11);
        assert_eq!(
            object.sha256,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
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
    async fn s3_compatible_store_exercises_bytes_and_hosted_images() {
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
        let signed = store
            .presign_stored_key_write(&stored.key, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(signed.method, "PUT");
        assert!(!signed.uri.is_empty());
        assert_eq!(signed.expires_in_seconds, 60);

        let multipart_key = store
            .scoped_key("uploads/teams/team-1/multipart/report.txt")
            .unwrap();
        let multipart = store
            .initiate_stored_key_multipart_upload(&multipart_key, Some("text/plain"))
            .await
            .unwrap();
        assert_eq!(multipart.key, multipart_key);
        assert!(!multipart.upload_id.is_empty());
        let presigned_part = store
            .presign_stored_key_multipart_upload_part(
                &multipart.key,
                &multipart.upload_id,
                1,
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        assert_eq!(presigned_part.part_number, 1);
        assert_eq!(presigned_part.method, "PUT");
        let mut request = reqwest::Client::new().put(&presigned_part.uri);
        for (name, value) in &presigned_part.headers {
            request = request.header(name, value);
        }
        let part_response = request
            .body("multipart hello".as_bytes().to_vec())
            .send()
            .await
            .unwrap();
        let part_status = part_response.status();
        let etag = part_response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        assert!(
            part_status.is_success(),
            "unexpected upload part status: {part_status}"
        );
        store
            .complete_stored_key_multipart_upload(
                &multipart.key,
                &multipart.upload_id,
                vec![MultipartUploadPart {
                    part_number: 1,
                    etag: etag.expect("upload part etag"),
                }],
            )
            .await
            .unwrap();
        let multipart_object = store.inspect_stored_key(&multipart.key).await.unwrap();
        assert_eq!(multipart_object.size_bytes, "multipart hello".len() as u64);
        assert_eq!(
            multipart_object.sha256,
            hex_sha256("multipart hello".as_bytes())
        );

        let aborted_key = store
            .scoped_key("uploads/teams/team-1/multipart/aborted.txt")
            .unwrap();
        let aborted = store
            .initiate_stored_key_multipart_upload(&aborted_key, Some("text/plain"))
            .await
            .unwrap();
        store
            .abort_stored_key_multipart_upload(&aborted.key, &aborted.upload_id)
            .await
            .unwrap();

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
        assert!(
            !store
                .exists("images/agents/agent-1/image-1.png")
                .await
                .unwrap()
        );
    }
}
