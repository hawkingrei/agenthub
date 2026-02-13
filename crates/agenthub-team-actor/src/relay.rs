use async_trait::async_trait;
use thiserror::Error;

use crate::message::ActorMessageRecord;

#[derive(Debug, Error)]
pub enum ActorRelayError<E: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    Retryable(E),
    #[error(transparent)]
    Permanent(E),
}

impl<E> ActorRelayError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    pub fn retryable(err: E) -> Self {
        Self::Retryable(err)
    }

    pub fn permanent(err: E) -> Self {
        Self::Permanent(err)
    }
}

#[async_trait]
pub trait ActorMessageRelay {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn deliver(
        &self,
        message: &ActorMessageRecord,
    ) -> Result<(), ActorRelayError<Self::Error>>;
}
