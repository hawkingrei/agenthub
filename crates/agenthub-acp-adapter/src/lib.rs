use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use claude_code_acp::Cli as UpstreamCli;
use codex_utils_cli::CliConfigOverrides;

const UPSTREAM_DEFAULT_OTEL_SERVICE_NAME: &str = "claude-code-acp-rs";
const AGENTHUB_OTEL_SERVICE_NAME: &str = "agenthub-acp";

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(name = "agenthub-acp")]
#[command(version, about = "AgentHub ACP provider adapter")]
pub struct Cli {
    #[command(subcommand)]
    pub provider: ProviderCommand,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ProviderCommand {
    /// Run Codex through AgentHub's ACP server mode.
    Codex(CodexCli),

    /// Run Claude Code through ACP server mode.
    Claude(ClaudeCli),
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct CodexCli {
    /// Override a Codex configuration value, using the same key=value format as codex -c.
    #[arg(short = 'c', long = "config", value_name = "key=value", action = clap::ArgAction::Append)]
    pub config_overrides: Vec<String>,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCli {
    /// Compatibility no-op. AgentHub's wrapper always runs ACP server mode.
    #[arg(long, hide = true)]
    pub acp: bool,

    /// Enable diagnostic mode (auto-log to temp file).
    #[arg(short, long)]
    pub diagnostic: bool,

    /// Log directory (implies diagnostic mode).
    #[arg(short = 'l', long, value_name = "DIR")]
    pub log_dir: Option<PathBuf>,

    /// Log file name (implies diagnostic mode).
    #[arg(short = 'f', long, value_name = "FILE")]
    pub log_file: Option<String>,

    /// Increase logging verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Quiet mode (only errors).
    #[arg(short, long)]
    pub quiet: bool,

    /// OpenTelemetry OTLP endpoint.
    #[arg(long, value_name = "URL", env = "OTEL_EXPORTER_OTLP_ENDPOINT")]
    pub otel_endpoint: Option<String>,

    /// OpenTelemetry service name.
    #[arg(long, value_name = "NAME", default_value = AGENTHUB_OTEL_SERVICE_NAME)]
    pub otel_service_name: String,
}

impl From<CodexCli> for CliConfigOverrides {
    fn from(cli: CodexCli) -> Self {
        Self {
            raw_overrides: cli.config_overrides,
        }
    }
}

impl From<ClaudeCli> for UpstreamCli {
    fn from(cli: ClaudeCli) -> Self {
        let diagnostic = cli.diagnostic || cli.log_dir.is_some() || cli.log_file.is_some();
        Self {
            acp: true,
            prompt: None,
            diagnostic,
            log_dir: cli.log_dir,
            log_file: cli.log_file,
            verbose: cli.verbose,
            quiet: cli.quiet,
            otel_endpoint: cli.otel_endpoint,
            otel_service_name: normalize_otel_service_name(cli.otel_service_name),
        }
    }
}

pub async fn run_with_cli(cli: Cli) -> anyhow::Result<()> {
    run_with_runtime_paths(cli, None, None).await
}

pub async fn run_with_runtime_paths(
    cli: Cli,
    codex_linux_sandbox_exe: Option<PathBuf>,
    actor_cli_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    match cli.provider {
        ProviderCommand::Codex(codex) => {
            run_codex_runtime(codex, codex_linux_sandbox_exe, actor_cli_path).await
        }
        ProviderCommand::Claude(claude) => {
            let upstream_cli = UpstreamCli::from(claude);
            claude_code_acp::run_acp_with_cli(&upstream_cli).await
        }
    }
}

pub async fn run_with_shutdown(
    cli: Cli,
    codex_linux_sandbox_exe: Option<PathBuf>,
    actor_cli_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let result = tokio::select! {
        result = run_with_runtime_paths(cli, codex_linux_sandbox_exe, actor_cli_path) => result,
        result = wait_for_shutdown_signal() => result,
    };
    shutdown();
    result
}

async fn wait_for_shutdown_signal() -> anyhow::Result<()> {
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = async {
            #[cfg(unix)]
            {
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate(),
                )?;
                sigterm.recv().await;
                Ok::<(), std::io::Error>(())
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<std::io::Result<()>>().await
            }
        } => {},
    }
    Ok(())
}

#[cfg(not(test))]
async fn run_codex_runtime(
    codex: CodexCli,
    codex_linux_sandbox_exe: Option<PathBuf>,
    actor_cli_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    agenthub_codex_acp_runtime::run_main(
        codex_linux_sandbox_exe,
        actor_cli_path,
        CliConfigOverrides::from(codex),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
async fn run_codex_runtime(
    codex: CodexCli,
    _codex_linux_sandbox_exe: Option<PathBuf>,
    _actor_cli_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let overrides = CliConfigOverrides::from(codex);
    if overrides
        .raw_overrides
        .iter()
        .any(|value| value == "__agenthub_test_error=true")
    {
        anyhow::bail!("test codex runtime failure");
    }
    Ok(())
}

pub fn shutdown() {
    claude_code_acp::shutdown_otel();
}

fn normalize_otel_service_name(service_name: String) -> String {
    if service_name == UPSTREAM_DEFAULT_OTEL_SERVICE_NAME {
        AGENTHUB_OTEL_SERVICE_NAME.to_string()
    } else {
        service_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_codex_config_overrides() {
        let Cli {
            provider: ProviderCommand::Codex(codex),
        } = Cli::parse_from([
            "agenthub-acp",
            "codex",
            "-c",
            "model=gpt-5",
            "--config",
            "sandbox_mode=workspace-write",
        ])
        else {
            panic!("expected codex provider");
        };
        let overrides = CliConfigOverrides::from(codex);
        assert_eq!(
            overrides.raw_overrides,
            vec![
                "model=gpt-5".to_string(),
                "sandbox_mode=workspace-write".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn run_with_cli_routes_codex_provider() {
        let cli = Cli::parse_from(["agenthub-acp", "codex", "-c", "model=gpt-5"]);
        run_with_cli(cli).await.expect("codex runtime dispatch");
    }

    #[tokio::test]
    async fn run_with_cli_propagates_codex_runtime_errors() {
        let cli = Cli::parse_from(["agenthub-acp", "codex", "-c", "__agenthub_test_error=true"]);
        let error = run_with_cli(cli)
            .await
            .expect_err("codex runtime error should propagate");
        assert_eq!(error.to_string(), "test codex runtime failure");
    }

    #[test]
    fn maps_to_upstream_acp_mode() {
        let Cli {
            provider: ProviderCommand::Claude(claude),
        } = Cli::parse_from(["agenthub-acp", "claude"])
        else {
            panic!("expected claude provider");
        };
        let upstream = UpstreamCli::from(claude);
        assert!(upstream.acp);
        assert!(upstream.prompt.is_none());
        assert_eq!(upstream.otel_service_name, "agenthub-acp");
    }

    #[test]
    fn keeps_upstream_acp_flag_compatible() {
        let Cli {
            provider: ProviderCommand::Claude(claude),
        } = Cli::parse_from(["agenthub-acp", "claude", "--acp"])
        else {
            panic!("expected claude provider");
        };
        let upstream = UpstreamCli::from(claude);
        assert!(upstream.acp);
        assert!(upstream.prompt.is_none());
    }

    #[test]
    fn maps_diagnostic_and_logging_flags() {
        let Cli {
            provider: ProviderCommand::Claude(claude),
        } = Cli::parse_from([
            "agenthub-acp",
            "claude",
            "--diagnostic",
            "--log-dir",
            "/tmp/agenthub-claude",
            "--log-file",
            "claude.log",
            "-vv",
            "--quiet",
            "--otel-endpoint",
            "http://localhost:4317",
            "--otel-service-name",
            "custom-claude",
        ])
        else {
            panic!("expected claude provider");
        };
        let upstream = UpstreamCli::from(claude);
        assert!(upstream.acp);
        assert!(upstream.diagnostic);
        assert_eq!(
            upstream.log_dir,
            Some(PathBuf::from("/tmp/agenthub-claude"))
        );
        assert_eq!(upstream.log_file, Some("claude.log".to_string()));
        assert_eq!(upstream.verbose, 2);
        assert!(upstream.quiet);
        assert_eq!(
            upstream.otel_endpoint,
            Some("http://localhost:4317".to_string())
        );
        assert_eq!(upstream.otel_service_name, "custom-claude");
    }

    #[test]
    fn log_output_flags_enable_diagnostic_mode() {
        let Cli {
            provider: ProviderCommand::Claude(claude),
        } = Cli::parse_from([
            "agenthub-acp",
            "claude",
            "--log-dir",
            "/tmp/agenthub-claude",
        ])
        else {
            panic!("expected claude provider");
        };
        let upstream = UpstreamCli::from(claude);
        assert!(upstream.diagnostic);
        assert_eq!(
            upstream.log_dir,
            Some(PathBuf::from("/tmp/agenthub-claude"))
        );

        let Cli {
            provider: ProviderCommand::Claude(claude),
        } = Cli::parse_from(["agenthub-acp", "claude", "--log-file", "claude.log"])
        else {
            panic!("expected claude provider");
        };
        let upstream = UpstreamCli::from(claude);
        assert!(upstream.diagnostic);
        assert_eq!(upstream.log_file, Some("claude.log".to_string()));
    }

    #[test]
    fn normalizes_upstream_default_service_name() {
        assert_eq!(
            normalize_otel_service_name("claude-code-acp-rs".to_string()),
            "agenthub-acp"
        );
    }
}
