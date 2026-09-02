use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};

use agenthub_acp_adapter::{Cli as AcpCli, ProviderCommand};
use clap::{Parser, Subcommand, error::ErrorKind};

const CLI_BINARY_NAME: &str = "agenthub";

#[derive(Debug, Parser)]
#[command(name = "agenthubd", version, about = "AgentHub daemon")]
struct DaemonCli {
    #[command(subcommand)]
    command: Option<DaemonCommand>,
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    /// Run an internal ACP provider worker over stdio.
    Acp {
        #[command(subcommand)]
        provider: ProviderCommand,
    },
}

enum Invocation {
    Daemon,
    Acp(AcpCli),
    ExitAfterClap,
}

pub fn run() -> anyhow::Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    match classify_invocation(args)? {
        Invocation::Daemon => block_on(agenthub::run_daemon()),
        Invocation::Acp(cli) => run_acp(cli),
        Invocation::ExitAfterClap => Ok(()),
    }
}

fn classify_invocation(args: Vec<OsString>) -> anyhow::Result<Invocation> {
    match DaemonCli::try_parse_from(args) {
        Ok(DaemonCli { command: None }) => Ok(Invocation::Daemon),
        Ok(DaemonCli {
            command: Some(DaemonCommand::Acp { provider }),
        }) => Ok(Invocation::Acp(AcpCli { provider })),
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            err.print()?;
            Ok(Invocation::ExitAfterClap)
        }
        Err(err) => Err(err.into()),
    }
}

fn run_acp(mut cli: AcpCli) -> anyhow::Result<()> {
    let actor_cli_path = resolve_sibling_cli_path(std::env::current_exe().ok().as_deref());
    let needs_codex_runtime =
        matches!(&cli.provider, ProviderCommand::Codex(codex) if codex.codex_binary.is_none());
    if needs_codex_runtime {
        let (config, _) = agenthub_config::AppConfig::load_with_info()?;
        configure_codex_runtime(&mut cli, &config);
    }
    block_on(agenthub_acp_adapter::run_with_shutdown(cli, actor_cli_path))
}

fn configure_codex_runtime(cli: &mut AcpCli, config: &agenthub_config::AppConfig) {
    if let ProviderCommand::Codex(codex) = &mut cli.provider
        && codex.codex_binary.is_none()
    {
        codex.codex_binary = Some(PathBuf::from(config.codex_runtime_binary()));
    }
}

fn block_on<F>(future: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(future)
}

fn resolve_sibling_cli_path(current_exe: Option<&Path>) -> Option<PathBuf> {
    let current_exe = current_exe?;
    let parent = current_exe.parent()?;
    let binary_dir = if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
        parent.parent()?
    } else {
        parent
    };
    let sibling = binary_dir.join(format!("{CLI_BINARY_NAME}{}", std::env::consts::EXE_SUFFIX));
    std::fs::canonicalize(sibling).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_default_invocation_as_daemon() {
        assert!(matches!(
            classify_invocation(vec![OsString::from("agenthubd")]).expect("classify daemon"),
            Invocation::Daemon
        ));
    }

    #[test]
    fn classifies_provider_worker() {
        let invocation = classify_invocation(
            ["agenthubd", "acp", "codex", "-c", "model=gpt-5"]
                .map(OsString::from)
                .to_vec(),
        )
        .expect("classify ACP worker");
        let Invocation::Acp(AcpCli {
            provider: ProviderCommand::Codex(codex),
        }) = invocation
        else {
            panic!("expected Codex ACP worker");
        };
        assert_eq!(codex.codex_binary, None);
        assert_eq!(codex.config_overrides, vec!["model=gpt-5"]);

        let invocation = classify_invocation(
            ["agenthubd", "acp", "claude", "--quiet"]
                .map(OsString::from)
                .to_vec(),
        )
        .expect("classify Claude ACP worker");
        assert!(matches!(
            invocation,
            Invocation::Acp(AcpCli {
                provider: ProviderCommand::Claude(_),
            })
        ));
    }

    #[test]
    fn configures_external_codex_runtime_without_overriding_cli_path() {
        let mut cli = AcpCli {
            provider: ProviderCommand::Codex(agenthub_acp_adapter::CodexCli {
                codex_binary: None,
                config_overrides: Vec::new(),
            }),
        };
        let config = agenthub_config::AppConfig {
            codex_acp: Some(agenthub_config::CodexAcpConfig {
                binary: None,
                runtime_binary: Some("/opt/codex/bin/codex".to_string()),
                default_mode: None,
                multi_agent_enabled: None,
            }),
            ..Default::default()
        };

        configure_codex_runtime(&mut cli, &config);
        let ProviderCommand::Codex(codex) = &mut cli.provider else {
            panic!("expected Codex provider");
        };
        assert_eq!(
            codex.codex_binary,
            Some(PathBuf::from("/opt/codex/bin/codex"))
        );

        codex.codex_binary = Some(PathBuf::from("/custom/codex"));
        configure_codex_runtime(&mut cli, &agenthub_config::AppConfig::default());
        let ProviderCommand::Codex(codex) = &cli.provider else {
            panic!("expected Codex provider");
        };
        assert_eq!(codex.codex_binary, Some(PathBuf::from("/custom/codex")));
    }

    #[test]
    fn resolves_only_an_existing_sibling_cli() {
        let root = std::env::temp_dir().join(format!(
            "agenthub-daemon-cli-path-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        std::fs::create_dir_all(&root).expect("create temp directory");
        let daemon = root.join(format!("agenthubd{}", std::env::consts::EXE_SUFFIX));
        let cli = root.join(format!("agenthub{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&daemon, b"daemon").expect("write daemon marker");
        assert_eq!(resolve_sibling_cli_path(Some(&daemon)), None);
        std::fs::write(&cli, b"cli").expect("write CLI marker");
        assert_eq!(
            resolve_sibling_cli_path(Some(&daemon)),
            std::fs::canonicalize(&cli).ok()
        );
        std::fs::remove_dir_all(root).expect("remove temp directory");
    }
}
