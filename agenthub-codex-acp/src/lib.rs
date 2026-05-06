//! Codex ACP - An Agent Client Protocol implementation for Codex.
#![deny(clippy::print_stdout, clippy::print_stderr)]

use agent_client_protocol::schema::{
    AuthenticateRequest, CancelNotification, CloseSessionRequest, InitializeRequest,
    ListSessionsRequest, LoadSessionRequest, NewSessionRequest, PromptRequest,
    SetSessionConfigOptionRequest, SetSessionModeRequest, SetSessionModelRequest,
};
use agent_client_protocol::{Agent, Client, ConnectionTo};
use codex_core::config::ManagedFeatures;
use codex_core::config::{Config, ConfigOverrides};
use codex_exec_server::{EnvironmentManager, EnvironmentManagerArgs, ExecServerRuntimePaths};
use codex_features::{Feature, Features};
use codex_utils_cli::CliConfigOverrides;
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
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

mod app_server_thread;
mod codex_agent;
#[cfg(target_os = "linux")]
mod linux_memfd_compat;
mod prompt_args;
mod thread;

pub static ACP_CLIENT: OnceLock<Arc<ConnectionTo<Client>>> = OnceLock::new();
const AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV: &str = "AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED";

#[derive(Clone)]
struct LocalCodexAgent(Rc<codex_agent::CodexAgent>);

// The ACP adapter runs inside a single-threaded Tokio runtime and LocalSet. The
// 0.11 ACP builder requires Send handlers even though this binary never moves
// CodexAgent across worker threads.
unsafe impl Send for LocalCodexAgent {}
unsafe impl Sync for LocalCodexAgent {}

impl std::ops::Deref for LocalCodexAgent {
    type Target = codex_agent::CodexAgent;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

struct LocalSendFuture<F>(F);

// These futures are only polled on the current-thread ACP runtime. This wrapper
// satisfies the 0.11 ACP handler bounds while preserving the existing local
// Codex state model.
unsafe impl<F> Send for LocalSendFuture<F> {}

impl<F: Future> Future for LocalSendFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: LocalSendFuture is a transparent wrapper and does not move the
        // inner future after it has been pinned by the executor.
        unsafe { self.map_unchecked_mut(|item| &mut item.0) }.poll(cx)
    }
}

fn local_send_future<F: Future>(future: F) -> LocalSendFuture<F> {
    LocalSendFuture(future)
}

pub(crate) async fn build_environment_manager(
    config: &Config,
) -> Result<Arc<EnvironmentManager>, agent_client_protocol::Error> {
    let current_exe = std::env::current_exe().map_err(|err| {
        agent_client_protocol::Error::internal_error().data(format!(
            "failed to determine current executable path: {err}"
        ))
    })?;
    let runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        Some(current_exe),
        config.codex_linux_sandbox_exe.clone(),
    )
    .map_err(|err| {
        agent_client_protocol::Error::internal_error().data(format!(
            "failed to resolve exec-server runtime paths: {err}"
        ))
    })?;
    Ok(Arc::new(
        EnvironmentManager::new(EnvironmentManagerArgs::new(runtime_paths)).await,
    ))
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn spawn_acp_io_task<F>(
    thread_name: &str,
    io_task: F,
) -> IoResult<tokio::sync::oneshot::Receiver<IoResult<()>>>
where
    F: Future<Output = agent_client_protocol::Result<()>> + Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    let thread_name = thread_name.to_string();

    std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(std::io::Error::other)
                .and_then(|runtime| {
                    let local_set = LocalSet::new();
                    runtime
                        .block_on(local_set.run_until(io_task))
                        .map_err(|err| std::io::Error::other(format!("ACP I/O error: {err}")))
                });
            drop(tx.send(result));
        })
        .map_err(std::io::Error::other)?;

    Ok(rx)
}

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

trait CollabFeatureState {
    fn collab_enabled(&self) -> bool;
    fn try_enable_collab(&mut self) -> Result<(), String>;
}

impl CollabFeatureState for Features {
    fn collab_enabled(&self) -> bool {
        self.enabled(Feature::Collab)
    }

    fn try_enable_collab(&mut self) -> Result<(), String> {
        self.enable(Feature::Collab);
        Ok(())
    }
}

impl CollabFeatureState for ManagedFeatures {
    fn collab_enabled(&self) -> bool {
        self.enabled(Feature::Collab)
    }

    fn try_enable_collab(&mut self) -> Result<(), String> {
        self.enable(Feature::Collab).map_err(|err| err.to_string())
    }
}

fn enable_default_multi_agent_collab<T: CollabFeatureState>(
    features: &mut T,
) -> Result<bool, String> {
    if features.collab_enabled() {
        return Ok(false);
    }
    features.try_enable_collab()?;
    Ok(true)
}

fn parse_agenthub_multi_agent_enabled_env(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        value => Err(format!(
            "invalid {} value '{}'; expected one of 1/0/true/false/on/off/yes/no",
            AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV, value
        )),
    }
}

fn resolve_agenthub_multi_agent_enabled_override() -> Result<Option<bool>, String> {
    let Some(raw) = std::env::var_os(AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV) else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    if raw.trim().is_empty() {
        return Err(format!(
            "{} must not be empty",
            AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV
        ));
    }
    parse_agenthub_multi_agent_enabled_env(&raw).map(Some)
}

fn apply_agenthub_multi_agent_override<T: CollabFeatureState>(
    features: &mut T,
    enabled: Option<bool>,
) -> Result<bool, String> {
    match enabled {
        Some(true) => enable_default_multi_agent_collab(features),
        Some(false) | None => Ok(false),
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
    match resolve_agenthub_multi_agent_enabled_override() {
        Ok(enabled) => {
            if let Err(err) = apply_agenthub_multi_agent_override(&mut config.features, enabled) {
                tracing::warn!(
                    error = %err,
                    "failed to apply AgentHub multi_agent feature override for agenthub-codex-acp",
                );
            }
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "ignoring invalid AgentHub multi_agent feature override for agenthub-codex-acp",
            );
        }
    }
    normalize_responses_websocket_support(&mut config);

    // Run the I/O task to handle the actual communication
    LocalSet::new()
        .run_until(async move {
            let agent = LocalCodexAgent(Rc::new(
                codex_agent::CodexAgent::new(config).await.map_err(|err| {
                    std::io::Error::other(format!("failed to initialize Codex ACP agent: {err}"))
                })?,
            ));
            let transport = agent_client_protocol::ByteStreams::new(
                tokio::io::stdout().compat_write(),
                tokio::io::stdin().compat(),
            );
            let agent_for_initialize = agent.clone();
            let agent_for_authenticate = agent.clone();
            let agent_for_new_session = agent.clone();
            let agent_for_load_session = agent.clone();
            let agent_for_list_sessions = agent.clone();
            let agent_for_close_session = agent.clone();
            let agent_for_prompt = agent.clone();
            let agent_for_cancel = agent.clone();
            let agent_for_mode = agent.clone();
            let agent_for_model = agent.clone();
            let agent_for_config = agent.clone();

            Agent
                .builder()
                .name("agenthub-codex-acp")
                .on_receive_request(
                    async move |request: InitializeRequest, responder, connection| {
                        drop(ACP_CLIENT.set(Arc::new(connection.clone())));
                        responder.respond_with_result(
                            local_send_future(agent_for_initialize.initialize(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: AuthenticateRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_authenticate.authenticate(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: NewSessionRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_new_session.new_session(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: LoadSessionRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_load_session.load_session(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: ListSessionsRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_list_sessions.list_sessions(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: CloseSessionRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_close_session.close_session(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: PromptRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_prompt.prompt(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |notification: CancelNotification, _connection| {
                        local_send_future(agent_for_cancel.cancel(notification)).await
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .on_receive_request(
                    async move |request: SetSessionModeRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_mode.set_session_mode(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: SetSessionModelRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_model.set_session_model(request)).await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_request(
                    async move |request: SetSessionConfigOptionRequest, responder, _connection| {
                        responder.respond_with_result(
                            local_send_future(agent_for_config.set_session_config_option(request))
                                .await,
                        )
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .connect_to(transport)
                .await
                .map_err(|err| std::io::Error::other(format!("ACP I/O error: {err}")))
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
        AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV, apply_agenthub_multi_agent_override,
        parse_agenthub_multi_agent_enabled_env, resolve_agenthub_multi_agent_enabled_override,
        responses_websocket_feature_opt_in_enabled, rewrite_misleading_timeout_message,
        should_disable_implicit_responses_websockets,
    };
    use codex_features::{Feature, Features};
    use std::sync::{Mutex, MutexGuard};
    use tracing::Level;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct FailingCollabFeatureState {
        enabled: bool,
    }

    impl super::CollabFeatureState for FailingCollabFeatureState {
        fn collab_enabled(&self) -> bool {
            self.enabled
        }

        fn try_enable_collab(&mut self) -> Result<(), String> {
            Err("pinned by test".to_string())
        }
    }

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().expect("lock env")
    }

    #[test]
    fn agenthub_codex_acp_enables_multi_agent_when_override_is_true() {
        let mut features = Features::with_defaults();
        features.disable(Feature::Collab);

        let changed = apply_agenthub_multi_agent_override(&mut features, Some(true))
            .expect("enable collab succeeds");

        assert!(changed);
        assert!(features.enabled(Feature::Collab));
    }

    #[test]
    fn agenthub_codex_acp_leaves_existing_multi_agent_enabled() {
        let mut features = Features::with_defaults();
        features.enable(Feature::Collab);

        let changed = apply_agenthub_multi_agent_override(&mut features, Some(true))
            .expect("no-op enable succeeds");

        assert!(!changed);
        assert!(features.enabled(Feature::Collab));
    }

    #[test]
    fn agenthub_codex_acp_skips_multi_agent_enable_when_override_is_false() {
        let mut features = Features::with_defaults();
        let initial = features.enabled(Feature::Collab);

        let changed = apply_agenthub_multi_agent_override(&mut features, Some(false))
            .expect("disable override is a no-op");

        assert!(!changed);
        assert_eq!(features.enabled(Feature::Collab), initial);
    }

    #[test]
    fn agenthub_codex_acp_skips_multi_agent_enable_without_override() {
        let mut features = Features::with_defaults();
        let initial = features.enabled(Feature::Collab);

        let changed = apply_agenthub_multi_agent_override(&mut features, None)
            .expect("missing override is a no-op");

        assert!(!changed);
        assert_eq!(features.enabled(Feature::Collab), initial);
    }

    #[test]
    fn agenthub_codex_acp_surfaces_collab_enable_failures() {
        let mut features = FailingCollabFeatureState { enabled: false };

        let err = apply_agenthub_multi_agent_override(&mut features, Some(true))
            .expect_err("failing feature gate should be surfaced");

        assert!(err.contains("pinned by test"));
    }

    #[test]
    fn parse_agenthub_multi_agent_enabled_env_accepts_common_true_values() {
        for raw in ["1", "true", "TRUE", " yes ", "on"] {
            assert!(parse_agenthub_multi_agent_enabled_env(raw).expect("parse true"));
        }
    }

    #[test]
    fn parse_agenthub_multi_agent_enabled_env_accepts_common_false_values() {
        for raw in ["0", "false", "FALSE", " no ", "off"] {
            assert!(!parse_agenthub_multi_agent_enabled_env(raw).expect("parse false"));
        }
    }

    #[test]
    fn parse_agenthub_multi_agent_enabled_env_rejects_invalid_values() {
        let err = parse_agenthub_multi_agent_enabled_env("maybe")
            .expect_err("invalid values should be rejected");
        assert!(err.contains(AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV));
    }

    #[test]
    fn resolve_agenthub_multi_agent_enabled_override_reads_env() {
        let _guard = lock_env();
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::set_var(AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV, "true");
        }
        assert_eq!(
            resolve_agenthub_multi_agent_enabled_override().expect("resolve env"),
            Some(true)
        );
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::remove_var(AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV);
        }
    }

    #[test]
    fn resolve_agenthub_multi_agent_enabled_override_rejects_blank_env() {
        let _guard = lock_env();
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::set_var(AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV, "   ");
        }
        let err = resolve_agenthub_multi_agent_enabled_override()
            .expect_err("blank env should be rejected");
        assert!(err.contains("must not be empty"));
        // SAFETY: tests serialize environment mutation and restore state before exit.
        unsafe {
            std::env::remove_var(AGENTHUB_CODEX_ACP_MULTI_AGENT_ENABLED_ENV);
        }
    }

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
        features.enable(codex_features::Feature::ResponsesWebsockets);

        assert!(responses_websocket_feature_opt_in_enabled(&features));
        assert!(!should_disable_implicit_responses_websockets(
            true, &features
        ));
    }

    #[test]
    fn explicit_responses_websocket_v2_feature_keeps_provider_websocket_support() {
        let mut features = Features::with_defaults();
        features.enable(codex_features::Feature::ResponsesWebsocketsV2);

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
