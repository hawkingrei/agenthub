//! Delivery-index key encoding and the body key.
//!
//! Keys are raw byte strings so that a bytewise (lexicographic) range scan over a prefix returns rows
//! in chronological order. The ordering guarantee holds *within one prefix* (for example one channel):
//! the prefix bytes are identical and only the fixed-width [`encode_sort_id`] suffix varies, so
//! bytewise order equals the authority sequence order.
//!
//! `sort_id` is an authority-assigned monotonic sequence, not a wall-clock timestamp: multiple
//! authority tables (and, later, multiple nodes) cannot rely on clock ordering. It is encoded
//! big-endian so the byte order matches the numeric order.
//!
//! Id components (group/channel/agent/run/actor ids) are treated as opaque tokens that do not contain
//! the [`SEP`] byte.

use crate::ids::AuthorityMessageId;

/// Separator between key components.
const SEP: u8 = b'/';

/// Fixed-width, bytewise-order-preserving encoding of an authority sequence.
pub fn encode_sort_id(seq: u64) -> [u8; 8] {
    seq.to_be_bytes()
}

fn join(parts: &[&[u8]], sort_id: Option<[u8; 8]>) -> Vec<u8> {
    let mut key = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            key.push(SEP);
        }
        key.extend_from_slice(part);
    }
    if let Some(sort_id) = sort_id {
        key.push(SEP);
        key.extend_from_slice(&sort_id);
    }
    key
}

/// `msg/by_channel/<group_id>/<channel_id>/<sort_id>`
pub fn channel_key(group_id: &str, channel_id: &str, seq: u64) -> Vec<u8> {
    let mut key = channel_prefix(group_id, channel_id);
    key.push(SEP);
    key.extend_from_slice(&encode_sort_id(seq));
    key
}

/// `msg/by_channel/<group_id>/<channel_id>`
pub fn channel_prefix(group_id: &str, channel_id: &str) -> Vec<u8> {
    join(
        &[
            b"msg/by_channel",
            group_id.as_bytes(),
            channel_id.as_bytes(),
        ],
        None,
    )
}

/// `msg/by_agent/<agent_id>/<sort_id>`
pub fn agent_key(agent_id: &str, seq: u64) -> Vec<u8> {
    let mut key = agent_prefix(agent_id);
    key.push(SEP);
    key.extend_from_slice(&encode_sort_id(seq));
    key
}

/// `msg/by_agent/<agent_id>`
pub fn agent_prefix(agent_id: &str) -> Vec<u8> {
    join(&[b"msg/by_agent", agent_id.as_bytes()], None)
}

/// `msg/by_run/<run_id>/<sort_id>`
pub fn run_key(run_id: &str, seq: u64) -> Vec<u8> {
    let mut key = run_prefix(run_id);
    key.push(SEP);
    key.extend_from_slice(&encode_sort_id(seq));
    key
}

/// `msg/by_run/<run_id>`
pub fn run_prefix(run_id: &str) -> Vec<u8> {
    join(&[b"msg/by_run", run_id.as_bytes()], None)
}

/// `msg/by_id/<message_id>`
pub fn message_id_key(message_id: &str) -> Vec<u8> {
    join(&[b"msg/by_id", message_id.as_bytes()], None)
}

/// `inbox/by_actor/<actor_id>/<sort_id>` — unified chronological inbox across runs.
pub fn inbox_key(actor_id: &str, seq: u64) -> Vec<u8> {
    let mut key = inbox_prefix(actor_id);
    key.push(SEP);
    key.extend_from_slice(&encode_sort_id(seq));
    key
}

/// `inbox/by_actor/<actor_id>`
pub fn inbox_prefix(actor_id: &str) -> Vec<u8> {
    join(&[b"inbox/by_actor", actor_id.as_bytes()], None)
}

/// `body/by_message/<authority_message_id>` — the body key. One body per logical message.
pub fn body_key(id: &AuthorityMessageId) -> Vec<u8> {
    join(&[b"body/by_message", id.as_str().as_bytes()], None)
}

/// `meta/high_water/<stream_id>` — projected authority high-water mark for one stream.
pub fn high_water_key(stream_id: &str) -> Vec<u8> {
    join(&[b"meta/high_water", stream_id.as_bytes()], None)
}
