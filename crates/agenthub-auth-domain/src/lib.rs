use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand_core::OsRng;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};

#[derive(Debug)]
pub enum RegisterStartResult {
    Challenge {
        challenge_id: String,
        options: Box<CreationChallengeResponse>,
    },
    Complete {
        user_id: String,
        role: String,
    },
}

#[derive(Debug)]
pub enum LoginStartResult {
    Challenge {
        challenge_id: String,
        options: Box<RequestChallengeResponse>,
    },
    Registration {
        challenge_id: String,
        options: Box<CreationChallengeResponse>,
        role: String,
    },
    Complete {
        user_id: String,
        role: String,
    },
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: Option<String>,
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(password.as_bytes(), &parsed).is_ok())
}
