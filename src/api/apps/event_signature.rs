use axum::http::HeaderMap;
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use agenthub_agent_domain::app_events::APP_EVENT_MAX_BYTES;

pub(super) struct AppEventSignature {
    pub key_version: i64,
    timestamp: i64,
    tag: [u8; 32],
}

impl AppEventSignature {
    pub fn parse(headers: &HeaderMap, now: i64) -> anyhow::Result<Self> {
        let key_version = integer_header(headers, "x-agenthub-app-key-version")?;
        let timestamp = integer_header(headers, "x-agenthub-app-timestamp")?;
        anyhow::ensure!(
            now >= 0 && timestamp.abs_diff(now) <= 300,
            "invalid app event signature"
        );
        let tag = decode_fixed(single_header(headers, "x-agenthub-app-signature")?)?;
        Ok(Self {
            key_version,
            timestamp,
            tag,
        })
    }

    pub fn verify(&self, app_id: &str, body: &[u8], key: &[u8; 32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            body.len() <= APP_EVENT_MAX_BYTES,
            "invalid app event signature"
        );
        mac(app_id, self.key_version, self.timestamp, body, key)
            .verify_slice(&self.tag)
            .map_err(|_| anyhow::anyhow!("invalid app event signature"))
    }
}

pub(super) fn decode_fixed(value: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(value.len() == 44, "invalid app event signature");
    STANDARD
        .decode(value)
        .ok()
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("invalid app event signature"))
}

fn single_header<'a>(headers: &'a HeaderMap, name: &'static str) -> anyhow::Result<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next().and_then(|value| value.to_str().ok());
    anyhow::ensure!(values.next().is_none(), "invalid app event signature");
    value.ok_or_else(|| anyhow::anyhow!("invalid app event signature"))
}

fn integer_header(headers: &HeaderMap, name: &'static str) -> anyhow::Result<i64> {
    let value = single_header(headers, name)?;
    anyhow::ensure!(value.len() <= 19, "invalid app event signature");
    let parsed = value
        .parse::<i64>()
        .ok()
        .filter(|parsed| *parsed > 0 && parsed.to_string() == value);
    parsed.ok_or_else(|| anyhow::anyhow!("invalid app event signature"))
}

fn mac(app_id: &str, version: i64, timestamp: i64, body: &[u8], key: &[u8; 32]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("fixed HMAC key length");
    mac.update(b"agenthub.app-event.v1\n");
    for value in [
        app_id.to_owned(),
        version.to_string(),
        timestamp.to_string(),
    ] {
        mac.update(value.as_bytes());
        mac.update(b"\n");
    }
    mac.update(body);
    mac
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(
        app_id: &str,
        version: i64,
        timestamp: i64,
        body: &[u8],
        key: &[u8; 32],
    ) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-agenthub-app-key-version",
            version.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-agenthub-app-timestamp",
            timestamp.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-agenthub-app-signature",
            STANDARD
                .encode(
                    mac(app_id, version, timestamp, body, key)
                        .finalize()
                        .into_bytes(),
                )
                .parse()
                .unwrap(),
        );
        headers
    }

    #[test]
    fn event_signatures_bind_app_key_version_timestamp_and_exact_body() {
        let key = [11; 32];
        let body = br#"{"event_id":"evt-1","cursor":1}"#;
        let headers = headers("app-a", 1, 1000, body, &key);
        let actual: String = mac("app-a", 1, 1000, body, &key)
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        // Independent Python hmac/sha256 vector fixes the framing contract.
        assert_eq!(
            actual,
            "2829dde9cb358e9089a1cc6fd235e5bede4be90355655dbd3e2fc1030d58a081"
        );
        for now in [700, 1000, 1300] {
            AppEventSignature::parse(&headers, now)
                .unwrap()
                .verify("app-a", body, &key)
                .unwrap();
        }
        let signature = AppEventSignature::parse(&headers, 1000).unwrap();
        assert!(signature.verify("app-b", body, &key).is_err());
        assert!(signature.verify("app-a", b"{}", &key).is_err());
        assert!(signature.verify("app-a", body, &[12; 32]).is_err());
        for (name, value) in [
            ("x-agenthub-app-key-version", "2"),
            ("x-agenthub-app-timestamp", "1001"),
        ] {
            let mut altered = headers.clone();
            altered.insert(name, value.parse().unwrap());
            assert!(
                AppEventSignature::parse(&altered, 1000)
                    .unwrap()
                    .verify("app-a", body, &key)
                    .is_err()
            );
        }
        for now in [-1, 699, 1301, i64::MAX] {
            assert!(AppEventSignature::parse(&headers, now).is_err());
        }
        assert!(
            signature
                .verify("app-a", &vec![b'a'; APP_EVENT_MAX_BYTES + 1], &key)
                .is_err()
        );
    }

    #[test]
    fn event_signature_headers_and_key_encodings_are_unambiguous_and_bounded() {
        let key = [11; 32];
        let original = headers("app-a", 1, 1000, b"{}", &key);
        for name in [
            "x-agenthub-app-key-version",
            "x-agenthub-app-timestamp",
            "x-agenthub-app-signature",
        ] {
            let mut missing = original.clone();
            missing.remove(name);
            assert!(AppEventSignature::parse(&missing, 1000).is_err());
            let mut duplicate = original.clone();
            duplicate.append(name, original[name].clone());
            assert!(AppEventSignature::parse(&duplicate, 1000).is_err());
        }
        for value in ["+1", "01", " 1", "-1", "0", "9223372036854775808"] {
            let mut headers = original.clone();
            headers.insert("x-agenthub-app-key-version", value.parse().unwrap());
            assert!(AppEventSignature::parse(&headers, 1000).is_err());
        }
        for key in [
            "",
            "secret",
            &STANDARD.encode([1; 16]),
            &STANDARD.encode([1; 64]),
        ] {
            assert!(decode_fixed(key).is_err());
        }
        assert_eq!(decode_fixed(&STANDARD.encode(key)).unwrap(), key);
    }
}
