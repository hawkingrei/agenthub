use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration};

#[derive(Debug)]
pub(super) enum PendingChallenge {
    Registration {
        user_id: String,
        state: PasskeyRegistration,
        role: String,
        device_name: Option<String>,
        user_agent: Option<String>,
    },
    Authentication {
        user_id: String,
        state: PasskeyAuthentication,
    },
}

#[derive(Clone, Default)]
pub(super) struct PendingChallengeStore {
    inner: Arc<RwLock<HashMap<String, PendingChallenge>>>,
}

impl PendingChallengeStore {
    pub(super) async fn insert_registration(
        &self,
        challenge_id: String,
        user_id: String,
        state: PasskeyRegistration,
        role: String,
        device_name: Option<String>,
        user_agent: Option<String>,
    ) {
        let mut pending = self.inner.write().await;
        pending.insert(
            challenge_id,
            PendingChallenge::Registration {
                user_id,
                state,
                role,
                device_name,
                user_agent,
            },
        );
    }

    pub(super) async fn insert_authentication(
        &self,
        challenge_id: String,
        user_id: String,
        state: PasskeyAuthentication,
    ) {
        let mut pending = self.inner.write().await;
        pending.insert(
            challenge_id,
            PendingChallenge::Authentication { user_id, state },
        );
    }

    pub(super) async fn take(&self, challenge_id: &str) -> Option<PendingChallenge> {
        let mut pending = self.inner.write().await;
        pending.remove(challenge_id)
    }
}
