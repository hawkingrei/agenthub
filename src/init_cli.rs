use std::{
    fs,
    io::{self, BufRead, IsTerminal, Write},
    path::PathBuf,
};

use anyhow::Context;
use clap::{CommandFactory, Parser, error::ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitCommand {
    Run,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitRole {
    Main,
    Node,
}

impl InitRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Node => "node",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalGrpcAnswers {
    enabled: bool,
    listen: String,
    security_mode: String,
    cert_dir: String,
    shared_secret: String,
    issuer: String,
    audience: String,
    bootstrap_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitAnswers {
    role: InitRole,
    listen: Option<String>,
    node_id: Option<String>,
    internal_grpc: Option<InternalGrpcAnswers>,
}

#[derive(Debug, Parser)]
#[command(
    name = "init",
    bin_name = "agenthub init",
    about = "Create a first-run AgentHub config under ~/.agenthub/config.toml.",
    disable_help_subcommand = true
)]
struct InitCli;

fn render_init_help_result() -> anyhow::Result<String> {
    let mut command = InitCli::command();
    let mut buffer = Vec::new();
    command
        .write_long_help(&mut buffer)
        .context("render init help")?;
    String::from_utf8(buffer).context("init help output should be valid utf8")
}

fn parse_init_args(args: &[String]) -> anyhow::Result<InitCommand> {
    if matches!(args, [arg] if arg.trim() == "help") {
        return Ok(InitCommand::Help);
    }

    let argv = std::iter::once("init".to_string()).chain(args.iter().cloned());
    match InitCli::try_parse_from(argv) {
        Ok(_) => Ok(InitCommand::Run),
        Err(err) if err.kind() == ErrorKind::DisplayHelp => Ok(InitCommand::Help),
        Err(err) => Err(err.into()),
    }
}

fn prompt_line<R, W>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: Option<&str>,
) -> anyhow::Result<String>
where
    R: BufRead,
    W: Write,
{
    match default {
        Some(default) => write!(writer, "{} [{}]: ", label, default)?,
        None => write!(writer, "{}: ", label)?,
    }
    writer.flush()?;

    let mut line = String::new();
    reader.read_line(&mut line).context("read init input")?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(default.unwrap_or_default().to_string());
    }
    Ok(trimmed.to_string())
}

fn prompt_required_line<R, W>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: Option<&str>,
) -> anyhow::Result<String>
where
    R: BufRead,
    W: Write,
{
    loop {
        let value = prompt_line(reader, writer, label, default)?;
        if !value.trim().is_empty() {
            return Ok(value);
        }
        writeln!(writer, "A value is required for {}.", label)?;
    }
}

fn prompt_bool<R, W>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: bool,
) -> anyhow::Result<bool>
where
    R: BufRead,
    W: Write,
{
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        write!(writer, "{} {}: ", label, suffix)?;
        writer.flush()?;

        let mut line = String::new();
        reader.read_line(&mut line).context("read init input")?;
        let trimmed = line.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Ok(default);
        }
        match trimmed.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => writeln!(writer, "Please answer yes or no.")?,
        }
    }
}

fn prompt_role<R, W>(reader: &mut R, writer: &mut W) -> anyhow::Result<InitRole>
where
    R: BufRead,
    W: Write,
{
    loop {
        let role = prompt_required_line(reader, writer, "Instance role (main/node)", Some("main"))?;
        match role.trim().to_ascii_lowercase().as_str() {
            "main" => return Ok(InitRole::Main),
            "node" => return Ok(InitRole::Node),
            _ => writeln!(writer, "Role must be either `main` or `node`.")?,
        }
    }
}

fn prompt_internal_grpc<R, W>(
    reader: &mut R,
    writer: &mut W,
    required: bool,
) -> anyhow::Result<Option<InternalGrpcAnswers>>
where
    R: BufRead,
    W: Write,
{
    if !required
        && !prompt_bool(
            reader,
            writer,
            "Enable internal gRPC now (required for remote nodes)",
            false,
        )?
    {
        return Ok(None);
    }

    let listen = prompt_required_line(
        reader,
        writer,
        "internal_grpc.listen",
        Some("127.0.0.1:50051"),
    )?;
    let security_mode =
        prompt_required_line(reader, writer, "internal_grpc.security.mode", Some("tls"))?;
    let cert_dir = prompt_required_line(
        reader,
        writer,
        "internal_grpc.security.cert_dir",
        Some("~/.agenthub/internal-grpc"),
    )?;
    let shared_secret =
        prompt_required_line(reader, writer, "internal_grpc.auth.shared_secret", None)?;
    let issuer = prompt_required_line(
        reader,
        writer,
        "internal_grpc.auth.issuer",
        Some("agenthub"),
    )?;
    let audience = prompt_required_line(
        reader,
        writer,
        "internal_grpc.auth.audience",
        Some("agenthub-internal"),
    )?;
    let bootstrap_token =
        prompt_required_line(reader, writer, "internal_grpc.bootstrap.token", None)?;

    Ok(Some(InternalGrpcAnswers {
        enabled: true,
        listen,
        security_mode,
        cert_dir,
        shared_secret,
        issuer,
        audience,
        bootstrap_token,
    }))
}

fn collect_init_answers<R, W>(reader: &mut R, writer: &mut W) -> anyhow::Result<InitAnswers>
where
    R: BufRead,
    W: Write,
{
    writeln!(
        writer,
        "AgentHub init will write a config file to {}.",
        agenthub_config::config_path().display()
    )?;
    let role = prompt_role(reader, writer)?;
    let listen = if role == InitRole::Main {
        Some(prompt_required_line(
            reader,
            writer,
            "server.listen",
            Some("127.0.0.1:8080"),
        )?)
    } else {
        None
    };
    let node_id = if role == InitRole::Node {
        Some(prompt_required_line(
            reader,
            writer,
            "server.node_id",
            None,
        )?)
    } else {
        None
    };
    let internal_grpc = prompt_internal_grpc(reader, writer, role == InitRole::Node)?;
    Ok(InitAnswers {
        role,
        listen,
        node_id,
        internal_grpc,
    })
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_init_config(answers: &InitAnswers) -> String {
    let mut lines = Vec::new();

    lines.push("[server]".to_string());
    lines.push(format!("role = \"{}\"", answers.role.as_str()));
    if let Some(listen) = &answers.listen {
        lines.push(format!("listen = \"{}\"", toml_escape(listen)));
    }
    if let Some(node_id) = &answers.node_id {
        lines.push(format!("node_id = \"{}\"", toml_escape(node_id)));
    }

    if let Some(internal_grpc) = &answers.internal_grpc {
        lines.push(String::new());
        lines.push("[internal_grpc]".to_string());
        lines.push(format!("enabled = {}", internal_grpc.enabled));
        lines.push(format!(
            "listen = \"{}\"",
            toml_escape(&internal_grpc.listen)
        ));

        lines.push(String::new());
        lines.push("[internal_grpc.security]".to_string());
        lines.push(format!(
            "mode = \"{}\"",
            toml_escape(&internal_grpc.security_mode)
        ));
        lines.push(format!(
            "cert_dir = \"{}\"",
            toml_escape(&internal_grpc.cert_dir)
        ));

        lines.push(String::new());
        lines.push("[internal_grpc.auth]".to_string());
        lines.push(format!(
            "shared_secret = \"{}\"",
            toml_escape(&internal_grpc.shared_secret)
        ));
        lines.push(format!(
            "issuer = \"{}\"",
            toml_escape(&internal_grpc.issuer)
        ));
        lines.push(format!(
            "audience = \"{}\"",
            toml_escape(&internal_grpc.audience)
        ));

        lines.push(String::new());
        lines.push("[internal_grpc.bootstrap]".to_string());
        lines.push(format!(
            "token = \"{}\"",
            toml_escape(&internal_grpc.bootstrap_token)
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

fn maybe_confirm_overwrite<R, W>(
    reader: &mut R,
    writer: &mut W,
    path: &PathBuf,
) -> anyhow::Result<()>
where
    R: BufRead,
    W: Write,
{
    if path.exists()
        && !prompt_bool(
            reader,
            writer,
            &format!("Config {} already exists. Overwrite it", path.display()),
            false,
        )?
    {
        anyhow::bail!("init cancelled; existing config was kept unchanged");
    }
    Ok(())
}

fn write_config_file(path: &PathBuf, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("write config file {}", path.display()))?;
    Ok(())
}

fn print_post_init_notes<W: Write>(writer: &mut W, answers: &InitAnswers) -> anyhow::Result<()> {
    writeln!(
        writer,
        "\nWrote {}",
        agenthub_config::config_path().display()
    )?;
    match answers.role {
        InitRole::Main => {
            writeln!(
                writer,
                "This machine will start as the main control plane. Run `agenthub` or `brew services start linkerdog/homebrew-tap/agenthub` after reviewing the config."
            )?;
        }
        InitRole::Node => {
            writeln!(
                writer,
                "This machine will start as a remote node. Run `agenthub` or `brew services start linkerdog/homebrew-tap/agenthub` after reviewing the config."
            )?;
            writeln!(
                writer,
                "ACP provider API base URLs and API keys are not configured by `agenthub init` yet. Configure any provider-specific runtime credentials separately before launching production workloads."
            )?;
        }
    }
    Ok(())
}

fn run_init_command<R, W>(
    command: InitCommand,
    reader: &mut R,
    writer: &mut W,
) -> anyhow::Result<()>
where
    R: BufRead,
    W: Write,
{
    match command {
        InitCommand::Help => {
            write!(writer, "{}", render_init_help_result()?)?;
        }
        InitCommand::Run => {
            let path = agenthub_config::config_path();
            maybe_confirm_overwrite(reader, writer, &path)?;
            let answers = collect_init_answers(reader, writer)?;
            let content = render_init_config(&answers);
            write_config_file(&path, &content)?;
            print_post_init_notes(writer, &answers)?;
        }
    }
    Ok(())
}

pub async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    let command = parse_init_args(args)?;
    if matches!(command, InitCommand::Run)
        && (!io::stdin().is_terminal() || !io::stdout().is_terminal())
    {
        anyhow::bail!("`agenthub init` currently requires an interactive terminal");
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut writer = io::BufWriter::new(stdout.lock());
    run_init_command(command, &mut reader, &mut writer)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        InitAnswers, InitCommand, InitRole, InternalGrpcAnswers, collect_init_answers,
        parse_init_args, render_init_config, render_init_help_result, run_init_command,
    };

    #[test]
    fn parse_init_defaults_to_run() {
        let parsed = parse_init_args(&[]).expect("parse init");
        assert_eq!(parsed, InitCommand::Run);
    }

    #[test]
    fn parse_init_accepts_help_flag() {
        let parsed = parse_init_args(&["--help".to_string()]).expect("parse init help");
        assert_eq!(parsed, InitCommand::Help);
    }

    #[test]
    fn parse_init_rejects_unknown_flag() {
        let err = parse_init_args(&["--verbose".to_string()]).expect_err("reject unknown flag");
        assert!(err.to_string().contains("unexpected argument '--verbose'"));
        assert!(err.to_string().contains("agenthub init"));
    }

    #[test]
    fn init_help_mentions_first_run_config() {
        let help = render_init_help_result().expect("render init help");
        assert!(help.contains("first-run AgentHub config"));
    }

    #[test]
    fn render_init_config_for_main_mode_omits_node_fields() {
        let config = render_init_config(&InitAnswers {
            role: InitRole::Main,
            listen: Some("127.0.0.1:8080".to_string()),
            node_id: None,
            internal_grpc: None,
        });
        assert!(config.contains("role = \"main\""));
        assert!(config.contains("listen = \"127.0.0.1:8080\""));
        assert!(!config.contains("node_id"));
        assert!(!config.contains("[internal_grpc]"));
    }

    #[test]
    fn render_init_config_for_node_mode_includes_internal_grpc_contract() {
        let config = render_init_config(&InitAnswers {
            role: InitRole::Node,
            listen: None,
            node_id: Some("node-east".to_string()),
            internal_grpc: Some(InternalGrpcAnswers {
                enabled: true,
                listen: "0.0.0.0:50051".to_string(),
                security_mode: "tls".to_string(),
                cert_dir: "~/.agenthub/internal-grpc".to_string(),
                shared_secret: "secret".to_string(),
                issuer: "agenthub".to_string(),
                audience: "agenthub-internal".to_string(),
                bootstrap_token: "token".to_string(),
            }),
        });
        assert!(config.contains("role = \"node\""));
        assert!(config.contains("node_id = \"node-east\""));
        assert!(config.contains("[internal_grpc]"));
        assert!(config.contains("shared_secret = \"secret\""));
        assert!(config.contains("token = \"token\""));
    }

    #[test]
    fn collect_init_answers_supports_main_without_internal_grpc() {
        let input = b"main\n127.0.0.1:8080\nn\n";
        let mut reader = Cursor::new(&input[..]);
        let mut output = Vec::new();
        let answers = collect_init_answers(&mut reader, &mut output).expect("collect answers");
        assert_eq!(
            answers,
            InitAnswers {
                role: InitRole::Main,
                listen: Some("127.0.0.1:8080".to_string()),
                node_id: None,
                internal_grpc: None,
            }
        );
    }

    #[test]
    fn collect_init_answers_requires_node_bootstrap_fields() {
        let input = b"node\nnode-east\n0.0.0.0:50051\ntls\n~/.agenthub/internal-grpc\nshared-secret\nagenthub\nagenthub-internal\nbootstrap-token\n";
        let mut reader = Cursor::new(&input[..]);
        let mut output = Vec::new();
        let answers = collect_init_answers(&mut reader, &mut output).expect("collect answers");
        assert_eq!(answers.role, InitRole::Node);
        assert_eq!(answers.node_id.as_deref(), Some("node-east"));
        let internal_grpc = answers
            .internal_grpc
            .expect("node mode should require internal grpc");
        assert_eq!(internal_grpc.listen, "0.0.0.0:50051");
        assert_eq!(internal_grpc.bootstrap_token, "bootstrap-token");
    }

    #[test]
    fn run_init_help_writes_help_text() {
        let mut reader = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        run_init_command(InitCommand::Help, &mut reader, &mut output).expect("run help");
        let text = String::from_utf8(output).expect("utf8");
        assert!(text.contains("agenthub init"));
    }
}
