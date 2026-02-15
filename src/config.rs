use serde::Deserialize;

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
    pub push: Option<PushConfig>,
    pub safe_paths: Option<Vec<String>>,
    pub web_dir: Option<String>,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    pub rp_id: Option<String>,
    pub rp_origin: Option<String>,
    pub rp_name: Option<String>,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct PushConfig {
    pub subject: Option<String>,
    pub keys_path: Option<String>,
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

    pub fn safe_paths(&self) -> Vec<String> {
        self.safe_paths.clone().unwrap_or_default()
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
        self.codex_acp
            .as_ref()
            .and_then(|c| c.default_mode.as_deref())
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

fn detect_env_overrides() -> Vec<String> {
    let keys = [
        "AGENTHUB_LISTEN",
        "AGENTHUB_RP_ID",
        "AGENTHUB_RP_ORIGIN",
        "AGENTHUB_RP_NAME",
        "AGENTHUB_SAFE_PATHS",
        "AGENTHUB_WEB_DIR",
        "AGENTHUB_LOG_PATH",
        "AGENTHUB_CODEX_ACP_BINARY",
        "AGENTHUB_CODEX_ACP_DEFAULT_MODE",
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

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        std::env::var("HOME").unwrap_or_else(|_| path.to_string())
    } else if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            std::path::Path::new(&home)
                .join(stripped)
                .to_string_lossy()
                .to_string()
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, WorktreeConfig};

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
}
