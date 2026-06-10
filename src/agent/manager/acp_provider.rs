use std::path::Path;

use crate::acp::AcpPromptDeliveryPolicy;

use super::AgentManager;

pub(super) const ACP_PROVIDER_CODEX: &str = "codex";
pub(super) const ACP_PROVIDER_GEMINI: &str = "gemini";
pub(super) const ACP_PROVIDER_KIMI: &str = "kimi";
pub(super) const ACP_PROVIDER_CLAUDE: &str = "claude";
pub(super) const AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV: &str =
    "AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AcpDefaultModeBehavior {
    ApplyWhenConfigured,
    IgnoreConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AcpProviderSpec {
    pub id: &'static str,
    pub prompt_delivery_policy: AcpPromptDeliveryPolicy,
    pub default_mode_behavior: AcpDefaultModeBehavior,
}

impl AcpProviderSpec {
    const CODEX: Self = Self {
        id: ACP_PROVIDER_CODEX,
        prompt_delivery_policy: AcpPromptDeliveryPolicy::AllowConcurrentPrompts,
        default_mode_behavior: AcpDefaultModeBehavior::ApplyWhenConfigured,
    };

    const GEMINI: Self = Self {
        id: ACP_PROVIDER_GEMINI,
        prompt_delivery_policy: AcpPromptDeliveryPolicy::StrictFifo,
        default_mode_behavior: AcpDefaultModeBehavior::IgnoreConfigured,
    };

    const KIMI: Self = Self {
        id: ACP_PROVIDER_KIMI,
        prompt_delivery_policy: AcpPromptDeliveryPolicy::StrictFifo,
        default_mode_behavior: AcpDefaultModeBehavior::IgnoreConfigured,
    };

    const CLAUDE: Self = Self {
        id: ACP_PROVIDER_CLAUDE,
        prompt_delivery_policy: AcpPromptDeliveryPolicy::StrictFifo,
        default_mode_behavior: AcpDefaultModeBehavior::IgnoreConfigured,
    };

    pub(super) const fn uses_default_mode_config(self) -> bool {
        matches!(
            self.default_mode_behavior,
            AcpDefaultModeBehavior::ApplyWhenConfigured
        )
    }
}

impl AgentManager {
    pub(super) fn acp_provider_spec_for_agent(
        &self,
        command: &str,
        args: &[String],
    ) -> Option<AcpProviderSpec> {
        acp_provider_spec_for_agent_with_binary(&self.codex_acp_binary, command, args)
    }

    pub fn acp_provider_for_agent(&self, command: &str, args: &[String]) -> Option<&'static str> {
        self.acp_provider_spec_for_agent(command, args)
            .map(|spec| spec.id)
    }

    pub(super) fn resolve_command_path(
        &self,
        command: &str,
        provider: Option<AcpProviderSpec>,
    ) -> String {
        if provider.map(|spec| spec.id) != Some(ACP_PROVIDER_CODEX) {
            return command.to_string();
        }
        let configured = &self.codex_acp_binary;
        if !should_use_configured_codex_binary(configured, command) {
            return command.to_string();
        }
        let configured_path = Path::new(configured);
        if configured_path.is_absolute() || configured_path.exists() {
            return configured.to_string();
        }
        command.to_string()
    }
}

#[cfg(test)]
pub(super) fn acp_provider_for_agent_with_binary(
    codex_acp_binary: &str,
    command: &str,
    args: &[String],
) -> Option<&'static str> {
    acp_provider_spec_for_agent_with_binary(codex_acp_binary, command, args).map(|spec| spec.id)
}

pub(super) fn acp_provider_spec_for_agent_with_binary(
    codex_acp_binary: &str,
    command: &str,
    args: &[String],
) -> Option<AcpProviderSpec> {
    let command_name = Path::new(command).file_name().and_then(|n| n.to_str());
    if command_name == Some("agenthub-acp") {
        return agenthub_acp_provider_spec_for_args(args);
    }

    let provider = acp_provider_for_command_with_binary(codex_acp_binary, command)?;
    match provider.id {
        ACP_PROVIDER_GEMINI => {
            let has_flag = args.iter().any(|arg| {
                arg == "--acp"
                    || arg.starts_with("--acp=")
                    || arg == "--experimental-acp"
                    || arg.starts_with("--experimental-acp=")
            });
            if has_flag { Some(provider) } else { None }
        }
        ACP_PROVIDER_KIMI => {
            if args.iter().any(|arg| arg == "acp") {
                Some(provider)
            } else {
                None
            }
        }
        ACP_PROVIDER_CLAUDE => {
            if command_name == Some("claude-code-acp-rs") && !args.iter().any(|arg| arg == "--acp")
            {
                None
            } else {
                Some(provider)
            }
        }
        _ => Some(provider),
    }
}

fn agenthub_acp_provider_spec_for_args(args: &[String]) -> Option<AcpProviderSpec> {
    match args.first().map(|arg| arg.as_str()) {
        Some("codex") => Some(AcpProviderSpec::CODEX),
        Some("claude") => Some(AcpProviderSpec::CLAUDE),
        _ => None,
    }
}

fn should_use_configured_codex_binary(codex_acp_binary: &str, command: &str) -> bool {
    if command == codex_acp_binary {
        return false;
    }
    let command_name = Path::new(command).file_name().and_then(|n| n.to_str());
    command_name != Some("agenthub-acp")
}

pub(super) fn default_env_for_acp_provider(
    provider: Option<AcpProviderSpec>,
    codex_acp_multi_agent_enabled: bool,
) -> Vec<(String, String)> {
    if provider.map(|spec| spec.id) != Some(ACP_PROVIDER_CODEX) {
        return Vec::new();
    }

    let mut env = vec![(
        AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV.to_string(),
        if codex_acp_multi_agent_enabled {
            "1"
        } else {
            "0"
        }
        .to_string(),
    )];
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        env.push(("RUST_BACKTRACE".to_string(), "1".to_string()));
    }
    env
}

fn acp_provider_for_command_with_binary(
    codex_acp_binary: &str,
    command: &str,
) -> Option<AcpProviderSpec> {
    if command == codex_acp_binary {
        return Some(AcpProviderSpec::CODEX);
    }
    let command_name = Path::new(command).file_name().and_then(|n| n.to_str())?;
    match command_name {
        "gemini" => Some(AcpProviderSpec::GEMINI),
        "kimi" => Some(AcpProviderSpec::KIMI),
        "claude-agent-acp" | "claude-code-acp-rs" => Some(AcpProviderSpec::CLAUDE),
        "agenthub-codex-acp" | "codex-acp" => Some(AcpProviderSpec::CODEX),
        name => {
            let target_name = Path::new(codex_acp_binary)
                .file_name()
                .and_then(|n| n.to_str());
            if target_name == Some(name) {
                Some(AcpProviderSpec::CODEX)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::{
        ACP_PROVIDER_CODEX, AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV, AcpProviderSpec,
        acp_provider_spec_for_agent_with_binary, default_env_for_acp_provider,
        should_use_configured_codex_binary,
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().expect("lock env")
    }

    #[test]
    fn default_env_for_codex_provider_enables_rust_backtrace() {
        let _guard = lock_env();
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::remove_var("RUST_BACKTRACE");
        }
        let env = default_env_for_acp_provider(Some(AcpProviderSpec::CODEX), true);
        assert_eq!(
            env,
            vec![
                (
                    AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV.to_string(),
                    "1".to_string()
                ),
                ("RUST_BACKTRACE".to_string(), "1".to_string())
            ]
        );
    }

    #[test]
    fn default_env_for_non_codex_provider_is_empty() {
        let provider = acp_provider_spec_for_agent_with_binary(
            "agenthub-codex-acp",
            "gemini",
            &["--acp".to_string()],
        );
        assert_ne!(provider.map(|spec| spec.id), Some(ACP_PROVIDER_CODEX));
        assert!(default_env_for_acp_provider(provider, true).is_empty());
    }

    #[test]
    fn generic_codex_adapter_does_not_resolve_to_legacy_configured_binary() {
        assert!(!should_use_configured_codex_binary(
            "/opt/agenthub/bin/agenthub-codex-acp",
            "agenthub-acp"
        ));
        assert!(!should_use_configured_codex_binary(
            "/opt/agenthub/bin/agenthub-codex-acp",
            "/usr/local/bin/agenthub-acp"
        ));
        assert!(should_use_configured_codex_binary(
            "/opt/agenthub/bin/agenthub-codex-acp",
            "codex-acp"
        ));
    }

    #[test]
    fn default_env_for_codex_provider_preserves_existing_backtrace_override() {
        let _guard = lock_env();
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::set_var("RUST_BACKTRACE", "full");
        }
        let env = default_env_for_acp_provider(Some(AcpProviderSpec::CODEX), false);
        assert_eq!(
            env,
            vec![(
                AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV.to_string(),
                "0".to_string()
            )]
        );
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::remove_var("RUST_BACKTRACE");
        }
    }
}
