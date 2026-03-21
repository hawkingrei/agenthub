use std::path::Path;

use crate::acp::AcpPromptDeliveryPolicy;

use super::AgentManager;

pub(super) const ACP_PROVIDER_CODEX: &str = "codex";
pub(super) const ACP_PROVIDER_GEMINI: &str = "gemini";
pub(super) const ACP_PROVIDER_KIMI: &str = "kimi";

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
        if configured == command {
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
    let provider = acp_provider_for_command_with_binary(codex_acp_binary, command)?;
    match provider.id {
        ACP_PROVIDER_GEMINI => {
            let has_flag = args
                .iter()
                .any(|arg| arg == "--experimental-acp" || arg.starts_with("--experimental-acp="));
            if has_flag { Some(provider) } else { None }
        }
        ACP_PROVIDER_KIMI => {
            if args.iter().any(|arg| arg == "acp") {
                Some(provider)
            } else {
                None
            }
        }
        _ => Some(provider),
    }
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
