use std::path::{Path, PathBuf};
use std::sync::Once;

use anyhow::Context;
use base64::URL_SAFE_NO_PAD;
use rand::RngCore;
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalGrpcSecurityMode {
    Disabled,
    Tls,
    Mtls,
}

pub fn install_rustls_crypto_provider() {
    static INSTALL_RUSTLS_PROVIDER: Once = Once::new();
    INSTALL_RUSTLS_PROVIDER.call_once(|| {
        if let Err(err) = rustls::crypto::aws_lc_rs::default_provider().install_default() {
            tracing::warn!(
                error = ?err,
                "failed to install default rustls crypto provider; TLS operations may fail later"
            );
        }
    });
}

impl InternalGrpcSecurityMode {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim() {
            "disabled" => Ok(Self::Disabled),
            "tls" => Ok(Self::Tls),
            "mtls" => Ok(Self::Mtls),
            other => Err(anyhow::anyhow!(
                "unsupported internal_grpc.security.mode '{}', expected one of: disabled, tls, mtls",
                other
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InternalGrpcTlsMaterial {
    pub server_cert_pem: Vec<u8>,
    pub server_key_pem: Vec<u8>,
    pub ca_cert_pem: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct InternalGrpcClientIdentity {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub ca_cert_pem: Vec<u8>,
}

pub fn ensure_shared_secret(cert_dir: &Path, configured: Option<String>) -> anyhow::Result<String> {
    if let Some(secret) = configured {
        return Ok(secret);
    }
    std::fs::create_dir_all(cert_dir)
        .with_context(|| format!("create internal grpc cert dir '{}'", cert_dir.display()))?;
    let secret_path = cert_dir.join("auth_secret.txt");
    if secret_path.exists() {
        let secret = std::fs::read_to_string(&secret_path)
            .with_context(|| format!("read internal auth secret '{}'", secret_path.display()))?;
        let secret = secret.trim().to_string();
        if !secret.is_empty() {
            return Ok(secret);
        }
    }
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let secret = base64::encode_config(bytes, URL_SAFE_NO_PAD);
    std::fs::write(&secret_path, &secret)
        .with_context(|| format!("write internal auth secret '{}'", secret_path.display()))?;
    Ok(secret)
}

pub fn ensure_bootstrap_token(
    cert_dir: &Path,
    configured: Option<String>,
) -> anyhow::Result<String> {
    if let Some(token) = configured {
        return Ok(token);
    }
    std::fs::create_dir_all(cert_dir)
        .with_context(|| format!("create internal grpc cert dir '{}'", cert_dir.display()))?;
    let token_path = cert_dir.join("bootstrap_token.txt");
    if token_path.exists() {
        let token = std::fs::read_to_string(&token_path)
            .with_context(|| format!("read internal bootstrap token '{}'", token_path.display()))?;
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = base64::encode_config(bytes, URL_SAFE_NO_PAD);
    std::fs::write(&token_path, &token)
        .with_context(|| format!("write internal bootstrap token '{}'", token_path.display()))?;
    Ok(token)
}

pub fn ensure_tls_material(
    cert_dir: &Path,
    mode: InternalGrpcSecurityMode,
) -> anyhow::Result<Option<InternalGrpcTlsMaterial>> {
    if mode == InternalGrpcSecurityMode::Disabled {
        return Ok(None);
    }

    std::fs::create_dir_all(cert_dir)
        .with_context(|| format!("create internal grpc cert dir '{}'", cert_dir.display()))?;
    let paths = TlsPaths::new(cert_dir);
    if paths.all_exist() {
        return load_tls_material(&paths).map(Some);
    }
    generate_tls_material(&paths)?;
    load_tls_material(&paths).map(Some)
}

pub fn load_bootstrap_client_identity(
    cert_dir: &Path,
) -> anyhow::Result<InternalGrpcClientIdentity> {
    let paths = TlsPaths::new(cert_dir);
    Ok(InternalGrpcClientIdentity {
        cert_pem: std::fs::read(&paths.bootstrap_client_cert_path).with_context(|| {
            format!(
                "read internal grpc bootstrap client cert '{}'",
                paths.bootstrap_client_cert_path.display()
            )
        })?,
        key_pem: std::fs::read(&paths.bootstrap_client_key_path).with_context(|| {
            format!(
                "read internal grpc bootstrap client key '{}'",
                paths.bootstrap_client_key_path.display()
            )
        })?,
        ca_cert_pem: std::fs::read(&paths.ca_cert_path).with_context(|| {
            format!(
                "read internal grpc ca cert '{}'",
                paths.ca_cert_path.display()
            )
        })?,
    })
}

fn load_tls_material(paths: &TlsPaths) -> anyhow::Result<InternalGrpcTlsMaterial> {
    Ok(InternalGrpcTlsMaterial {
        server_cert_pem: std::fs::read(&paths.server_cert_path).with_context(|| {
            format!(
                "read internal grpc server cert '{}'",
                paths.server_cert_path.display()
            )
        })?,
        server_key_pem: std::fs::read(&paths.server_key_path).with_context(|| {
            format!(
                "read internal grpc server key '{}'",
                paths.server_key_path.display()
            )
        })?,
        ca_cert_pem: std::fs::read(&paths.ca_cert_path).with_context(|| {
            format!(
                "read internal grpc ca cert '{}'",
                paths.ca_cert_path.display()
            )
        })?,
    })
}

fn generate_tls_material(paths: &TlsPaths) -> anyhow::Result<()> {
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "agenthub-internal-ca");
    let ca_key = KeyPair::generate().context("generate internal grpc ca key")?;
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .context("generate internal grpc ca cert")?;
    let ca_issuer = Issuer::from_params(&ca_params, &ca_key);

    let mut server_params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    server_params
        .distinguished_name
        .push(DnType::CommonName, "agenthub-internal-server");
    server_params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ServerAuth);
    let server_key = KeyPair::generate().context("generate internal grpc server key")?;
    let server_cert = server_params
        .signed_by(&server_key, &ca_issuer)
        .context("sign internal grpc server cert")?;

    let mut client_params = CertificateParams::new(vec!["agenthub-internal-client".to_string()])?;
    client_params
        .distinguished_name
        .push(DnType::CommonName, "agenthub-internal-bootstrap-client");
    client_params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ClientAuth);
    let client_key = KeyPair::generate().context("generate internal grpc client key")?;
    let client_cert = client_params
        .signed_by(&client_key, &ca_issuer)
        .context("sign internal grpc client cert")?;

    std::fs::write(&paths.ca_cert_path, ca_cert.pem()).with_context(|| {
        format!(
            "write internal grpc ca cert '{}'",
            paths.ca_cert_path.display()
        )
    })?;
    std::fs::write(&paths.ca_key_path, ca_key.serialize_pem()).with_context(|| {
        format!(
            "write internal grpc ca key '{}'",
            paths.ca_key_path.display()
        )
    })?;
    std::fs::write(&paths.server_cert_path, server_cert.pem()).with_context(|| {
        format!(
            "write internal grpc server cert '{}'",
            paths.server_cert_path.display()
        )
    })?;
    std::fs::write(&paths.server_key_path, server_key.serialize_pem()).with_context(|| {
        format!(
            "write internal grpc server key '{}'",
            paths.server_key_path.display()
        )
    })?;
    std::fs::write(&paths.bootstrap_client_cert_path, client_cert.pem()).with_context(|| {
        format!(
            "write internal grpc bootstrap client cert '{}'",
            paths.bootstrap_client_cert_path.display()
        )
    })?;
    std::fs::write(&paths.bootstrap_client_key_path, client_key.serialize_pem()).with_context(
        || {
            format!(
                "write internal grpc bootstrap client key '{}'",
                paths.bootstrap_client_key_path.display()
            )
        },
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
struct TlsPaths {
    ca_cert_path: PathBuf,
    ca_key_path: PathBuf,
    server_cert_path: PathBuf,
    server_key_path: PathBuf,
    bootstrap_client_cert_path: PathBuf,
    bootstrap_client_key_path: PathBuf,
}

impl TlsPaths {
    fn new(cert_dir: &Path) -> Self {
        Self {
            ca_cert_path: cert_dir.join("ca-cert.pem"),
            ca_key_path: cert_dir.join("ca-key.pem"),
            server_cert_path: cert_dir.join("server-cert.pem"),
            server_key_path: cert_dir.join("server-key.pem"),
            bootstrap_client_cert_path: cert_dir.join("client-cert.pem"),
            bootstrap_client_key_path: cert_dir.join("client-key.pem"),
        }
    }

    fn all_exist(&self) -> bool {
        self.ca_cert_path.exists()
            && self.ca_key_path.exists()
            && self.server_cert_path.exists()
            && self.server_key_path.exists()
            && self.bootstrap_client_cert_path.exists()
            && self.bootstrap_client_key_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        InternalGrpcSecurityMode, ensure_bootstrap_token, ensure_shared_secret, ensure_tls_material,
    };

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agenthub-internal-grpc-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn parse_security_mode() {
        assert_eq!(
            InternalGrpcSecurityMode::parse("disabled").expect("parse disabled"),
            InternalGrpcSecurityMode::Disabled
        );
        assert_eq!(
            InternalGrpcSecurityMode::parse("tls").expect("parse tls"),
            InternalGrpcSecurityMode::Tls
        );
        assert_eq!(
            InternalGrpcSecurityMode::parse("mtls").expect("parse mtls"),
            InternalGrpcSecurityMode::Mtls
        );
    }

    #[test]
    fn ensure_secret_persists() {
        let dir = test_dir("secret");
        let first = ensure_shared_secret(&dir, None).expect("generate secret");
        let second = ensure_shared_secret(&dir, None).expect("reuse secret");
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_bootstrap_token_persists() {
        let dir = test_dir("bootstrap");
        let first = ensure_bootstrap_token(&dir, None).expect("generate bootstrap token");
        let second = ensure_bootstrap_token(&dir, None).expect("reuse bootstrap token");
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_tls_material_generates_files() {
        let dir = test_dir("tls");
        let material = ensure_tls_material(&dir, InternalGrpcSecurityMode::Tls)
            .expect("generate tls material")
            .expect("tls enabled");
        assert!(!material.server_cert_pem.is_empty());
        assert!(!material.server_key_pem.is_empty());
        assert!(!material.ca_cert_pem.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
