//! Body-free delivery-index projection.
//!
//! `cf_index` is derived from SQLite authority metadata and stores compact [`MessageRef`] rows for
//! hot ordered delivery scans. Full bodies stay in `cf_body`; this module only handles refs.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::ids::DeliveryMessageId;
use crate::keys;
use crate::reference::MessageRef;

pub type MessageIndexEntry = (Vec<u8>, Vec<u8>);

#[derive(Debug, Error)]
pub enum MessageIndexError {
    #[error("failed to encode message ref: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("message index backend error: {0}")]
    Backend(String),
}

/// One authority-derived delivery projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageIndexProjection {
    pub sort_id: u64,
    pub reference: MessageRef,
    pub channel: Option<ChannelProjection>,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub inbox_actor_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelProjection {
    pub group_id: String,
    pub channel_id: String,
}

impl ChannelProjection {
    pub fn new(group_id: impl Into<String>, channel_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            channel_id: channel_id.into(),
        }
    }
}

impl MessageIndexProjection {
    pub fn new(sort_id: u64, reference: MessageRef) -> Self {
        Self {
            sort_id,
            reference,
            channel: None,
            agent_id: None,
            run_id: None,
            inbox_actor_id: None,
        }
    }

    pub fn for_channel(
        mut self,
        group_id: impl Into<String>,
        channel_id: impl Into<String>,
    ) -> Self {
        self.channel = Some(ChannelProjection::new(group_id, channel_id));
        self
    }

    pub fn for_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    pub fn for_run(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn for_inbox_actor(mut self, actor_id: impl Into<String>) -> Self {
        self.inbox_actor_id = Some(actor_id.into());
        self
    }

    pub fn entries(&self) -> Result<Vec<MessageIndexEntry>, MessageIndexError> {
        let encoded = self.reference.to_bytes()?;
        let mut entries = Vec::new();
        entries.push((keys::by_id_key(&self.reference.message_id), encoded.clone()));
        if let Some(channel) = &self.channel {
            entries.push((
                keys::channel_key(&channel.group_id, &channel.channel_id, self.sort_id),
                encoded.clone(),
            ));
        }
        if let Some(agent_id) = &self.agent_id {
            entries.push((keys::agent_key(agent_id, self.sort_id), encoded.clone()));
        }
        if let Some(run_id) = &self.run_id {
            entries.push((keys::run_key(run_id, self.sort_id), encoded.clone()));
        }
        if let Some(actor_id) = &self.inbox_actor_id {
            entries.push((keys::inbox_key(actor_id, self.sort_id), encoded));
        }
        Ok(entries)
    }
}

/// Body-free delivery-index API.
pub trait MessageIndex {
    fn put_message(&self, projection: &MessageIndexProjection) -> Result<(), MessageIndexError>;

    fn get_by_id(
        &self,
        message_id: &DeliveryMessageId,
    ) -> Result<Option<MessageRef>, MessageIndexError>;

    fn scan_prefix(
        &self,
        prefix: &[u8],
        limit: usize,
    ) -> Result<Vec<MessageRef>, MessageIndexError>;

    fn scan_channel(
        &self,
        group_id: &str,
        channel_id: &str,
        limit: usize,
    ) -> Result<Vec<MessageRef>, MessageIndexError> {
        self.scan_prefix(&keys::channel_prefix(group_id, channel_id), limit)
    }

    fn scan_agent(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<MessageRef>, MessageIndexError> {
        self.scan_prefix(&keys::agent_prefix(agent_id), limit)
    }

    fn scan_run(&self, run_id: &str, limit: usize) -> Result<Vec<MessageRef>, MessageIndexError> {
        self.scan_prefix(&keys::run_prefix(run_id), limit)
    }

    fn scan_inbox(
        &self,
        actor_id: &str,
        limit: usize,
    ) -> Result<Vec<MessageRef>, MessageIndexError> {
        self.scan_prefix(&keys::inbox_prefix(actor_id), limit)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryMessageIndex {
    entries: Arc<Mutex<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

impl InMemoryMessageIndex {
    pub fn new() -> Self {
        Self::default()
    }

    fn decode(bytes: &[u8]) -> Result<MessageRef, MessageIndexError> {
        MessageRef::from_bytes(bytes).map_err(MessageIndexError::Encode)
    }
}

impl MessageIndex for InMemoryMessageIndex {
    fn put_message(&self, projection: &MessageIndexProjection) -> Result<(), MessageIndexError> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|err| MessageIndexError::Backend(err.to_string()))?;
        for (key, value) in projection.entries()? {
            entries.insert(key, value);
        }
        Ok(())
    }

    fn get_by_id(
        &self,
        message_id: &DeliveryMessageId,
    ) -> Result<Option<MessageRef>, MessageIndexError> {
        let entries = self
            .entries
            .lock()
            .map_err(|err| MessageIndexError::Backend(err.to_string()))?;
        entries
            .get(&keys::by_id_key(message_id))
            .map(|bytes| Self::decode(bytes))
            .transpose()
    }

    fn scan_prefix(
        &self,
        prefix: &[u8],
        limit: usize,
    ) -> Result<Vec<MessageRef>, MessageIndexError> {
        let entries = self
            .entries
            .lock()
            .map_err(|err| MessageIndexError::Backend(err.to_string()))?;
        entries
            .range(prefix.to_vec()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .take(limit)
            .map(|(_, bytes)| Self::decode(bytes))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AuthorityMessageId, MessageKind};

    fn sample_ref(seq: u64) -> MessageRef {
        MessageRef {
            message_id: DeliveryMessageId::new(format!("delivery-{seq}")),
            authority_message_id: AuthorityMessageId::new(format!("auth-{seq}")),
            archive_document_id: Some(format!("doc-{seq}")),
            created_at: 1_700_000_000 + seq as i64,
            source_kind: "team_conversation_messages".to_string(),
            message_kind: MessageKind::Text,
            correlation_id: Some(format!("corr-{seq}")),
            group_id: Some("group-a".to_string()),
            run_id: Some("run-a".to_string()),
            conversation_id: Some("channel-a".to_string()),
            agent_id: Some("agent-a".to_string()),
        }
    }

    #[test]
    fn in_memory_index_projects_ordered_body_free_refs() {
        let index = InMemoryMessageIndex::new();
        for seq in [2, 0, 1] {
            let reference = sample_ref(seq);
            let projection = MessageIndexProjection::new(seq, reference)
                .for_channel("group-a", "channel-a")
                .for_agent("agent-a")
                .for_run("run-a")
                .for_inbox_actor("actor-a");
            index.put_message(&projection).unwrap();
        }

        let channel = index.scan_channel("group-a", "channel-a", 10).unwrap();
        let ids: Vec<_> = channel.iter().map(|row| row.message_id.as_str()).collect();
        assert_eq!(ids, ["delivery-0", "delivery-1", "delivery-2"]);
        assert_eq!(
            index
                .get_by_id(&DeliveryMessageId::new("delivery-1"))
                .unwrap()
                .unwrap()
                .authority_message_id
                .as_str(),
            "auth-1"
        );
        assert_eq!(index.scan_agent("agent-a", 2).unwrap().len(), 2);
        assert_eq!(index.scan_run("run-a", 10).unwrap().len(), 3);
        assert_eq!(index.scan_inbox("actor-a", 10).unwrap().len(), 3);
    }
}
