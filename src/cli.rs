use std::ffi::OsString;

use clap::{Args, Parser, Subcommand, error::ErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RootCliCommand {
    Serve,
    Init { args: Vec<String> },
    Doctor { args: Vec<String> },
    Actor { args: Vec<String> },
    LegacyActorMcp,
}

#[derive(Debug, Parser)]
#[command(name = "agenthub", version, disable_help_subcommand = true)]
struct AgentHubCli {
    #[command(subcommand)]
    command: Option<AgentHubSubcommand>,
}

#[derive(Debug, Subcommand)]
enum AgentHubSubcommand {
    Init(PassthroughArgs),
    Doctor(PassthroughArgs),
    Actor(PassthroughArgs),
    #[command(name = "actor-mcp", hide = true)]
    ActorMcp(PassthroughArgs),
}

#[derive(Debug, Default, Args)]
#[command(
    disable_help_flag = true,
    disable_help_subcommand = true,
    trailing_var_arg = true
)]
struct PassthroughArgs {
    #[arg(num_args = 0.., allow_hyphen_values = true, value_name = "ARGS")]
    args: Vec<String>,
}

pub(crate) fn parse_root_cli_from<I, T>(iter: I) -> Result<RootCliCommand, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = AgentHubCli::try_parse_from(iter)?;
    Ok(match cli.command {
        None => RootCliCommand::Serve,
        Some(AgentHubSubcommand::Init(args)) => RootCliCommand::Init { args: args.args },
        Some(AgentHubSubcommand::Doctor(args)) => RootCliCommand::Doctor { args: args.args },
        Some(AgentHubSubcommand::Actor(args)) => RootCliCommand::Actor { args: args.args },
        Some(AgentHubSubcommand::ActorMcp(_)) => RootCliCommand::LegacyActorMcp,
    })
}

pub(crate) fn parse_root_cli_from_env() -> Result<RootCliCommand, clap::Error> {
    parse_root_cli_from(std::env::args_os())
}

pub(crate) fn is_non_error_clap_exit(err: &clap::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    )
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::{RootCliCommand, is_non_error_clap_exit, parse_root_cli_from};

    #[test]
    fn parse_root_defaults_to_serve_without_subcommand() {
        let parsed = parse_root_cli_from(["agenthub"]).expect("parse root");
        assert_eq!(parsed, RootCliCommand::Serve);
    }

    #[test]
    fn parse_root_preserves_doctor_passthrough_args() {
        let parsed = parse_root_cli_from(["agenthub", "doctor", "--help"]).expect("parse doctor");
        assert_eq!(
            parsed,
            RootCliCommand::Doctor {
                args: vec!["--help".to_string()]
            }
        );
    }

    #[test]
    fn parse_root_preserves_init_passthrough_args() {
        let parsed = parse_root_cli_from(["agenthub", "init", "--help"]).expect("parse init");
        assert_eq!(
            parsed,
            RootCliCommand::Init {
                args: vec!["--help".to_string()]
            }
        );
    }

    #[test]
    fn parse_root_preserves_actor_passthrough_args() {
        let parsed = parse_root_cli_from(["agenthub", "actor", "--json", "inbox", "--help"])
            .expect("parse actor");
        assert_eq!(
            parsed,
            RootCliCommand::Actor {
                args: vec![
                    "--json".to_string(),
                    "inbox".to_string(),
                    "--help".to_string()
                ]
            }
        );
    }

    #[test]
    fn parse_root_maps_legacy_actor_mcp_command() {
        let parsed = parse_root_cli_from(["agenthub", "actor-mcp"]).expect("parse actor-mcp");
        assert_eq!(parsed, RootCliCommand::LegacyActorMcp);
    }

    #[test]
    fn parse_root_keeps_legacy_actor_mcp_help_on_removed_path() {
        let parsed =
            parse_root_cli_from(["agenthub", "actor-mcp", "--help"]).expect("parse actor-mcp");
        assert_eq!(parsed, RootCliCommand::LegacyActorMcp);
    }

    #[test]
    fn parse_root_reports_help_without_treating_it_as_failure() {
        let err = parse_root_cli_from(["agenthub", "--help"]).expect_err("display help");
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        assert!(is_non_error_clap_exit(&err));
    }

    #[test]
    fn parse_root_rejects_unknown_top_level_subcommands() {
        let err = parse_root_cli_from(["agenthub", "unknown"]).expect_err("reject unknown");
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
        assert!(err.to_string().contains("unknown"));
    }
}
