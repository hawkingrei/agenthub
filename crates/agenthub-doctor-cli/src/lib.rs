use std::path::PathBuf;

use agenthub_managed_skills::install_managed_skills;
use anyhow::Context;
use clap::{Args, CommandFactory, Parser, Subcommand, error::ErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
enum DoctorCommand {
    Run,
    Help,
    AgentTrace(AgentTraceCli),
}

#[derive(Debug, Parser)]
#[command(
    name = "doctor",
    bin_name = "agenthub doctor",
    about = "Run AgentHub diagnostics. Without a subcommand, materialize managed AgentHub runtime skills.",
    disable_help_subcommand = true
)]
struct DoctorCli {
    #[command(subcommand)]
    command: Option<DoctorSubcommand>,
}

#[derive(Debug, Clone, Subcommand, PartialEq, Eq)]
enum DoctorSubcommand {
    #[command(
        name = "agent-trace",
        about = "Inspect read-only agent delivery metadata for stalled ACP/team sessions. Debug builds only."
    )]
    AgentTrace(AgentTraceCli),
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
struct AgentTraceCli {
    #[arg(long, conflicts_with_all = ["team_id", "member_id"])]
    agent_id: Option<String>,
    #[arg(long, requires = "member_id")]
    team_id: Option<String>,
    #[arg(long, requires = "team_id")]
    member_id: Option<String>,
    #[arg(long)]
    session_id: Option<String>,
    #[arg(long, default_value_t = 16)]
    event_limit: i64,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    server_url: Option<String>,
    #[arg(long)]
    token: Option<String>,
}

fn render_doctor_help_result() -> anyhow::Result<String> {
    let mut command = DoctorCli::command();
    let mut buffer = Vec::new();
    command
        .write_long_help(&mut buffer)
        .context("render doctor help")?;
    String::from_utf8(buffer).context("doctor help output should be valid utf8")
}

fn parse_doctor_args(args: &[String]) -> anyhow::Result<DoctorCommand> {
    if matches!(args, [arg] if arg.trim() == "help") {
        return Ok(DoctorCommand::Help);
    }

    let argv = std::iter::once("doctor".to_string()).chain(args.iter().cloned());
    match DoctorCli::try_parse_from(argv) {
        Ok(cli) => match cli.command {
            Some(DoctorSubcommand::AgentTrace(args)) => Ok(DoctorCommand::AgentTrace(args)),
            None => Ok(DoctorCommand::Run),
        },
        Err(err) if err.kind() == ErrorKind::DisplayHelp => Ok(DoctorCommand::Help),
        Err(err) => Err(err.into()),
    }
}

fn render_install_report(installed: &[PathBuf]) -> String {
    if installed.is_empty() {
        return "No managed skills were installed because no home directory environment variable (HOME/USERPROFILE) is set.".to_string();
    }

    let mut lines = vec![format!(
        "Ensured {} managed skill document(s):",
        installed.len()
    )];
    lines.extend(installed.iter().map(|path| format!("- {}", path.display())));
    lines.join("\n")
}

async fn run_doctor_command(command: DoctorCommand) -> anyhow::Result<()> {
    match command {
        DoctorCommand::Help => {
            print!("{}", render_doctor_help_result()?);
        }
        DoctorCommand::Run => {
            let installed = install_managed_skills(None)?;
            println!("{}", render_install_report(&installed));
        }
        DoctorCommand::AgentTrace(args) => {
            run_agent_trace(args).await?;
        }
    }
    Ok(())
}

pub async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    let command = parse_doctor_args(args)?;
    run_doctor_command(command).await
}

#[cfg(debug_assertions)]
async fn run_agent_trace(args: AgentTraceCli) -> anyhow::Result<()> {
    use agenthub_diagnostics::agent_trace::{
        AgentTraceRequest, collect_from_default_paths, render_human,
    };

    let server_url = args
        .server_url
        .clone()
        .or_else(|| std::env::var("AGENTHUB_SERVER_URL").ok());
    let token = args
        .token
        .clone()
        .or_else(|| std::env::var("AGENTHUB_TOKEN").ok());
    let request = AgentTraceRequest {
        agent_id: args.agent_id,
        team_id: args.team_id,
        member_id: args.member_id,
        session_id: args.session_id,
        event_limit: args.event_limit,
    };
    let json_output = args.json;
    let report = if let Some(server_url) = server_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        let token = token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("--token or AGENTHUB_TOKEN is required with --server-url")
            })?;
        collect_from_live_server(server_url, token, &request).await?
    } else {
        collect_from_default_paths(request).await?
    };
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", render_human(&report));
    }
    Ok(())
}

#[cfg(debug_assertions)]
async fn collect_from_live_server(
    server_url: &str,
    token: &str,
    request: &agenthub_diagnostics::agent_trace::AgentTraceRequest,
) -> anyhow::Result<agenthub_diagnostics::agent_trace::AgentTraceReport> {
    let mut url = reqwest::Url::parse(server_url)
        .or_else(|_| reqwest::Url::parse(&format!("http://{server_url}")))
        .context("parse agenthub server url")?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("server url cannot be used as a base"))?
        .extend(["api", "diagnostics", "agent_trace"]);
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(agent_id) = request.agent_id.as_deref() {
            pairs.append_pair("agent_id", agent_id);
        }
        if let Some(team_id) = request.team_id.as_deref() {
            pairs.append_pair("team_id", team_id);
        }
        if let Some(member_id) = request.member_id.as_deref() {
            pairs.append_pair("member_id", member_id);
        }
        if let Some(session_id) = request.session_id.as_deref() {
            pairs.append_pair("session_id", session_id);
        }
        pairs.append_pair("event_limit", &request.event_limit.to_string());
    }

    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context("request live agent trace")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("live agent trace request failed: {status} {body}");
    }
    response
        .json::<agenthub_diagnostics::agent_trace::AgentTraceReport>()
        .await
        .context("decode live agent trace response")
}

#[cfg(not(debug_assertions))]
async fn run_agent_trace(_args: AgentTraceCli) -> anyhow::Result<()> {
    anyhow::bail!("agent-trace diagnostics are only available in debug builds")
}

#[cfg(test)]
mod tests {
    use super::{
        DoctorCommand, parse_doctor_args, render_doctor_help_result, render_install_report,
    };
    use std::path::PathBuf;

    #[test]
    fn parse_doctor_defaults_to_run() {
        let parsed = parse_doctor_args(&[]).expect("parse doctor");
        assert_eq!(parsed, DoctorCommand::Run);
    }

    #[test]
    fn parse_doctor_accepts_help_flag() {
        let parsed = parse_doctor_args(&["--help".to_string()]).expect("parse doctor help");
        assert_eq!(parsed, DoctorCommand::Help);
    }

    #[test]
    fn parse_doctor_rejects_unknown_flag() {
        let err = parse_doctor_args(&["--verbose".to_string()]).expect_err("reject unknown flag");
        assert!(err.to_string().contains("unexpected argument '--verbose'"));
        assert!(err.to_string().contains("agenthub doctor"));
    }

    #[test]
    fn parse_doctor_accepts_agent_trace_agent_target() {
        let parsed = parse_doctor_args(&[
            "agent-trace".to_string(),
            "--agent-id".to_string(),
            "worker".to_string(),
            "--session-id".to_string(),
            "session-1".to_string(),
            "--event-limit".to_string(),
            "8".to_string(),
            "--json".to_string(),
            "--server-url".to_string(),
            "http://127.0.0.1:8080".to_string(),
            "--token".to_string(),
            "session-token".to_string(),
        ])
        .expect("parse agent trace");
        let DoctorCommand::AgentTrace(args) = parsed else {
            panic!("expected agent trace command");
        };
        assert_eq!(args.agent_id.as_deref(), Some("worker"));
        assert_eq!(args.session_id.as_deref(), Some("session-1"));
        assert_eq!(args.event_limit, 8);
        assert!(args.json);
        assert_eq!(args.server_url.as_deref(), Some("http://127.0.0.1:8080"));
        assert_eq!(args.token.as_deref(), Some("session-token"));
    }

    #[test]
    fn parse_doctor_accepts_agent_trace_team_target() {
        let parsed = parse_doctor_args(&[
            "agent-trace".to_string(),
            "--team-id".to_string(),
            "team-1".to_string(),
            "--member-id".to_string(),
            "worker".to_string(),
        ])
        .expect("parse team agent trace");
        let DoctorCommand::AgentTrace(args) = parsed else {
            panic!("expected agent trace command");
        };
        assert_eq!(args.team_id.as_deref(), Some("team-1"));
        assert_eq!(args.member_id.as_deref(), Some("worker"));
    }

    #[test]
    fn render_install_report_lists_materialized_paths() {
        let output = render_install_report(&[
            PathBuf::from("/tmp/a/SKILL.md"),
            PathBuf::from("/tmp/b/SKILL.md"),
        ]);
        assert!(output.contains("Ensured 2 managed skill document(s):"));
        assert!(output.contains("/tmp/a/SKILL.md"));
        assert!(output.contains("/tmp/b/SKILL.md"));
    }

    #[test]
    fn render_install_report_mentions_home_env_vars_when_missing() {
        let output = render_install_report(&[]);
        assert!(output.contains("HOME/USERPROFILE"));
    }

    #[test]
    fn doctor_help_mentions_managed_skills() {
        let help = render_doctor_help_result().expect("render doctor help");
        assert!(help.contains("Run AgentHub diagnostics"));
        assert!(help.contains("managed AgentHub runtime skills"));
        assert!(help.contains("agent-trace"));
    }
}
