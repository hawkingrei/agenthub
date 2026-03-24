use std::path::PathBuf;

use agenthub_managed_skills::install_managed_skills;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoctorCommand {
    Run,
    Help,
}

fn is_help_flag(arg: &str) -> bool {
    matches!(arg.trim(), "--help" | "-h")
}

fn doctor_usage() -> &'static str {
    "Usage:\n  agenthub doctor\n  agenthub doctor --help\n\nBehavior:\n  Materialize managed AgentHub runtime skills under ~/.agents/skills/agenthub-runtime and exit.\n"
}

fn parse_doctor_args(args: &[String]) -> anyhow::Result<DoctorCommand> {
    match args {
        [] => Ok(DoctorCommand::Run),
        [arg] if is_help_flag(arg) || arg.trim() == "help" => Ok(DoctorCommand::Help),
        [arg] => Err(anyhow::anyhow!(
            "unknown flag for doctor: {}\n{}",
            arg,
            doctor_usage()
        )),
        _ => Err(anyhow::anyhow!(
            "doctor does not accept positional arguments\n{}",
            doctor_usage()
        )),
    }
}

fn render_install_report(installed: &[PathBuf]) -> String {
    if installed.is_empty() {
        return "No managed skills were installed because HOME is not set.".to_string();
    }

    let mut lines = vec![format!(
        "Installed {} managed skill document(s):",
        installed.len()
    )];
    lines.extend(installed.iter().map(|path| format!("- {}", path.display())));
    lines.join("\n")
}

fn run_doctor_command(command: DoctorCommand) -> anyhow::Result<()> {
    match command {
        DoctorCommand::Help => {
            println!("{}", doctor_usage());
        }
        DoctorCommand::Run => {
            let installed = install_managed_skills(None)?;
            println!("{}", render_install_report(&installed));
        }
    }
    Ok(())
}

pub async fn maybe_run_from_args() -> Option<anyhow::Result<()>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("doctor") {
        return None;
    }

    Some(match parse_doctor_args(&args[1..]) {
        Ok(command) => run_doctor_command(command),
        Err(err) => Err(err),
    })
}

#[cfg(test)]
mod tests {
    use super::{DoctorCommand, doctor_usage, parse_doctor_args, render_install_report};
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
        assert!(err.to_string().contains("unknown flag for doctor"));
        assert!(err.to_string().contains("agenthub doctor"));
    }

    #[test]
    fn render_install_report_lists_materialized_paths() {
        let output = render_install_report(&[
            PathBuf::from("/tmp/a/SKILL.md"),
            PathBuf::from("/tmp/b/SKILL.md"),
        ]);
        assert!(output.contains("Installed 2 managed skill document(s):"));
        assert!(output.contains("/tmp/a/SKILL.md"));
        assert!(output.contains("/tmp/b/SKILL.md"));
    }

    #[test]
    fn doctor_usage_mentions_managed_skills() {
        assert!(doctor_usage().contains("managed AgentHub runtime skills"));
    }
}
