pub mod path_utils;

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use path_utils::expand_tilde;

const DEFAULT_SAFE_PATH: &str = "~/.agenthub/worktrees";
const DEFAULT_CODEX_ACP_MODE: &str = "full-access";
const DEFAULT_HISTORY_EVENT_RETENTION_DAYS: u32 = 5;
const DEFAULT_HISTORY_DELETE_BATCH_SIZE: u32 = 10_000;
const MAIN_SERVER_NODE_ID: &str = "main";

#[derive(Debug, Clone)]
pub struct ConfigLoadInfo {
    pub path: std::path::PathBuf,
    pub file_exists: bool,
    pub env_overrides: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AppConfig {
    pub server: Option<ServerConfig>,
    pub web: Option<WebConfig>,
    pub proxy: Option<ProxyConfig>,
    pub worktree: Option<WorktreeConfig>,
    pub codex_acp: Option<CodexAcpConfig>,
    pub nowledge_mem: Option<NowledgeMemConfig>,
    pub history: Option<HistoryConfig>,
    pub message_archive: Option<MessageArchiveConfig>,
    pub message_body_store: Option<MessageBodyStoreConfig>,
    pub object_store: Option<ObjectStoreConfig>,
    pub push: Option<PushConfig>,
    pub internal_grpc: Option<InternalGrpcConfig>,
    pub safe_paths: Option<Vec<String>>,
    pub web_dir: Option<String>,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen: Option<String>,
    pub role: Option<ServerRole>,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerRole {
    #[default]
    Main,
    Node,
}

impl ServerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Node => "node",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    pub rp_id: Option<String>,
    pub rp_origin: Option<String>,
    pub rp_name: Option<String>,
    pub passkey_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub http: Option<String>,
    pub https: Option<String>,
    pub all: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeConfig {
    pub default_root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexAcpConfig {
    pub binary: Option<String>,
    pub default_mode: Option<String>,
    pub multi_agent_enabled: Option<bool>,
}

/// Secret-free connection profiles for the local Team Context Lens adapter.
/// `credential_env` is an environment-variable reference, never a credential.
#[derive(Debug, Clone, Deserialize)]
pub struct NowledgeMemConfig {
    pub profiles: Option<HashMap<String, NowledgeMemProfileConfig>>,
    pub team_bindings: Option<HashMap<String, NowledgeMemTeamBindingConfig>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct NowledgeMemProfileConfig {
    pub endpoint: String,
    pub credential_env: String,
    pub tool_set: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NowledgeMemTeamBindingConfig {
    pub profile: String,
    pub space_id: String,
    pub actor_profiles: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNowledgeMemBinding {
    pub profile_name: String,
    pub profile: NowledgeMemProfileConfig,
    pub space_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PushConfig {
    pub subject: Option<String>,
    pub keys_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryConfig {
    pub event_retention_days: Option<u32>,
    pub vacuum_on_cleanup: Option<bool>,
    pub delete_batch_size: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageArchiveConfig {
    pub backend: Option<String>,
    pub uri: Option<String>,
    pub message_table: Option<String>,
}

/// Tiered message body store (compresses message bodies into RocksDB). It is opt-in: set
/// `enabled = true` after building with the `rocksdb` feature.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageBodyStoreConfig {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    /// Whether to backfill existing SQLite compatibility bodies into the store on startup. Default-on
    /// once the store itself is enabled; set `auto_migrate = false` to use `agenthub migrate`.
    pub auto_migrate: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectStoreConfig {
    pub backend: Option<String>,
    pub root: Option<String>,
    pub public_base_url: Option<String>,
    pub prefix: Option<String>,
    pub bucket: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key_id_env: Option<String>,
    pub secret_access_key_env: Option<String>,
    pub download_max_bytes: Option<u64>,
    pub download_max_redirects: Option<u8>,
    pub download_timeout_seconds: Option<u64>,
    pub download_allow_private_networks: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InternalGrpcConfig {
    pub enabled: Option<bool>,
    pub listen: Option<String>,
    pub security: Option<InternalGrpcSecurityConfig>,
    pub auth: Option<InternalGrpcAuthConfig>,
    pub bootstrap: Option<InternalGrpcBootstrapConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InternalGrpcSecurityConfig {
    pub mode: Option<String>,
    pub cert_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InternalGrpcAuthConfig {
    pub shared_secret: Option<String>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InternalGrpcBootstrapConfig {
    pub token: Option<String>,
}

impl AppConfig {
    pub fn nowledge_mem_profile(&self, name: &str) -> anyhow::Result<NowledgeMemProfileConfig> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("Nowledge Mem profile name is required");
        }
        let profile = self
            .nowledge_mem
            .as_ref()
            .and_then(|config| config.profiles.as_ref())
            .and_then(|profiles| profiles.get(name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Nowledge Mem profile {name:?} is not configured"))?;
        if profile.endpoint.trim().is_empty() {
            anyhow::bail!("Nowledge Mem profile {name:?} has an empty endpoint");
        }
        if profile.credential_env.trim().is_empty() {
            anyhow::bail!("Nowledge Mem profile {name:?} has an empty credential_env");
        }
        Ok(profile)
    }

    pub fn resolve_nowledge_mem_binding(
        &self,
        team_id: &str,
        actor_id: &str,
    ) -> anyhow::Result<ResolvedNowledgeMemBinding> {
        let team_id = team_id.trim();
        if team_id.is_empty() {
            anyhow::bail!("team_id is required");
        }
        let actor_id = actor_id.trim();
        if actor_id.is_empty() {
            anyhow::bail!("actor_id is required");
        }
        let binding = self
            .nowledge_mem
            .as_ref()
            .and_then(|config| config.team_bindings.as_ref())
            .and_then(|bindings| bindings.get(team_id))
            .ok_or_else(|| {
                anyhow::anyhow!("Nowledge Mem binding for team {team_id:?} is not configured")
            })?;
        let profile_name = binding
            .actor_profiles
            .as_ref()
            .and_then(|profiles| profiles.get(actor_id))
            .unwrap_or(&binding.profile)
            .trim()
            .to_string();
        let space_id = binding.space_id.trim().to_string();
        if space_id.is_empty() {
            anyhow::bail!("Nowledge Mem binding for team {team_id:?} has no space_id");
        }
        Ok(ResolvedNowledgeMemBinding {
            profile: self.nowledge_mem_profile(&profile_name)?,
            profile_name,
            space_id,
        })
    }

    pub fn load_with_info() -> anyhow::Result<(Self, ConfigLoadInfo)> {
        let path = config_path();
        let file_exists = path.exists();
        let config = if file_exists {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str::<AppConfig>(&content)?
        } else {
            Self::default()
        };
        let info = ConfigLoadInfo {
            path,
            file_exists,
            env_overrides: detect_env_overrides(),
        };
        Ok((config, info))
    }
}

pub fn config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".agenthub/config.toml")
}

impl AppConfig {
    pub fn server_role(&self) -> ServerRole {
        self.server
            .as_ref()
            .and_then(|server| server.role)
            .unwrap_or_default()
    }

    pub fn is_node_role(&self) -> bool {
        self.server_role() == ServerRole::Node
    }

    pub fn configured_server_node_id(&self) -> Option<String> {
        self.server
            .as_ref()
            .and_then(|server| server.node_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    }

    pub fn server_node_id(&self) -> anyhow::Result<String> {
        match self.server_role() {
            ServerRole::Main => Ok(MAIN_SERVER_NODE_ID.to_string()),
            ServerRole::Node => {
                let node_id = self.configured_server_node_id().ok_or_else(|| {
                    anyhow::anyhow!("server.node_id is required when server.role = \"node\"")
                })?;
                if node_id == MAIN_SERVER_NODE_ID {
                    anyhow::bail!(
                        "server.node_id must not be \"{}\" when server.role = \"node\"",
                        MAIN_SERVER_NODE_ID
                    );
                }
                Ok(node_id)
            }
        }
    }

    pub fn default_worktree_root(&self) -> String {
        self.worktree
            .as_ref()
            .and_then(|w| w.default_root.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("~/.agenthub/worktrees")
            .to_string()
    }

    pub fn effective_web_dir(&self) -> Option<String> {
        if cfg!(debug_assertions) {
            let dir = self
                .web_dir
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("web/dist");
            return Some(dir.to_string());
        }
        None
    }

    pub fn listen_addr(&self) -> String {
        self.server
            .as_ref()
            .and_then(|s| s.listen.clone())
            .unwrap_or_else(|| "127.0.0.1:8080".to_string())
    }

    pub fn rp_id(&self) -> String {
        self.web
            .as_ref()
            .and_then(|w| w.rp_id.clone())
            .unwrap_or_else(|| "localhost".to_string())
    }

    pub fn rp_origin(&self) -> String {
        self.web
            .as_ref()
            .and_then(|w| w.rp_origin.clone())
            .unwrap_or_else(|| "http://localhost:8080".to_string())
    }

    pub fn rp_name(&self) -> String {
        self.web
            .as_ref()
            .and_then(|w| w.rp_name.clone())
            .unwrap_or_else(|| "AgentHub".to_string())
    }

    pub fn passkey_enabled(&self) -> bool {
        self.web
            .as_ref()
            .and_then(|w| w.passkey_enabled)
            .unwrap_or(false)
    }

    pub fn safe_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        paths.push(expand_tilde(DEFAULT_SAFE_PATH));
        if let Some(configured_paths) = &self.safe_paths {
            for path in configured_paths {
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    paths.push(expand_tilde(trimmed));
                }
            }
        }

        let mut seen = HashSet::new();
        paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect()
    }

    pub fn log_path(&self) -> Option<String> {
        self.log_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(expand_tilde)
    }

    pub fn codex_acp_binary(&self) -> String {
        self.codex_acp
            .as_ref()
            .and_then(|c| c.binary.clone())
            .unwrap_or_else(|| "agenthub-acp".to_string())
    }

    pub fn codex_acp_default_mode(&self) -> Option<String> {
        let configured = self
            .codex_acp
            .as_ref()
            .and_then(|c| c.default_mode.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_codex_acp_mode_id);
        Some(configured.unwrap_or_else(|| DEFAULT_CODEX_ACP_MODE.to_string()))
    }

    pub fn codex_acp_multi_agent_enabled(&self) -> bool {
        self.codex_acp
            .as_ref()
            .and_then(|c| c.multi_agent_enabled)
            .unwrap_or(true)
    }

    pub fn message_archive_backend(&self) -> String {
        self.message_archive
            .as_ref()
            .and_then(|config| config.backend.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("lancedb")
            .to_ascii_lowercase()
    }

    pub fn message_archive_uri(&self) -> String {
        let uri = self
            .message_archive
            .as_ref()
            .and_then(|config| config.uri.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("~/.agenthub/message-archive");
        expand_tilde(uri)
    }

    pub fn message_archive_table(&self) -> String {
        self.message_archive
            .as_ref()
            .and_then(|config| config.message_table.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("messages")
            .to_string()
    }

    /// Whether the tiered message body store is enabled. It is opt-in, and the build must include the
    /// `rocksdb` feature for the store to actually be constructed.
    pub fn message_body_store_enabled(&self) -> bool {
        self.message_body_store
            .as_ref()
            .and_then(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Filesystem path for the message body store. Defaults next to the other agenthub data.
    pub fn message_body_store_path(&self) -> String {
        let path = self
            .message_body_store
            .as_ref()
            .and_then(|config| config.path.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("~/.agenthub/message-bodies");
        expand_tilde(path)
    }

    /// Whether to backfill existing SQLite compatibility bodies into the store on startup. Default-on
    /// once the store is enabled; only an explicit `auto_migrate = false` disables it.
    pub fn message_body_store_auto_migrate(&self) -> bool {
        self.message_body_store
            .as_ref()
            .and_then(|config| config.auto_migrate)
            .unwrap_or(true)
    }

    pub fn object_store_backend(&self) -> String {
        self.object_store
            .as_ref()
            .and_then(|config| config.backend.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("fs")
            .to_ascii_lowercase()
    }

    pub fn object_store_root(&self) -> Option<String> {
        self.object_store
            .as_ref()
            .and_then(|config| config.root.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(expand_tilde)
    }

    pub fn object_store_prefix(&self) -> Option<String> {
        self.object_store
            .as_ref()
            .and_then(|config| config.prefix.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.trim_matches('/').to_string())
            .filter(|value| !value.is_empty())
    }

    pub fn object_store_public_base_url(&self) -> Option<String> {
        trimmed_object_store_value(
            self.object_store
                .as_ref()
                .and_then(|config| config.public_base_url.as_deref()),
        )
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
    }

    pub fn object_store_bucket(&self) -> Option<String> {
        trimmed_object_store_value(
            self.object_store
                .as_ref()
                .and_then(|config| config.bucket.as_deref()),
        )
    }

    pub fn object_store_endpoint(&self) -> Option<String> {
        trimmed_object_store_value(
            self.object_store
                .as_ref()
                .and_then(|config| config.endpoint.as_deref()),
        )
    }

    pub fn object_store_region(&self) -> Option<String> {
        trimmed_object_store_value(
            self.object_store
                .as_ref()
                .and_then(|config| config.region.as_deref()),
        )
    }

    pub fn object_store_access_key_id_env(&self) -> Option<String> {
        trimmed_object_store_value(
            self.object_store
                .as_ref()
                .and_then(|config| config.access_key_id_env.as_deref()),
        )
    }

    pub fn object_store_secret_access_key_env(&self) -> Option<String> {
        trimmed_object_store_value(
            self.object_store
                .as_ref()
                .and_then(|config| config.secret_access_key_env.as_deref()),
        )
    }

    pub fn object_store_download_max_bytes(&self) -> u64 {
        self.object_store
            .as_ref()
            .and_then(|config| config.download_max_bytes)
            .filter(|value| *value > 0)
            .unwrap_or(512 * 1024 * 1024)
    }

    pub fn object_store_download_max_redirects(&self) -> u8 {
        self.object_store
            .as_ref()
            .and_then(|config| config.download_max_redirects)
            .unwrap_or(5)
    }

    pub fn object_store_download_timeout_seconds(&self) -> u64 {
        self.object_store
            .as_ref()
            .and_then(|config| config.download_timeout_seconds)
            .filter(|value| *value > 0)
            .unwrap_or(120)
    }

    pub fn object_store_download_allow_private_networks(&self) -> bool {
        self.object_store
            .as_ref()
            .and_then(|config| config.download_allow_private_networks)
            .unwrap_or(false)
    }

    pub fn history_event_retention_days(&self) -> Option<u32> {
        let days = self
            .history
            .as_ref()
            .and_then(|history| history.event_retention_days)
            .unwrap_or(DEFAULT_HISTORY_EVENT_RETENTION_DAYS);
        if days == 0 {
            return None;
        }
        Some(days)
    }

    pub fn history_vacuum_on_cleanup(&self) -> bool {
        self.history
            .as_ref()
            .and_then(|history| history.vacuum_on_cleanup)
            .unwrap_or(false)
    }

    pub fn history_delete_batch_size(&self) -> u32 {
        let configured = self
            .history
            .as_ref()
            .and_then(|history| history.delete_batch_size)
            .unwrap_or(DEFAULT_HISTORY_DELETE_BATCH_SIZE);
        configured.clamp(100, 200_000)
    }

    pub fn internal_grpc_enabled(&self) -> bool {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.enabled)
            .unwrap_or(false)
    }

    pub fn internal_grpc_listen_addr(&self) -> String {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.listen.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "127.0.0.1:50051".to_string())
    }

    pub fn internal_grpc_security_mode(&self) -> String {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.security.as_ref())
            .and_then(|security| security.mode.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| "tls".to_string())
    }

    pub fn internal_grpc_cert_dir(&self) -> String {
        let configured = self
            .internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.security.as_ref())
            .and_then(|security| security.cert_dir.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(expand_tilde);
        if let Some(path) = configured {
            return path;
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::Path::new(&home)
            .join(".agenthub/internal-grpc")
            .to_string_lossy()
            .to_string()
    }

    pub fn internal_grpc_auth_shared_secret(&self) -> Option<String> {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.auth.as_ref())
            .and_then(|auth| auth.shared_secret.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    }

    pub fn internal_grpc_auth_issuer(&self) -> Option<String> {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.auth.as_ref())
            .and_then(|auth| auth.issuer.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    }

    pub fn internal_grpc_auth_audience(&self) -> Option<String> {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.auth.as_ref())
            .and_then(|auth| auth.audience.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    }

    pub fn internal_grpc_bootstrap_token(&self) -> Option<String> {
        self.internal_grpc
            .as_ref()
            .and_then(|cfg| cfg.bootstrap.as_ref())
            .and_then(|bootstrap| bootstrap.token.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    }

    pub fn proxy_env(&self) -> Vec<(String, String)> {
        let mut items = Vec::new();
        if let Some(proxy) = &self.proxy {
            if let Some(val) = proxy.http.clone() {
                items.push(("HTTP_PROXY".to_string(), val.clone()));
                items.push(("http_proxy".to_string(), val));
            }
            if let Some(val) = proxy.https.clone() {
                items.push(("HTTPS_PROXY".to_string(), val.clone()));
                items.push(("https_proxy".to_string(), val));
            }
            if let Some(val) = proxy.all.clone() {
                items.push(("ALL_PROXY".to_string(), val.clone()));
                items.push(("all_proxy".to_string(), val));
            }
        }
        items
    }

    pub fn vapid_subject(&self) -> String {
        self.push
            .as_ref()
            .and_then(|p| p.subject.clone())
            .unwrap_or_else(|| "mailto:admin@example.com".to_string())
    }

    pub fn vapid_keys_path(&self) -> std::path::PathBuf {
        if let Some(path) = self.push.as_ref().and_then(|p| p.keys_path.clone()) {
            return std::path::PathBuf::from(path);
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::Path::new(&home).join(".agenthub/vapid.json")
    }
}

fn trimmed_object_store_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn normalize_codex_acp_mode_id(mode_id: &str) -> String {
    match mode_id.trim() {
        "yolo" | "yalo" | "danger_full_access" | "danger-full-access" => "full-access".to_string(),
        value => value.to_string(),
    }
}

pub fn normalize_optional_codex_acp_mode_id(mode_id: Option<&str>) -> Option<String> {
    mode_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_codex_acp_mode_id)
}

/// Provider-neutral thinking levels for the Agent Runtime Profiles feature, ordered low → high.
pub const AGENT_THINKING_LEVELS: [&str; 4] = ["low", "medium", "high", "max"];

/// Normalize an operator-provided runtime model override: trim, and treat blank as unset. Unknown model
/// names are intentionally allowed — provider availability drifts independently of AgentHub releases.
pub fn normalize_optional_runtime_model(model: Option<&str>) -> Option<String> {
    model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Normalize a runtime thinking level: trim, lowercase, and keep only the constrained enum
/// (`low|medium|high|max`). Blank is unset; an unrecognized value yields `None` so callers can reject
/// it. Stored provider-neutrally — the launch translation maps it to provider-specific options.
pub fn normalize_optional_thinking_level(level: Option<&str>) -> Option<String> {
    let normalized = level.map(str::trim).filter(|value| !value.is_empty())?;
    let lowered = normalized.to_ascii_lowercase();
    AGENT_THINKING_LEVELS
        .iter()
        .find(|candidate| **candidate == lowered)
        .map(|candidate| (*candidate).to_string())
}

/// Whether a thinking level string is a recognized enum value (ignoring case/whitespace). Used by the
/// API layer to reject invalid input rather than silently dropping it.
pub fn is_valid_thinking_level(level: &str) -> bool {
    normalize_optional_thinking_level(Some(level)).is_some()
}

fn detect_env_overrides() -> Vec<String> {
    let keys = [
        "AGENTHUB_LISTEN",
        "AGENTHUB_SERVER_ROLE",
        "AGENTHUB_SERVER_NODE_ID",
        "AGENTHUB_RP_ID",
        "AGENTHUB_RP_ORIGIN",
        "AGENTHUB_RP_NAME",
        "AGENTHUB_PASSKEY_ENABLED",
        "AGENTHUB_SAFE_PATHS",
        "AGENTHUB_WEB_DIR",
        "AGENTHUB_LOG_PATH",
        "AGENTHUB_CODEX_ACP_BINARY",
        "AGENTHUB_CODEX_ACP_DEFAULT_MODE",
        "AGENTHUB_HISTORY_EVENT_RETENTION_DAYS",
        "AGENTHUB_HISTORY_VACUUM_ON_CLEANUP",
        "AGENTHUB_INTERNAL_GRPC_ENABLED",
        "AGENTHUB_INTERNAL_GRPC_LISTEN",
        "AGENTHUB_INTERNAL_GRPC_SECURITY_MODE",
        "AGENTHUB_INTERNAL_GRPC_CERT_DIR",
        "AGENTHUB_INTERNAL_GRPC_AUTH_SHARED_SECRET",
        "AGENTHUB_INTERNAL_GRPC_AUTH_ISSUER",
        "AGENTHUB_INTERNAL_GRPC_AUTH_AUDIENCE",
        "AGENTHUB_INTERNAL_GRPC_BOOTSTRAP_TOKEN",
        "AGENTHUB_HTTP_PROXY",
        "AGENTHUB_HTTPS_PROXY",
        "AGENTHUB_ALL_PROXY",
        "AGENTHUB_VAPID_SUBJECT",
        "AGENTHUB_VAPID_PUBLIC_KEY",
        "AGENTHUB_VAPID_PRIVATE_KEY",
    ];
    keys.iter()
        .filter(|key| std::env::var(key).ok().filter(|v| !v.is_empty()).is_some())
        .map(|key| key.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, CodexAcpConfig, HistoryConfig, MessageArchiveConfig, MessageBodyStoreConfig,
        NowledgeMemConfig, NowledgeMemProfileConfig, NowledgeMemTeamBindingConfig,
        ObjectStoreConfig, ServerConfig, ServerRole, WorktreeConfig,
    };

    #[test]
    fn message_body_store_is_opt_in() {
        assert!(!AppConfig::default().message_body_store_enabled());
        let config = AppConfig {
            message_body_store: Some(MessageBodyStoreConfig {
                enabled: Some(true),
                path: None,
                auto_migrate: None,
            }),
            ..Default::default()
        };
        assert!(config.message_body_store_enabled());
    }

    #[test]
    fn nowledge_mem_profile_returns_secret_free_reference() {
        let config = AppConfig {
            nowledge_mem: Some(NowledgeMemConfig {
                profiles: Some(std::collections::HashMap::from([(
                    "team".to_string(),
                    NowledgeMemProfileConfig {
                        endpoint: "https://mem.example.test/mcp".to_string(),
                        credential_env: "NOWLEDGE_MEM_TEAM_KEY".to_string(),
                        tool_set: Some("external-agent".to_string()),
                    },
                )])),
                team_bindings: None,
            }),
            ..Default::default()
        };

        let profile = config
            .nowledge_mem_profile("team")
            .expect("configured profile");
        assert_eq!(profile.credential_env, "NOWLEDGE_MEM_TEAM_KEY");
        assert!(config.nowledge_mem_profile("missing").is_err());
        assert!(config.nowledge_mem_profile(" ").is_err());
    }

    #[test]
    fn resolve_nowledge_mem_binding_uses_actor_override_without_expanding_scope() {
        let config = AppConfig {
            nowledge_mem: Some(NowledgeMemConfig {
                profiles: Some(std::collections::HashMap::from([
                    (
                        "team".to_string(),
                        NowledgeMemProfileConfig {
                            endpoint: "https://mem.example.test/mcp".to_string(),
                            credential_env: "NOWLEDGE_MEM_TEAM_KEY".to_string(),
                            tool_set: Some("external-agent".to_string()),
                        },
                    ),
                    (
                        "worker".to_string(),
                        NowledgeMemProfileConfig {
                            endpoint: "https://mem.example.test/mcp".to_string(),
                            credential_env: "NOWLEDGE_MEM_WORKER_KEY".to_string(),
                            tool_set: Some("external-agent".to_string()),
                        },
                    ),
                ])),
                team_bindings: Some(std::collections::HashMap::from([(
                    "team-1".to_string(),
                    NowledgeMemTeamBindingConfig {
                        profile: "team".to_string(),
                        space_id: "space-1".to_string(),
                        actor_profiles: Some(std::collections::HashMap::from([(
                            "worker-1".to_string(),
                            "worker".to_string(),
                        )])),
                    },
                )])),
            }),
            ..Default::default()
        };

        let worker = config
            .resolve_nowledge_mem_binding("team-1", "worker-1")
            .expect("worker binding");
        assert_eq!(worker.profile_name, "worker");
        assert_eq!(worker.profile.credential_env, "NOWLEDGE_MEM_WORKER_KEY");
        assert_eq!(worker.space_id, "space-1");

        let team_default = config
            .resolve_nowledge_mem_binding("team-1", "worker-2")
            .expect("team default binding");
        assert_eq!(team_default.profile_name, "team");
        assert_eq!(team_default.space_id, "space-1");
        assert!(
            config
                .resolve_nowledge_mem_binding(" ", "worker-1")
                .is_err()
        );
        assert!(config.resolve_nowledge_mem_binding("team-1", " ").is_err());
    }

    #[test]
    fn message_body_store_auto_migrate_defaults_on() {
        assert!(AppConfig::default().message_body_store_auto_migrate());
        let config = AppConfig {
            message_body_store: Some(MessageBodyStoreConfig {
                enabled: Some(true),
                path: None,
                auto_migrate: None,
            }),
            ..Default::default()
        };
        assert!(config.message_body_store_auto_migrate());
    }

    #[test]
    fn message_body_store_auto_migrate_can_be_disabled() {
        let config = AppConfig {
            message_body_store: Some(MessageBodyStoreConfig {
                enabled: Some(true),
                path: None,
                auto_migrate: Some(false),
            }),
            ..Default::default()
        };
        assert!(!config.message_body_store_auto_migrate());
    }

    #[test]
    fn object_store_defaults_to_local_fs_backend() {
        let config = AppConfig::default();
        assert_eq!(config.object_store_backend(), "fs");
        assert_eq!(config.object_store_root(), None);
        assert_eq!(config.object_store_prefix(), None);
        assert_eq!(config.object_store_bucket(), None);
        assert_eq!(config.object_store_download_max_bytes(), 512 * 1024 * 1024);
        assert_eq!(config.object_store_download_max_redirects(), 5);
        assert_eq!(config.object_store_download_timeout_seconds(), 120);
        assert!(!config.object_store_download_allow_private_networks());
    }

    #[test]
    fn object_store_config_trims_secret_free_s3_references() {
        let config = AppConfig {
            object_store: Some(ObjectStoreConfig {
                backend: Some(" S3 ".to_string()),
                root: Some(" /tenant-root/ ".to_string()),
                public_base_url: Some(" https://img.example.test/ ".to_string()),
                prefix: Some("/agenthub/prod/".to_string()),
                bucket: Some(" agenthub-artifacts ".to_string()),
                endpoint: Some(" https://s3.example.test ".to_string()),
                region: Some(" auto ".to_string()),
                access_key_id_env: Some(" AGENTHUB_OBJECT_STORE_ACCESS_KEY_ID ".to_string()),
                secret_access_key_env: Some(
                    " AGENTHUB_OBJECT_STORE_SECRET_ACCESS_KEY ".to_string(),
                ),
                download_max_bytes: Some(1024),
                download_max_redirects: Some(2),
                download_timeout_seconds: Some(9),
                download_allow_private_networks: Some(true),
            }),
            ..Default::default()
        };

        assert_eq!(config.object_store_backend(), "s3");
        assert_eq!(config.object_store_root().as_deref(), Some("/tenant-root/"));
        assert_eq!(
            config.object_store_public_base_url().as_deref(),
            Some("https://img.example.test")
        );
        assert_eq!(
            config.object_store_prefix().as_deref(),
            Some("agenthub/prod")
        );
        assert_eq!(
            config.object_store_bucket().as_deref(),
            Some("agenthub-artifacts")
        );
        assert_eq!(
            config.object_store_endpoint().as_deref(),
            Some("https://s3.example.test")
        );
        assert_eq!(config.object_store_region().as_deref(), Some("auto"));
        assert_eq!(
            config.object_store_access_key_id_env().as_deref(),
            Some("AGENTHUB_OBJECT_STORE_ACCESS_KEY_ID")
        );
        assert_eq!(
            config.object_store_secret_access_key_env().as_deref(),
            Some("AGENTHUB_OBJECT_STORE_SECRET_ACCESS_KEY")
        );
        assert_eq!(config.object_store_download_max_bytes(), 1024);
        assert_eq!(config.object_store_download_max_redirects(), 2);
        assert_eq!(config.object_store_download_timeout_seconds(), 9);
        assert!(config.object_store_download_allow_private_networks());
    }

    #[test]
    fn server_role_defaults_to_main() {
        let config = AppConfig::default();
        assert_eq!(config.server_role(), ServerRole::Main);
        assert_eq!(config.server_node_id().expect("default node id"), "main");
    }

    #[test]
    fn node_role_requires_non_main_node_id() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            config
                .server_node_id()
                .expect_err("node role should require node id")
                .to_string(),
            "server.node_id is required when server.role = \"node\""
        );
    }

    #[test]
    fn node_role_rejects_main_node_id() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: Some(" main ".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(
            config
                .server_node_id()
                .expect_err("node role should reject main node id")
                .to_string(),
            "server.node_id must not be \"main\" when server.role = \"node\""
        );
    }

    #[test]
    fn node_role_uses_trimmed_node_id() {
        let config = AppConfig {
            server: Some(ServerConfig {
                listen: None,
                role: Some(ServerRole::Node),
                node_id: Some(" node-east ".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(config.server_role(), ServerRole::Node);
        assert_eq!(
            config.server_node_id().expect("node mode node id"),
            "node-east"
        );
    }

    #[test]
    fn default_worktree_root_uses_builtin_default() {
        let config = AppConfig::default();
        assert_eq!(config.default_worktree_root(), "~/.agenthub/worktrees");
    }

    #[test]
    fn default_worktree_root_uses_configured_value() {
        let config = AppConfig {
            worktree: Some(WorktreeConfig {
                default_root: Some("/tmp/custom-worktrees".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(config.default_worktree_root(), "/tmp/custom-worktrees");
    }

    #[test]
    fn default_worktree_root_trims_blank_value() {
        let config = AppConfig {
            worktree: Some(WorktreeConfig {
                default_root: Some("   ".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(config.default_worktree_root(), "~/.agenthub/worktrees");
    }

    #[test]
    fn safe_paths_includes_default_worktrees_path() {
        let config = AppConfig::default();
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        assert_eq!(
            config.safe_paths(),
            vec![format!("{home}/.agenthub/worktrees")]
        );
    }

    #[test]
    fn safe_paths_merges_configured_paths_and_deduplicates() {
        let config = AppConfig {
            safe_paths: Some(vec![
                " /tmp/a ".to_string(),
                "~/.agenthub/worktrees".to_string(),
                "".to_string(),
                "/tmp/a".to_string(),
            ]),
            ..Default::default()
        };
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        assert_eq!(
            config.safe_paths(),
            vec![format!("{home}/.agenthub/worktrees"), "/tmp/a".to_string()]
        );
    }

    #[test]
    fn internal_grpc_defaults_are_stable() {
        let config = AppConfig::default();
        assert!(!config.internal_grpc_enabled());
        assert_eq!(config.internal_grpc_listen_addr(), "127.0.0.1:50051");
        assert_eq!(config.internal_grpc_security_mode(), "tls");
        assert!(config.internal_grpc_auth_shared_secret().is_none());
        assert!(config.internal_grpc_auth_issuer().is_none());
        assert!(config.internal_grpc_auth_audience().is_none());
        assert!(config.internal_grpc_bootstrap_token().is_none());
    }

    #[test]
    fn history_defaults_use_five_days_and_no_vacuum() {
        let config = AppConfig::default();
        assert_eq!(config.history_event_retention_days(), Some(5));
        assert!(!config.history_vacuum_on_cleanup());
        assert_eq!(config.history_delete_batch_size(), 10_000);
    }

    #[test]
    fn history_config_applies_custom_values() {
        let config = AppConfig {
            history: Some(HistoryConfig {
                event_retention_days: Some(14),
                vacuum_on_cleanup: Some(true),
                delete_batch_size: Some(5000),
            }),
            ..Default::default()
        };
        assert_eq!(config.history_event_retention_days(), Some(14));
        assert!(config.history_vacuum_on_cleanup());
        assert_eq!(config.history_delete_batch_size(), 5000);
    }

    #[test]
    fn history_retention_can_be_disabled_with_zero() {
        let config = AppConfig {
            history: Some(HistoryConfig {
                event_retention_days: Some(0),
                vacuum_on_cleanup: Some(false),
                delete_batch_size: Some(1),
            }),
            ..Default::default()
        };
        assert_eq!(config.history_event_retention_days(), None);
        assert_eq!(config.history_delete_batch_size(), 100);
    }

    #[test]
    fn codex_acp_default_mode_falls_back_to_full_access() {
        let config = AppConfig::default();
        assert_eq!(
            config.codex_acp_default_mode().as_deref(),
            Some("full-access")
        );
    }

    #[test]
    fn codex_acp_binary_defaults_to_canonical_adapter() {
        let config = AppConfig::default();
        assert_eq!(config.codex_acp_binary(), "agenthub-acp");
    }

    #[test]
    fn codex_acp_binary_preserves_configured_legacy_adapter() {
        let config = AppConfig {
            codex_acp: Some(CodexAcpConfig {
                binary: Some("agenthub-codex-acp".to_string()),
                default_mode: None,
                multi_agent_enabled: None,
            }),
            ..Default::default()
        };
        assert_eq!(config.codex_acp_binary(), "agenthub-codex-acp");
    }

    #[test]
    fn codex_acp_default_mode_preserves_configured_value() {
        let config = AppConfig {
            codex_acp: Some(CodexAcpConfig {
                binary: None,
                default_mode: Some(" full-access ".to_string()),
                multi_agent_enabled: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            config.codex_acp_default_mode().as_deref(),
            Some("full-access")
        );
    }

    #[test]
    fn codex_acp_default_mode_accepts_yolo_aliases() {
        for alias in ["yolo", "yalo", "danger_full_access", "danger-full-access"] {
            let config = AppConfig {
                codex_acp: Some(CodexAcpConfig {
                    binary: None,
                    default_mode: Some(format!(" {alias} ")),
                    multi_agent_enabled: None,
                }),
                ..Default::default()
            };
            assert_eq!(
                config.codex_acp_default_mode().as_deref(),
                Some("full-access"),
                "alias {alias} should map to Codex full access mode"
            );
        }
    }

    #[test]
    fn normalize_optional_codex_acp_mode_id_ignores_blank_and_accepts_aliases() {
        assert_eq!(super::normalize_optional_codex_acp_mode_id(None), None);
        assert_eq!(
            super::normalize_optional_codex_acp_mode_id(Some("  ")),
            None
        );
        assert_eq!(
            super::normalize_optional_codex_acp_mode_id(Some(" yolo ")).as_deref(),
            Some("full-access")
        );
        assert_eq!(
            super::normalize_optional_codex_acp_mode_id(Some(" auto ")).as_deref(),
            Some("auto")
        );
    }

    #[test]
    fn normalize_optional_runtime_model_trims_and_drops_blank() {
        assert_eq!(super::normalize_optional_runtime_model(None), None);
        assert_eq!(super::normalize_optional_runtime_model(Some("   ")), None);
        assert_eq!(
            super::normalize_optional_runtime_model(Some("  gpt-5.4-codex  ")).as_deref(),
            Some("gpt-5.4-codex")
        );
        // Unknown model names are allowed: provider availability drifts independently.
        assert_eq!(
            super::normalize_optional_runtime_model(Some("some-future-model")).as_deref(),
            Some("some-future-model")
        );
    }

    #[test]
    fn normalize_optional_thinking_level_keeps_only_the_enum() {
        assert_eq!(super::normalize_optional_thinking_level(None), None);
        assert_eq!(super::normalize_optional_thinking_level(Some(" ")), None);
        for level in ["low", "medium", "high", "max"] {
            assert_eq!(
                super::normalize_optional_thinking_level(Some(level)).as_deref(),
                Some(level)
            );
        }
        // Case-insensitive + trimmed.
        assert_eq!(
            super::normalize_optional_thinking_level(Some("  HIGH ")).as_deref(),
            Some("high")
        );
        // Unrecognized values are rejected (None) so the API layer can surface an error.
        assert_eq!(
            super::normalize_optional_thinking_level(Some("ultra")),
            None
        );
        assert!(super::is_valid_thinking_level("Max"));
        assert!(!super::is_valid_thinking_level("turbo"));
    }

    #[test]
    fn codex_acp_default_mode_ignores_blank_override() {
        let config = AppConfig {
            codex_acp: Some(CodexAcpConfig {
                binary: None,
                default_mode: Some("   ".to_string()),
                multi_agent_enabled: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            config.codex_acp_default_mode().as_deref(),
            Some("full-access")
        );
    }

    #[test]
    fn codex_acp_multi_agent_enabled_defaults_to_true() {
        let config = AppConfig::default();
        assert!(config.codex_acp_multi_agent_enabled());
    }

    #[test]
    fn codex_acp_multi_agent_enabled_preserves_false_override() {
        let config = AppConfig {
            codex_acp: Some(CodexAcpConfig {
                binary: None,
                default_mode: None,
                multi_agent_enabled: Some(false),
            }),
            ..Default::default()
        };
        assert!(!config.codex_acp_multi_agent_enabled());
    }

    #[test]
    fn passkey_enabled_defaults_to_false() {
        let config = AppConfig::default();
        assert!(!config.passkey_enabled());
    }

    #[test]
    fn message_archive_defaults_point_to_lancedb() {
        let config = AppConfig::default();
        assert_eq!(config.message_archive_backend(), "lancedb");
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        assert_eq!(
            config.message_archive_uri(),
            std::path::Path::new(&home)
                .join(".agenthub/message-archive")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(config.message_archive_table(), "messages");
    }

    #[test]
    fn message_archive_config_preserves_trimmed_values() {
        let config = AppConfig {
            message_archive: Some(MessageArchiveConfig {
                backend: Some(" lanceDb ".to_string()),
                uri: Some(" /tmp/archive ".to_string()),
                message_table: Some(" archive_messages ".to_string()),
            }),
            ..Default::default()
        };
        assert_eq!(config.message_archive_backend(), "lancedb");
        assert_eq!(config.message_archive_uri(), "/tmp/archive");
        assert_eq!(config.message_archive_table(), "archive_messages");
    }

    #[test]
    fn message_archive_uri_expands_tilde_for_configured_values() {
        let config = AppConfig {
            message_archive: Some(MessageArchiveConfig {
                backend: None,
                uri: Some("~/custom-archive".to_string()),
                message_table: None,
            }),
            ..Default::default()
        };
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        assert_eq!(
            config.message_archive_uri(),
            std::path::Path::new(&home)
                .join("custom-archive")
                .to_string_lossy()
                .to_string()
        );
    }
}
