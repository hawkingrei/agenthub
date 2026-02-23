#![deny(clippy::print_stdout, clippy::print_stderr)]

mod agent;
mod runtime;

use clap::Parser;
use std::ffi::OsString;

pub use runtime::{run_from_args, run_main};

#[derive(Parser, Debug, Default, Clone)]
#[command(name = "linkerdog")]
pub struct CliArgs {
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

/// Runtime defaults parsed from CLI overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkerdogRuntimeConfig {
    pub default_provider: String,
    pub default_model: String,
    pub default_mode: String,
}

impl Default for LinkerdogRuntimeConfig {
    fn default() -> Self {
        Self {
            default_provider: "openai".to_string(),
            default_model: "gpt-5".to_string(),
            default_mode: "code".to_string(),
        }
    }
}

impl LinkerdogRuntimeConfig {
    pub fn from_raw_overrides(raw_overrides: &[String]) -> Result<Self, String> {
        let mut out = Self::default();

        for (raw_key, raw_value) in parse_cli_overrides(raw_overrides)? {
            let key = raw_key.trim().to_ascii_lowercase();
            let value = toml_value_to_string(&raw_value);
            if value.is_empty() {
                continue;
            }

            match key.as_str() {
                "provider" | "linkerdog.provider" | "agent.provider" => {
                    out.default_provider = value.to_ascii_lowercase();
                }
                "model" | "linkerdog.model" | "agent.model" => {
                    out.default_model = value;
                }
                "mode" | "linkerdog.mode" | "agent.mode" => {
                    out.default_mode = value.to_ascii_lowercase();
                }
                _ => {}
            }
        }

        Ok(out)
    }
}

/// Accept both `linkerdog` and `linkerdog acp` as entry forms.
pub fn normalize_cli_args<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    let mut normalized: Vec<OsString> = args.into_iter().collect();
    if normalized.get(1).and_then(|arg| arg.to_str()) == Some("acp") {
        normalized.remove(1);
    }
    normalized
}

pub fn parse_cli_overrides(raw_overrides: &[String]) -> Result<Vec<(String, toml::Value)>, String> {
    raw_overrides
        .iter()
        .map(|s| {
            let mut parts = s.splitn(2, '=');
            let key = parts
                .next()
                .map(str::trim)
                .ok_or_else(|| "override missing key".to_string())?;
            let value_str = parts
                .next()
                .map(str::trim)
                .ok_or_else(|| format!("invalid override (missing '='): {s}"))?;
            if key.is_empty() {
                return Err(format!("empty key in override: {s}"));
            }

            let value = parse_toml_value(value_str).unwrap_or_else(|_| {
                let trimmed = value_str.trim().trim_matches(|c| c == '\"' || c == '\'');
                toml::Value::String(trimmed.to_string())
            });
            Ok((key.to_string(), value))
        })
        .collect()
}

fn parse_toml_value(raw: &str) -> Result<toml::Value, toml::de::Error> {
    let wrapped = format!("_x_ = {raw}");
    let table: toml::Table = toml::from_str(&wrapped)?;
    Ok(table
        .get("_x_")
        .cloned()
        .unwrap_or_else(|| toml::Value::String(raw.to_string())))
}

fn toml_value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(v) => v.to_string(),
        toml::Value::Float(v) => v.to_string(),
        toml::Value::Boolean(v) => v.to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{LinkerdogRuntimeConfig, normalize_cli_args, parse_cli_overrides};
    use std::ffi::OsString;

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn normalize_cli_args_keeps_direct_invocation() {
        let args = os_args(&["linkerdog", "-c", "model=o3"]);
        assert_eq!(normalize_cli_args(args.clone()), args);
    }

    #[test]
    fn normalize_cli_args_strips_acp_subcommand_once() {
        let args = os_args(&["linkerdog", "acp", "-c", "model=o3"]);
        let expected = os_args(&["linkerdog", "-c", "model=o3"]);
        assert_eq!(normalize_cli_args(args), expected);
    }

    #[test]
    fn normalize_cli_args_keeps_non_first_acp_argument() {
        let args = os_args(&["linkerdog", "-c", "mode=acp"]);
        assert_eq!(normalize_cli_args(args.clone()), args);
    }

    #[test]
    fn parse_cli_overrides_supports_toml_and_raw_string() {
        let raw = vec!["model=\"gpt-5\"".to_string(), "mode=code".to_string()];
        let parsed = parse_cli_overrides(&raw).expect("parse overrides");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "model");
        assert_eq!(parsed[0].1.as_str(), Some("gpt-5"));
        assert_eq!(parsed[1].0, "mode");
        assert_eq!(parsed[1].1.as_str(), Some("code"));
    }

    #[test]
    fn runtime_config_applies_overrides() {
        let raw = vec![
            "provider=anthropic".to_string(),
            "linkerdog.model=claude-sonnet-4".to_string(),
            "agent.mode=review".to_string(),
        ];
        let config = LinkerdogRuntimeConfig::from_raw_overrides(&raw).expect("runtime config");
        assert_eq!(
            config,
            LinkerdogRuntimeConfig {
                default_provider: "anthropic".to_string(),
                default_model: "claude-sonnet-4".to_string(),
                default_mode: "review".to_string(),
            }
        );
    }
}
