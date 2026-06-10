use std::path::PathBuf;

use clap::Parser;
use claude_code_acp::Cli as UpstreamCli;

const UPSTREAM_DEFAULT_OTEL_SERVICE_NAME: &str = "claude-code-acp-rs";
const AGENTHUB_OTEL_SERVICE_NAME: &str = "agenthub-claude-acp";

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(name = "agenthub-claude-acp")]
#[command(version, about = "AgentHub ACP adapter wrapper for Claude Code")]
pub struct Cli {
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

impl From<Cli> for UpstreamCli {
    fn from(cli: Cli) -> Self {
        Self {
            acp: true,
            prompt: None,
            diagnostic: cli.diagnostic,
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
    let upstream_cli = UpstreamCli::from(cli);
    claude_code_acp::run_acp_with_cli(&upstream_cli).await
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
    fn maps_to_upstream_acp_mode() {
        let upstream = UpstreamCli::from(Cli::parse_from(["agenthub-claude-acp"]));
        assert!(upstream.acp);
        assert!(upstream.prompt.is_none());
        assert_eq!(upstream.otel_service_name, "agenthub-claude-acp");
    }

    #[test]
    fn keeps_upstream_acp_flag_compatible() {
        let upstream = UpstreamCli::from(Cli::parse_from(["agenthub-claude-acp", "--acp"]));
        assert!(upstream.acp);
        assert!(upstream.prompt.is_none());
    }

    #[test]
    fn maps_diagnostic_and_logging_flags() {
        let upstream = UpstreamCli::from(Cli::parse_from([
            "agenthub-claude-acp",
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
        ]));
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
    fn normalizes_upstream_default_service_name() {
        assert_eq!(
            normalize_otel_service_name("claude-code-acp-rs".to_string()),
            "agenthub-claude-acp"
        );
    }
}
