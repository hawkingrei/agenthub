#![deny(clippy::print_stdout, clippy::print_stderr)]

use clap::Parser;
use linkerdog_core::{LinkerdogRuntimeConfig, normalize_acp_cli_args, run_main};
use std::ffi::OsString;
use std::future::Future;
use std::io::Result as IoResult;

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

async fn run_from_overrides_with_runner<F, Fut>(
    raw_overrides: &[String],
    runner: F,
) -> anyhow::Result<()>
where
    F: FnOnce(LinkerdogRuntimeConfig) -> Fut,
    Fut: Future<Output = IoResult<()>>,
{
    let config =
        LinkerdogRuntimeConfig::from_raw_overrides(raw_overrides).map_err(anyhow::Error::msg)?;
    runner(config).await.map_err(anyhow::Error::from)
}

pub async fn run_from_overrides(raw_overrides: &[String]) -> anyhow::Result<()> {
    run_from_overrides_with_runner(raw_overrides, run_main).await
}

async fn run_from_args_with_runner<I, F, Fut>(args: I, runner: F) -> anyhow::Result<()>
where
    I: IntoIterator<Item = OsString>,
    F: FnOnce(LinkerdogRuntimeConfig) -> Fut,
    Fut: Future<Output = IoResult<()>>,
{
    let normalized = normalize_acp_cli_args(args);
    let cli = AcpCliArgs::parse_from(normalized);
    run_from_overrides_with_runner(&cli.raw_overrides, runner).await
}

pub async fn run_from_args<I>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = OsString>,
{
    run_from_args_with_runner(args, run_main).await
}

#[cfg(test)]
mod tests {
    use super::{AcpCliArgs, run_from_args_with_runner, run_from_overrides_with_runner};
    use clap::Parser;
    use linkerdog_core::LinkerdogRuntimeConfig;
    use std::cell::RefCell;
    use std::rc::Rc;

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

    #[tokio::test(flavor = "current_thread")]
    async fn run_from_overrides_forwards_parsed_runtime_config() {
        let raw = vec![
            "provider=google".to_string(),
            "model=gemini-2.5-flash".to_string(),
            "mode=review".to_string(),
        ];
        let captured = Rc::new(RefCell::new(None));
        let captured_runner = captured.clone();

        run_from_overrides_with_runner(&raw, move |config| {
            *captured_runner.borrow_mut() = Some(config);
            async { Ok(()) }
        })
        .await
        .expect("run from overrides");

        let config = captured.borrow().clone().expect("captured config");
        assert_eq!(
            config,
            LinkerdogRuntimeConfig {
                default_provider: "google".to_string(),
                default_model: "gemini-2.5-flash".to_string(),
                default_mode: "review".to_string(),
            }
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_from_args_normalizes_acp_subcommand() {
        let args = vec![
            std::ffi::OsString::from("linkerdog"),
            std::ffi::OsString::from("acp"),
            std::ffi::OsString::from("-c"),
            std::ffi::OsString::from("provider=anthropic"),
            std::ffi::OsString::from("-c"),
            std::ffi::OsString::from("model=claude-opus-4.1"),
        ];
        let captured = Rc::new(RefCell::new(None));
        let captured_runner = captured.clone();

        run_from_args_with_runner(args, move |config| {
            *captured_runner.borrow_mut() = Some(config);
            async { Ok(()) }
        })
        .await
        .expect("run from args");

        let config = captured.borrow().clone().expect("captured config");
        assert_eq!(config.default_provider, "anthropic");
        assert_eq!(config.default_model, "claude-opus-4.1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_from_overrides_rejects_invalid_key_value_pair() {
        let raw = vec!["provider".to_string()];
        let err = run_from_overrides_with_runner(&raw, |_| async { Ok(()) })
            .await
            .expect_err("invalid overrides should fail");
        assert!(err.to_string().contains("invalid override (missing '=')"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_from_overrides_maps_runner_io_error() {
        let err = run_from_overrides_with_runner(&[], |_| async {
            Err(std::io::Error::other("runner-failed"))
        })
        .await
        .expect_err("runner failure should propagate");
        assert!(err.to_string().contains("runner-failed"));
    }
}
