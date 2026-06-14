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

    /// Whether this provider applies the agent runtime profile (model + thinking level) at runtime via
    /// ACP session config options (`set_model` / `set_config("reasoning_effort", ...)`). Only Codex
    /// exposes these runtime config handlers today; Claude takes the profile as spawn env instead, and
    /// Gemini/Kimi do not support per-agent runtime profiles.
    pub(super) fn applies_runtime_profile_via_session_config(self) -> bool {
        self.id == ACP_PROVIDER_CODEX
    }
}

/// Map the provider-neutral thinking level to the Codex `reasoning_effort` config value. Codex tops out
/// at `high`, so `max` maps to `high`; an unrecognized level yields `None` (caller skips the option and
/// the provider default applies).
pub(super) fn codex_reasoning_effort_for_thinking_level(level: &str) -> Option<&'static str> {
    match level.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "max" => Some("high"),
        _ => None,
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
    let command_name = command_executable_name(command);
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
    let command_name = command_executable_name(command);
    command_name != Some("agenthub-acp")
}

pub(super) fn default_env_for_acp_provider(
    provider: Option<AcpProviderSpec>,
    codex_acp_multi_agent_enabled: bool,
    runtime_model: Option<&str>,
    thinking_level: Option<&str>,
) -> Vec<(String, String)> {
    let Some(spec) = provider else {
        return Vec::new();
    };
    match spec.id {
        ACP_PROVIDER_CODEX => {
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
        // Claude has no runtime session-config for model/thinking, so the runtime profile is applied as
        // startup env. Changing it later requires a new session/restart to take effect.
        ACP_PROVIDER_CLAUDE => claude_runtime_profile_env(runtime_model, thinking_level),
        _ => Vec::new(),
    }
}

/// Build the Claude spawn env for the agent runtime profile: `ANTHROPIC_MODEL` for the model override
/// and `MAX_THINKING_TOKENS` for the (mapped) thinking level. Unset fields are omitted so the Claude
/// default stays authoritative.
fn claude_runtime_profile_env(
    runtime_model: Option<&str>,
    thinking_level: Option<&str>,
) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(model) = runtime_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        env.push(("ANTHROPIC_MODEL".to_string(), model.to_string()));
    }
    if let Some(level) = thinking_level
        && let Some(tokens) = claude_max_thinking_tokens_for_thinking_level(level)
    {
        env.push(("MAX_THINKING_TOKENS".to_string(), tokens.to_string()));
    }
    env
}

/// Map the provider-neutral thinking level to Claude's `MAX_THINKING_TOKENS` budget (typical upstream
/// values). An unrecognized level yields `None` (the env var is omitted and the Claude default applies).
pub(super) fn claude_max_thinking_tokens_for_thinking_level(level: &str) -> Option<&'static str> {
    match level.trim().to_ascii_lowercase().as_str() {
        "low" => Some("4096"),
        "medium" => Some("8000"),
        "high" => Some("16000"),
        "max" => Some("20000"),
        _ => None,
    }
}

fn acp_provider_for_command_with_binary(
    codex_acp_binary: &str,
    command: &str,
) -> Option<AcpProviderSpec> {
    if command == codex_acp_binary {
        return Some(AcpProviderSpec::CODEX);
    }
    let command_name = command_executable_name(command)?;
    match command_name {
        "gemini" => Some(AcpProviderSpec::GEMINI),
        "kimi" => Some(AcpProviderSpec::KIMI),
        "claude-agent-acp" | "claude-code-acp-rs" => Some(AcpProviderSpec::CLAUDE),
        "agenthub-codex-acp" | "codex-acp" => Some(AcpProviderSpec::CODEX),
        name => {
            let target_name = command_executable_name(codex_acp_binary);
            if target_name == Some(name) {
                Some(AcpProviderSpec::CODEX)
            } else {
                None
            }
        }
    }
}

fn command_executable_name(command: &str) -> Option<&str> {
    let name = command.rsplit(['/', '\\']).next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.strip_suffix(".exe").unwrap_or(name))
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::{
        ACP_PROVIDER_CODEX, AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV, AcpProviderSpec,
        acp_provider_spec_for_agent_with_binary, claude_max_thinking_tokens_for_thinking_level,
        codex_reasoning_effort_for_thinking_level, default_env_for_acp_provider,
        should_use_configured_codex_binary,
    };

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().expect("lock env")
    }

    #[test]
    fn only_codex_applies_runtime_profile_via_session_config() {
        assert!(AcpProviderSpec::CODEX.applies_runtime_profile_via_session_config());
        for spec in [
            AcpProviderSpec::CLAUDE,
            AcpProviderSpec::GEMINI,
            AcpProviderSpec::KIMI,
        ] {
            assert!(
                !spec.applies_runtime_profile_via_session_config(),
                "provider {} must not use session-config runtime profile",
                spec.id
            );
        }
    }

    #[test]
    fn codex_reasoning_effort_maps_thinking_levels() {
        assert_eq!(
            codex_reasoning_effort_for_thinking_level("low"),
            Some("low")
        );
        assert_eq!(
            codex_reasoning_effort_for_thinking_level("medium"),
            Some("medium")
        );
        assert_eq!(
            codex_reasoning_effort_for_thinking_level("high"),
            Some("high")
        );
        // Codex tops out at high, so max maps to high.
        assert_eq!(
            codex_reasoning_effort_for_thinking_level("max"),
            Some("high")
        );
        // Case-insensitive + trimmed.
        assert_eq!(
            codex_reasoning_effort_for_thinking_level(" MAX "),
            Some("high")
        );
        // Unknown levels are skipped (provider default applies).
        assert_eq!(codex_reasoning_effort_for_thinking_level("turbo"), None);
    }

    #[test]
    fn default_env_for_codex_provider_enables_rust_backtrace() {
        let _guard = lock_env();
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::remove_var("RUST_BACKTRACE");
        }
        let env = default_env_for_acp_provider(Some(AcpProviderSpec::CODEX), true, None, None);
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
        assert!(default_env_for_acp_provider(provider, true, None, None).is_empty());
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
        assert!(!should_use_configured_codex_binary(
            r"C:\agenthub\agenthub-codex-acp.exe",
            r"C:\agenthub\agenthub-acp.exe"
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
        let env = default_env_for_acp_provider(Some(AcpProviderSpec::CODEX), false, None, None);
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

    #[test]
    fn claude_max_thinking_tokens_maps_thinking_levels() {
        assert_eq!(
            claude_max_thinking_tokens_for_thinking_level("low"),
            Some("4096")
        );
        assert_eq!(
            claude_max_thinking_tokens_for_thinking_level("medium"),
            Some("8000")
        );
        assert_eq!(
            claude_max_thinking_tokens_for_thinking_level("high"),
            Some("16000")
        );
        assert_eq!(
            claude_max_thinking_tokens_for_thinking_level(" MAX "),
            Some("20000")
        );
        assert_eq!(claude_max_thinking_tokens_for_thinking_level("turbo"), None);
    }

    #[test]
    fn claude_runtime_profile_injects_model_and_thinking_env() {
        let env = default_env_for_acp_provider(
            Some(AcpProviderSpec::CLAUDE),
            true,
            Some("claude-opus-4-8"),
            Some("high"),
        );
        assert_eq!(
            env,
            vec![
                ("ANTHROPIC_MODEL".to_string(), "claude-opus-4-8".to_string()),
                ("MAX_THINKING_TOKENS".to_string(), "16000".to_string()),
            ]
        );
    }

    #[test]
    fn claude_runtime_profile_omits_unset_fields() {
        // No profile -> no env (Claude defaults stay authoritative).
        assert!(
            default_env_for_acp_provider(Some(AcpProviderSpec::CLAUDE), true, None, None)
                .is_empty()
        );
        // Blank model is treated as unset; an unknown level is dropped.
        assert!(
            default_env_for_acp_provider(
                Some(AcpProviderSpec::CLAUDE),
                true,
                Some("   "),
                Some("turbo"),
            )
            .is_empty()
        );
        // Only the model set.
        assert_eq!(
            default_env_for_acp_provider(
                Some(AcpProviderSpec::CLAUDE),
                true,
                Some("claude-opus-4-8"),
                None,
            ),
            vec![("ANTHROPIC_MODEL".to_string(), "claude-opus-4-8".to_string())]
        );
    }

    #[test]
    fn codex_env_ignores_runtime_profile_fields() {
        let _guard = lock_env();
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::set_var("RUST_BACKTRACE", "full");
        }
        // Codex carries its profile via session config, not env, so the env builder ignores the
        // profile args and just emits the Codex multi-agent flag.
        let env = default_env_for_acp_provider(
            Some(AcpProviderSpec::CODEX),
            true,
            Some("gpt-5.4-codex"),
            Some("high"),
        );
        assert_eq!(
            env,
            vec![(
                AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV.to_string(),
                "1".to_string()
            )]
        );
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::remove_var("RUST_BACKTRACE");
        }
    }
}
