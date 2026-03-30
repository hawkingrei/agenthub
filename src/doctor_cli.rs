use std::path::PathBuf;

use agenthub_managed_skills::install_managed_skills;
use clap::{CommandFactory, Parser, error::ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoctorCommand {
    Run,
    Help,
}

#[derive(Debug, Parser)]
#[command(
    name = "doctor",
    bin_name = "agenthub doctor",
    about = "Materialize managed AgentHub runtime skills under ~/.agents/skills/agenthub-runtime and exit.",
    disable_help_subcommand = true
)]
struct DoctorCli;

fn render_doctor_help() -> String {
    let mut command = DoctorCli::command();
    let mut buffer = Vec::new();
    command
        .write_long_help(&mut buffer)
        .expect("render doctor help");
    String::from_utf8(buffer).expect("doctor help should be utf8")
}

fn parse_doctor_args(args: &[String]) -> anyhow::Result<DoctorCommand> {
    if matches!(args, [arg] if arg.trim() == "help") {
        return Ok(DoctorCommand::Help);
    }

    let argv = std::iter::once("doctor".to_string()).chain(args.iter().cloned());
    match DoctorCli::try_parse_from(argv) {
        Ok(_) => Ok(DoctorCommand::Run),
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

fn run_doctor_command(command: DoctorCommand) -> anyhow::Result<()> {
    match command {
        DoctorCommand::Help => {
            print!("{}", render_doctor_help());
        }
        DoctorCommand::Run => {
            let installed = install_managed_skills(None)?;
            println!("{}", render_install_report(&installed));
        }
    }
    Ok(())
}

pub async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    let command = parse_doctor_args(args)?;
    run_doctor_command(command)
}

#[cfg(test)]
mod tests {
    use super::{DoctorCommand, parse_doctor_args, render_doctor_help, render_install_report};
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
        assert!(render_doctor_help().contains("managed AgentHub runtime skills"));
    }
}
