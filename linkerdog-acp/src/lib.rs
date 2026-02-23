#![deny(clippy::print_stdout, clippy::print_stderr)]

use clap::Parser;
use linkerdog_core::{LinkerdogRuntimeConfig, normalize_acp_cli_args, run_main};

#[derive(Parser, Debug, Default, Clone)]
#[command(name = "linkerdog-acp")]
pub struct AcpCliArgs {
    /// Override runtime defaults using key=value.
    ///
    /// Supported keys:
    /// - provider
    /// - model
    /// - mode
    ///
    /// Namespaced aliases are also supported:
    /// - linkerdog.provider
    /// - linkerdog.model
    /// - linkerdog.mode
    #[arg(
        short = 'c',
        long = "config",
        value_name = "key=value",
        action = clap::ArgAction::Append,
        global = true,
    )]
    pub raw_overrides: Vec<String>,
}

pub async fn run_from_overrides(raw_overrides: &[String]) -> anyhow::Result<()> {
    let config =
        LinkerdogRuntimeConfig::from_raw_overrides(raw_overrides).map_err(anyhow::Error::msg)?;
    run_main(config).await.map_err(anyhow::Error::from)
}

pub async fn run_from_args<I>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let normalized = normalize_acp_cli_args(args);
    let cli = AcpCliArgs::parse_from(normalized);
    run_from_overrides(&cli.raw_overrides).await
}

#[cfg(test)]
mod tests {
    use super::AcpCliArgs;
    use clap::Parser;

    #[test]
    fn acp_cli_args_parses_repeated_config() {
        let parsed = AcpCliArgs::try_parse_from([
            "linkerdog-acp",
            "-c",
            "provider=openai",
            "-c",
            "model=gpt-5",
        ])
        .expect("parse args");
        assert_eq!(parsed.raw_overrides.len(), 2);
        assert_eq!(parsed.raw_overrides[0], "provider=openai");
        assert_eq!(parsed.raw_overrides[1], "model=gpt-5");
    }
}
