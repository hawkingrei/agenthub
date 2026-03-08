//! Codex ACP - An Agent Client Protocol implementation for Codex.
#![deny(clippy::print_stdout, clippy::print_stderr)]

use agent_client_protocol::AgentSideConnection;
use codex_core::config::{Config, ConfigOverrides};
use codex_core::features::{Feature, Features};
use codex_utils_cli::CliConfigOverrides;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::{io::Result as IoResult, rc::Rc};
use tokio::task::LocalSet;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{Format, Full, Writer};
use tracing_subscriber::fmt::time::{FormatTime, SystemTime};
use tracing_subscriber::fmt::{FormatEvent, FormatFields};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

mod codex_agent;
mod local_spawner;
mod prompt_args;
mod thread;

pub static ACP_CLIENT: OnceLock<Arc<AgentSideConnection>> = OnceLock::new();

const MISLEADING_MODELS_REFRESH_TIMEOUT_MESSAGE: &str =
    "failed to refresh available models: timeout waiting for child process to exit";
const REWRITTEN_MODELS_REFRESH_TIMEOUT_MESSAGE: &str = "failed to refresh available models: timed out fetching remote model list (/models, 5s timeout)";

#[derive(Default)]
struct EventMessageVisitor {
    message: Option<String>,
}

impl Visit for EventMessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(trim_debug_string(format!("{value:?}")));
        }
    }
}

fn trim_debug_string(value: String) -> String {
    value
        .strip_prefix('"')
        .and_then(|trimmed| trimmed.strip_suffix('"'))
        .unwrap_or(&value)
        .to_owned()
}

fn rewrite_misleading_timeout_message(
    target: &str,
    level: &Level,
    message: &str,
) -> Option<&'static str> {
    if *level == Level::ERROR
        && target == "codex_core::models_manager::manager"
        && message == MISLEADING_MODELS_REFRESH_TIMEOUT_MESSAGE
    {
        Some(REWRITTEN_MODELS_REFRESH_TIMEOUT_MESSAGE)
    } else {
        None
    }
}

#[derive(Default)]
struct AgenthubEventFormat {
    timer: SystemTime,
    inner: Format<Full, SystemTime>,
}

impl<S, N> FormatEvent<S, N> for AgenthubEventFormat
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visitor = EventMessageVisitor::default();
        event.record(&mut visitor);

        if let Some(rewritten_message) = visitor.message.as_deref().and_then(|message| {
            rewrite_misleading_timeout_message(
                event.metadata().target(),
                event.metadata().level(),
                message,
            )
        }) {
            self.timer.format_time(&mut writer)?;
            write!(
                &mut writer,
                " {} {}: {}",
                event.metadata().level(),
                event.metadata().target(),
                rewritten_message
            )?;
            writeln!(&mut writer)?;
            return Ok(());
        }

        self.inner.format_event(ctx, writer, event)
    }
}

fn responses_websocket_feature_opt_in_enabled(features: &Features) -> bool {
    features.enabled(Feature::ResponsesWebsockets)
        || features.enabled(Feature::ResponsesWebsocketsV2)
}

fn should_disable_implicit_responses_websockets(
    supports_websockets: bool,
    features: &Features,
) -> bool {
    supports_websockets && !responses_websocket_feature_opt_in_enabled(features)
}

fn normalize_responses_websocket_support(config: &mut Config) {
    if should_disable_implicit_responses_websockets(
        config.model_provider.supports_websockets,
        &config.features,
    ) {
        tracing::info!(
            model_provider_id = %config.model_provider_id,
            "disabling implicit responses websocket support because websocket features are not enabled"
        );
        config.model_provider.supports_websockets = false;
    }
}

/// Run the Codex ACP agent.
///
/// This sets up an ACP agent that communicates over stdio, bridging
/// the ACP protocol with the existing codex-rs infrastructure.
///
/// # Errors
///
/// If unable to parse the config or start the program.
pub async fn run_main(
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
) -> IoResult<()> {
    // Install a simple subscriber so `tracing` output is visible.
    // Users can control the log level with `RUST_LOG`.
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .event_format(AgenthubEventFormat::default()),
        )
        .init();

    // Parse CLI overrides and load configuration
    let cli_kv_overrides = cli_config_overrides.parse_overrides().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("error parsing -c overrides: {e}"),
        )
    })?;

    let config_overrides = ConfigOverrides {
        codex_linux_sandbox_exe,
        ..ConfigOverrides::default()
    };

    let mut config =
        Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, config_overrides)
            .await
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("error loading config: {e}"),
                )
            })?;
    normalize_responses_websocket_support(&mut config);

    // Create our Agent implementation with notification channel
    let agent = Rc::new(codex_agent::CodexAgent::new(config));

    let stdin = tokio::io::stdin().compat();
    let stdout = tokio::io::stdout().compat_write();

    // Run the I/O task to handle the actual communication
    LocalSet::new()
        .run_until(async move {
            // Create the ACP connection
            let (client, io_task) = AgentSideConnection::new(agent.clone(), stdout, stdin, |fut| {
                tokio::task::spawn_local(fut);
            });

            if ACP_CLIENT.set(Arc::new(client)).is_err() {
                return Err(std::io::Error::other("ACP client already set"));
            }

            io_task
                .await
                .map_err(|e| std::io::Error::other(format!("ACP I/O error: {e}")))
        })
        .await?;

    Ok(())
}

// Re-export the MCP server types for compatibility
pub use codex_mcp_server::{
    CodexToolCallParam, CodexToolCallReplyParam, ExecApprovalElicitRequestParams,
    ExecApprovalResponse, PatchApprovalElicitRequestParams, PatchApprovalResponse,
};

#[cfg(test)]
mod tests {
    use super::{
        responses_websocket_feature_opt_in_enabled, rewrite_misleading_timeout_message,
        should_disable_implicit_responses_websockets,
    };
    use codex_core::features::Features;
    use tracing::Level;

    #[test]
    fn implicit_responses_websockets_are_disabled_without_feature_opt_in() {
        let features = Features::with_defaults();

        assert!(!responses_websocket_feature_opt_in_enabled(&features));
        assert!(should_disable_implicit_responses_websockets(
            true, &features
        ));
        assert!(!should_disable_implicit_responses_websockets(
            false, &features
        ));
    }

    #[test]
    fn explicit_responses_websocket_feature_keeps_provider_websocket_support() {
        let mut features = Features::with_defaults();
        features.enable(codex_core::features::Feature::ResponsesWebsockets);

        assert!(responses_websocket_feature_opt_in_enabled(&features));
        assert!(!should_disable_implicit_responses_websockets(
            true, &features
        ));
    }

    #[test]
    fn explicit_responses_websocket_v2_feature_keeps_provider_websocket_support() {
        let mut features = Features::with_defaults();
        features.enable(codex_core::features::Feature::ResponsesWebsocketsV2);

        assert!(responses_websocket_feature_opt_in_enabled(&features));
        assert!(!should_disable_implicit_responses_websockets(
            true, &features
        ));
    }

    #[test]
    fn rewrites_models_refresh_timeout_log_message() {
        assert_eq!(
            rewrite_misleading_timeout_message(
                "codex_core::models_manager::manager",
                &Level::ERROR,
                "failed to refresh available models: timeout waiting for child process to exit",
            ),
            Some(
                "failed to refresh available models: timed out fetching remote model list (/models, 5s timeout)"
            )
        );
    }

    #[test]
    fn does_not_rewrite_non_matching_log_messages() {
        assert_eq!(
            rewrite_misleading_timeout_message(
                "codex_core::models_manager::manager",
                &Level::ERROR,
                "failed to refresh available models: quota exceeded",
            ),
            None
        );
        assert_eq!(
            rewrite_misleading_timeout_message(
                "codex_core::other_target",
                &Level::ERROR,
                "failed to refresh available models: timeout waiting for child process to exit",
            ),
            None
        );
    }
}
