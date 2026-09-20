use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::internal::client::{InternalGrpcMailboxClient, InternalGrpcMailboxClientConfig};

pub(crate) const LOOP_ACTIVATION_ENV: &str = "AGENTHUB_LOOP_ACTIVATION_ID";
pub(crate) const LOOP_CREDENTIAL_FILE_ENV: &str = "AGENTHUB_LOOP_CREDENTIAL_FILE";

// Do not derive Debug: this envelope is private launch material, never lifecycle trace data.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoopCredentialEnvelope {
    pub actor_id: String,
    pub run_id: String,
    pub activation_id: String,
    pub generation: i64,
    pub target: String,
    pub access_token: String,
    pub expires_at: i64,
    pub ca_cert_path: Option<String>,
}

impl LoopCredentialEnvelope {
    pub(crate) fn read(path: &Path) -> anyhow::Result<Self> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(16_385)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= 16_384,
            "loop credential envelope exceeds its size limit"
        );
        let envelope: Self = serde_json::from_slice(&bytes)?;
        anyhow::ensure!(
            envelope.expires_at > chrono::Utc::now().timestamp(),
            "loop credentials expired; the executor must stop"
        );
        Ok(envelope)
    }

    pub(crate) async fn connect(self) -> anyhow::Result<InternalGrpcMailboxClient> {
        let tls_server_name = self
            .target
            .starts_with("https://")
            .then(|| "localhost".to_string());
        InternalGrpcMailboxClient::connect(InternalGrpcMailboxClientConfig {
            target: self.target,
            access_token: self.access_token,
            ca_cert_path: self.ca_cert_path,
            tls_server_name,
            client_cert_path: None,
            client_key_path: None,
        })
        .await
    }
}

pub(crate) struct LoopCredentialFile {
    directory: PathBuf,
    pub path: PathBuf,
}

impl LoopCredentialFile {
    pub(crate) fn create() -> anyhow::Result<Self> {
        let directory =
            std::env::temp_dir().join(format!("agenthub-loop-runtime-{}", uuid::Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        Ok(Self {
            path: directory.join("control.json"),
            directory,
        })
    }

    pub(crate) fn replace(&self, envelope: &LoopCredentialEnvelope) -> anyhow::Result<()> {
        use std::io::Write;
        let temporary = self
            .directory
            .join(format!("control-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&serde_json::to_vec(envelope)?)?;
            std::fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }
}

impl Drop for LoopCredentialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(token: &str, expires_at: i64) -> LoopCredentialEnvelope {
        LoopCredentialEnvelope {
            actor_id: "worker".into(),
            run_id: "mailbox".into(),
            activation_id: "activation".into(),
            generation: 1,
            target: "http://127.0.0.1:1".into(),
            access_token: token.into(),
            expires_at,
            ca_cert_path: None,
        }
    }

    #[test]
    fn loop_credentials_rotate_at_a_stable_private_path_and_are_removed_on_cleanup() {
        let file = LoopCredentialFile::create().unwrap();
        let path = file.path.clone();
        let now = chrono::Utc::now().timestamp();
        file.replace(&envelope("first", now + 60)).unwrap();
        assert_eq!(
            LoopCredentialEnvelope::read(&path).unwrap().access_token,
            "first"
        );
        file.replace(&envelope("refreshed", now + 120)).unwrap();
        assert_eq!(
            LoopCredentialEnvelope::read(&path).unwrap().access_token,
            "refreshed"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        drop(file);
        assert!(!path.parent().unwrap().exists());
    }

    #[test]
    fn loop_credentials_reject_expired_oversized_and_invalid_envelopes() {
        let file = LoopCredentialFile::create().unwrap();
        file.replace(&envelope("expired", 1)).unwrap();
        assert!(LoopCredentialEnvelope::read(&file.path).is_err());
        std::fs::write(&file.path, "x".repeat(16_385)).unwrap();
        assert!(LoopCredentialEnvelope::read(&file.path).is_err());
        std::fs::write(&file.path, "{}").unwrap();
        assert!(LoopCredentialEnvelope::read(&file.path).is_err());
    }
}
