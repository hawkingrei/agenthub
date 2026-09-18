use std::path::Path;

use agenthub_config::RaraLaunchConfig;

use crate::protocol::validate_label;
use crate::{PROTOCOL_VERSION, ProtocolError, TRANSPORT};

/// Exact argv for an already authorized local workspace; never invokes a shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl LaunchCommand {
    pub fn new(config: &RaraLaunchConfig, workspace: &Path) -> Result<Self, ProtocolError> {
        if !workspace.is_absolute()
            || config.binary.trim().is_empty()
            || config.binary.chars().any(char::is_control)
        {
            return Err(ProtocolError::InvalidTarget);
        }
        let workspace = workspace.to_str().ok_or(ProtocolError::InvalidTarget)?;
        if workspace.chars().any(char::is_control) {
            return Err(ProtocolError::InvalidTarget);
        }
        let mut args = vec![
            "app-server".into(),
            "--protocol-version".into(),
            PROTOCOL_VERSION.to_string(),
            "--transport".into(),
            TRANSPORT.into(),
            "--cwd".into(),
            workspace.into(),
        ];
        for (flag, value) in [
            ("--provider", &config.default_provider),
            ("--model", &config.default_model),
        ] {
            if let Some(value) = value {
                validate_label(value)?;
                // One argv value prevents a label beginning with '-' from becoming another flag.
                args.push(format!("{flag}={value}"));
            }
        }
        Ok(Self {
            program: config.binary.clone(),
            args,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_startup_mode_preserves_argv_boundaries_and_native_permission_policy() {
        let config = agenthub_config::RaraConfig {
            binary: Some("/opt/custom runtime".into()),
            default_provider: Some("fixture".into()),
            default_model: Some("--dangerously-skip-permissions".into()),
            ..Default::default()
        }
        .resolve_with(|_| None)
        .unwrap();
        let workspace = std::env::temp_dir().join("workspace with spaces");
        let command = LaunchCommand::new(&config, &workspace).unwrap();
        assert_eq!(command.program, "/opt/custom runtime");
        assert_eq!(
            &command.args[..5],
            [
                "app-server",
                "--protocol-version",
                "1",
                "--transport",
                "stdio-jsonl"
            ]
        );
        assert_eq!(command.args[6], workspace.to_str().unwrap());
        assert!(
            command
                .args
                .contains(&"--model=--dangerously-skip-permissions".into())
        );
        assert!(
            !command
                .args
                .contains(&"--dangerously-skip-permissions".into())
        );
        assert!(!command.args.contains(&"--api-key".into()));
        assert_eq!(
            LaunchCommand::new(&config, Path::new("relative")),
            Err(ProtocolError::InvalidTarget)
        );
    }
}
