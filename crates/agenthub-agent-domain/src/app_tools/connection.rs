use serde::{Deserialize, Serialize};
use url::{Host, Url};

use super::valid_name;

/// Provisioned only by an instance administrator. Never include this record in provider discovery.
/// Authority and namespace identify the same external effects across versions and secret rotation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConnection {
    pub endpoint: String,
    pub credential_env: Option<String>,
    pub authority: String,
    pub namespace: String,
}

impl AppConnection {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.endpoint.len() <= 2048,
            "app endpoint exceeds its limit"
        );
        let endpoint =
            Url::parse(&self.endpoint).map_err(|_| anyhow::anyhow!("invalid app endpoint"))?;
        let loopback = match endpoint.host() {
            Some(Host::Domain(host)) => host == "localhost",
            Some(Host::Ipv4(address)) => address.is_loopback(),
            Some(Host::Ipv6(address)) => address.is_loopback(),
            None => false,
        };
        anyhow::ensure!(
            (endpoint.scheme() == "https" || (endpoint.scheme() == "http" && loopback))
                && endpoint.host().is_some()
                && endpoint.username().is_empty()
                && endpoint.password().is_none()
                && endpoint.query().is_none()
                && endpoint.fragment().is_none(),
            "app endpoint requires HTTPS or loopback HTTP without embedded credentials or query"
        );
        anyhow::ensure!(
            valid_name(&self.authority) && valid_name(&self.namespace),
            "invalid app authority or namespace"
        );
        if let Some(reference) = &self.credential_env {
            anyhow::ensure!(
                valid_credential_reference(reference),
                "invalid app credential reference"
            );
        }
        Ok(())
    }
}

/// Credential removal must not strip process configuration or the daemon's actor credentials.
pub fn valid_credential_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 128
        && reference.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
        })
        && !reference.starts_with("AGENTHUB_")
        && !matches!(
            reference,
            "HOME" | "PATH" | "USER" | "SHELL" | "TMPDIR" | "TMP" | "TEMP"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_authority_is_explicit_and_credentials_cannot_hide_in_urls() {
        let mut connection = AppConnection {
            endpoint: "https://tools.example.test/mcp".into(),
            credential_env: Some("APP_TOKEN_1".into()),
            authority: "tools.example.test".into(),
            namespace: "workspace-1".into(),
        };
        connection.validate().unwrap();
        for endpoint in [
            "http://localhost/mcp",
            "http://127.0.0.1:8000/mcp",
            "http://[::1]/mcp",
        ] {
            connection.endpoint = endpoint.into();
            connection.validate().unwrap();
        }
        for endpoint in [
            "http://tools.example.test/mcp",
            "file:///tmp/app",
            "https://token@tools.example.test/mcp",
            "https://tools.example.test/mcp?token=secret",
            "https://tools.example.test/mcp#secret",
        ] {
            connection.endpoint = endpoint.into();
            let error = connection.validate().unwrap_err().to_string();
            assert!(!error.contains(endpoint) && !error.contains("secret"));
        }
        connection.endpoint = "https://tools.example.test/mcp".into();
        for reference in [
            "",
            "1TOKEN",
            "APP-TOKEN",
            "APP\nTOKEN",
            "HOME",
            "PATH",
            "AGENTHUB_LOOP_CREDENTIAL_FILE",
        ] {
            connection.credential_env = Some(reference.into());
            assert!(connection.validate().is_err());
        }
        connection.credential_env = None;
        connection.namespace.clear();
        assert!(connection.validate().is_err());
    }
}
