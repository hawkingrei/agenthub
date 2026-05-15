pub mod path_utils;

use serde::Deserialize;
use std::collections::HashSet;

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
    pub history: Option<HistoryConfig>,
    pub message_archive: Option<MessageArchiveConfig>,
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
            .unwrap_or_else(|| "agenthub-codex-acp".to_string())
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

pub fn normalize_codex_acp_mode_id(mode_id: &str) -> String {
    match mode_id.trim() {
        "yolo" | "yalo" | "danger_full_access" | "danger-full-access" => "full-access".to_string(),
        value => value.to_string(),
    }
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
        AppConfig, CodexAcpConfig, HistoryConfig, MessageArchiveConfig, ServerConfig, ServerRole,
        WorktreeConfig,
    };

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
