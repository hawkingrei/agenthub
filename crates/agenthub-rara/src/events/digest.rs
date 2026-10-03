use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{EventFrame, ProtocolError};

impl EventFrame {
    /// Stable across object-key order and serde_json's optional preserve_order feature.
    pub fn fingerprint(&self) -> Result<[u8; 32], ProtocolError> {
        let value = serde_json::to_value(self).map_err(|_| ProtocolError::Serialization)?;
        let mut writer = HashWriter(Sha256::new());
        serde_json::to_writer(&mut writer, &Canonical(&value))
            .map_err(|_| ProtocolError::Serialization)?;
        Ok(writer.0.finalize().into())
    }
}

struct HashWriter(Sha256);

impl std::io::Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Canonical<'a>(&'a Value);

impl Serialize for Canonical<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(object) => {
                let mut keys: Vec<_> = object.keys().collect();
                keys.sort_unstable();
                let mut map = serializer.serialize_map(Some(keys.len()))?;
                for key in keys {
                    map.serialize_entry(key, &Canonical(&object[key]))?;
                }
                map.end()
            }
            Value::Array(values) => serializer.collect_seq(values.iter().map(Canonical)),
            value => value.serialize(serializer),
        }
    }
}
