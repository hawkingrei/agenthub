//! The SQLite-staged body outbox.
//!
//! The body is primary data. To keep it durable across the window between the authority commit and the
//! body store's durable ack, the write path stages the body in this outbox *inside* the authority
//! transaction. A background drain moves staged bodies into the body store and only clears an outbox
//! row once the store durably acknowledges it. So a body store write failure (or a crash mid-write)
//! never loses a body: it survives in the outbox until a later drain succeeds.
//!
//! This type models that staging/drain contract. The production backing is a SQLite table
//! (`message_body_outbox`); this in-memory model is the executable contract and the test double.

use std::collections::BTreeMap;

use crate::body_store::MessageBodyStore;
use crate::ids::AuthorityMessageId;

/// Bodies staged but not yet confirmed in the body store.
#[derive(Debug, Default)]
pub struct BodyOutbox {
    pending: BTreeMap<AuthorityMessageId, Vec<u8>>,
}

impl BodyOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage a body for `id`. Called inside the authority transaction. Idempotent per logical message.
    pub fn stage(&mut self, id: &AuthorityMessageId, body: &[u8]) {
        self.pending.insert(id.clone(), body.to_vec());
    }

    /// Number of bodies awaiting confirmation in the body store.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Whether `id` is still awaiting confirmation.
    pub fn contains(&self, id: &AuthorityMessageId) -> bool {
        self.pending.contains_key(id)
    }

    /// Drain pending bodies into `store`. Each body is written to the store; an outbox row is removed
    /// only when its write succeeds (durable ack). A failed write leaves the row pending for a later
    /// retry, so no body is lost. Returns the number of bodies confirmed in this drain.
    pub fn drain_into<S: MessageBodyStore + ?Sized>(&mut self, store: &S) -> usize {
        let mut confirmed = Vec::new();
        for (id, body) in &self.pending {
            if store.put_body(id, body).is_ok() {
                confirmed.push(id.clone());
            }
        }
        for id in &confirmed {
            self.pending.remove(id);
        }
        confirmed.len()
    }
}
