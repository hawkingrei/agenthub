use std::time::Duration;

use serde::Deserialize;

use crate::path_utils::expand_tilde;

/// Runtime-owned credentials are deliberately absent from this configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaraConfig {
    pub binary: Option<String>,
    pub transport: Option<String>,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub startup_timeout_seconds: Option<u64>,
    pub shutdown_timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaraLaunchConfig {
    pub binary: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub startup_timeout: Duration,
    pub shutdown_timeout: Duration,
}

impl RaraConfig {
    /// Resolve explicit overrides without changing process-global environment.
    pub fn resolve_with(
        self,
        mut environment: impl FnMut(&str) -> Option<String>,
    ) -> anyhow::Result<RaraLaunchConfig> {
        anyhow::ensure!(
            self.transport.as_deref().unwrap_or("stdio-jsonl") == "stdio-jsonl",
            "rara_app_server_unsupported: transport must be stdio-jsonl"
        );
        let binary = environment("AGENTHUB_RARA_BINARY")
            .or(self.binary)
            .unwrap_or_else(|| "rara".into());
        let binary = binary.trim();
        anyhow::ensure!(
            !binary.is_empty() && !binary.chars().any(char::is_control),
            "rara binary must be a nonempty executable path"
        );
        let provider = environment("AGENTHUB_RARA_PROVIDER").or(self.default_provider);
        let model = environment("AGENTHUB_RARA_MODEL").or(self.default_model);
        for label in [provider.as_deref(), model.as_deref()]
            .into_iter()
            .flatten()
        {
            anyhow::ensure!(
                !label.trim().is_empty()
                    && label.len() <= 256
                    && !label.chars().any(char::is_control),
                "rara provider/model labels must contain 1-256 bytes without control characters"
            );
        }
        let startup_timeout = timeout(
            environment("AGENTHUB_RARA_STARTUP_TIMEOUT_SECONDS"),
            self.startup_timeout_seconds.unwrap_or(120),
        )?;
        let shutdown_timeout = timeout(
            environment("AGENTHUB_RARA_SHUTDOWN_TIMEOUT_SECONDS"),
            self.shutdown_timeout_seconds.unwrap_or(30),
        )?;
        Ok(RaraLaunchConfig {
            binary: expand_tilde(binary),
            default_provider: provider,
            default_model: model,
            startup_timeout,
            shutdown_timeout,
        })
    }
}

fn timeout(override_value: Option<String>, configured: u64) -> anyhow::Result<Duration> {
    let seconds = match override_value {
        Some(value) => value
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid rara timeout"))?,
        None => configured,
    };
    anyhow::ensure!(
        (1..=600).contains(&seconds),
        "rara timeout must be 1-600 seconds"
    );
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_is_separate_and_defaults_do_not_override_provider_credentials() {
        let app: crate::AppConfig = toml::from_str("").unwrap();
        assert!(app.rara.is_none());
        let config = RaraConfig::default().resolve_with(|_| None).unwrap();
        assert_eq!(config.binary, "rara");
        assert_eq!(config.default_provider, None);
        assert_eq!(config.default_model, None);
        assert_eq!(config.startup_timeout, Duration::from_secs(120));
        assert_eq!(config.shutdown_timeout, Duration::from_secs(30));
        assert_eq!(app.codex_acp_binary(), "agenthubd");
    }

    #[test]
    fn explicit_config_and_environment_resolve_without_ambient_mutation() {
        let app: crate::AppConfig = toml::from_str(
            r#"
            [rara]
            binary = "/opt/runtime"
            transport = "stdio-jsonl"
            default_provider = "deepseek"
            default_model = "fixture"
            startup_timeout_seconds = 45
            shutdown_timeout_seconds = 10
        "#,
        )
        .unwrap();
        let resolved = app
            .rara
            .unwrap()
            .resolve_with(|key| match key {
                "AGENTHUB_RARA_BINARY" => Some("/opt/new-runtime".into()),
                "AGENTHUB_RARA_MODEL" => Some("override".into()),
                "AGENTHUB_RARA_SHUTDOWN_TIMEOUT_SECONDS" => Some("20".into()),
                _ => None,
            })
            .unwrap();
        assert_eq!(resolved.binary, "/opt/new-runtime");
        assert_eq!(resolved.default_provider.as_deref(), Some("deepseek"));
        assert_eq!(resolved.default_model.as_deref(), Some("override"));
        assert_eq!(resolved.startup_timeout, Duration::from_secs(45));
        assert_eq!(resolved.shutdown_timeout, Duration::from_secs(20));
    }

    #[test]
    fn invalid_policy_and_secret_fields_fail_explicitly() {
        for body in [
            "transport = 'acp'",
            "binary = ''",
            "default_model = ''",
            "startup_timeout_seconds = 0",
            "shutdown_timeout_seconds = 601",
        ] {
            let config: RaraConfig = toml::from_str(body).unwrap();
            assert!(config.resolve_with(|_| None).is_err(), "{body}");
        }
        assert!(toml::from_str::<RaraConfig>("api_key = 'private'").is_err());
        let error = RaraConfig::default()
            .resolve_with(|key| (key == "AGENTHUB_RARA_MODEL").then(|| "private\nvalue".into()))
            .unwrap_err();
        assert!(!error.to_string().contains("private"));
    }
}
