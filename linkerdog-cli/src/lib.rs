#![deny(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkerdogCommand {
    Acp { raw_overrides: Vec<String> },
}

#[derive(Parser, Debug, Default, Clone)]
#[command(name = "linkerdog")]
struct CliArgs {
    /// Override runtime defaults using key=value.
    #[arg(
        short = 'c',
        long = "config",
        value_name = "key=value",
        action = clap::ArgAction::Append,
        global = true,
    )]
    raw_overrides: Vec<String>,

    #[command(subcommand)]
    command: Option<CliSubcommand>,
}

#[derive(Subcommand, Debug, Clone)]
enum CliSubcommand {
    /// Start ACP runtime.
    Acp,
}

pub fn parse_command_from<I, T>(args: I) -> Result<LinkerdogCommand, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = CliArgs::try_parse_from(args)?;
    let command = match cli.command {
        Some(CliSubcommand::Acp) | None => LinkerdogCommand::Acp {
            raw_overrides: cli.raw_overrides,
        },
    };
    Ok(command)
}

pub async fn run_from_args<I>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    match parse_command_from(args).map_err(anyhow::Error::from)? {
        LinkerdogCommand::Acp { raw_overrides } => {
            linkerdog_acp::run_from_overrides(&raw_overrides).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LinkerdogCommand, parse_command_from};

    #[test]
    fn parse_default_command_routes_to_acp() {
        let command = parse_command_from(["linkerdog"]).expect("parse command");
        assert_eq!(
            command,
            LinkerdogCommand::Acp {
                raw_overrides: Vec::new()
            }
        );
    }

    #[test]
    fn parse_acp_subcommand_routes_to_acp() {
        let command = parse_command_from(["linkerdog", "acp"]).expect("parse command");
        assert_eq!(
            command,
            LinkerdogCommand::Acp {
                raw_overrides: Vec::new()
            }
        );
    }

    #[test]
    fn parse_config_overrides_for_acp() {
        let command = parse_command_from([
            "linkerdog",
            "acp",
            "-c",
            "provider=deepseek",
            "-c",
            "mode=review",
        ])
        .expect("parse command");
        assert_eq!(
            command,
            LinkerdogCommand::Acp {
                raw_overrides: vec!["provider=deepseek".to_string(), "mode=review".to_string(),],
            }
        );
    }
}
