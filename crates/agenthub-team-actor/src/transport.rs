use crate::ActorMessageTransport;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseActorTransportError {
    #[error("transport must be either 'local' or 'remote'")]
    InvalidValue,
}

pub fn parse_actor_transport(
    raw: Option<&str>,
) -> Result<ActorMessageTransport, ParseActorTransportError> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(ActorMessageTransport::Local),
        Some("local") => Ok(ActorMessageTransport::Local),
        Some("remote") => Ok(ActorMessageTransport::Remote),
        Some(_) => Err(ParseActorTransportError::InvalidValue),
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseActorTransportError, parse_actor_transport};
    use crate::ActorMessageTransport;

    #[test]
    fn parse_actor_transport_supports_local_remote_and_default() {
        assert_eq!(
            parse_actor_transport(None).expect("default transport"),
            ActorMessageTransport::Local
        );
        assert_eq!(
            parse_actor_transport(Some("local")).expect("local"),
            ActorMessageTransport::Local
        );
        assert_eq!(
            parse_actor_transport(Some("remote")).expect("remote"),
            ActorMessageTransport::Remote
        );
        assert_eq!(
            parse_actor_transport(Some(" remote ")).expect("trimmed remote"),
            ActorMessageTransport::Remote
        );
    }

    #[test]
    fn parse_actor_transport_rejects_unknown_value() {
        let err = parse_actor_transport(Some("cluster")).expect_err("unknown transport");
        assert!(matches!(err, ParseActorTransportError::InvalidValue));
    }
}
