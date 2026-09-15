//! Application payload accounting. Leases follow retained/queued data, not request futures.

use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::{MAX_MESSAGE_BYTES, McpTransportError};

/// Credits for bounded transient copies during policy, HTTP/SSE decoding, and result hashing.
/// This is a wire-payload working allowance, not an allocator/RSS measurement.
pub const EXCHANGE_WORKSPACE_BYTES: usize = 8 * MAX_MESSAGE_BYTES;

#[derive(Clone)]
pub struct ByteBudget(Arc<Counter>);

struct Counter {
    limit: usize,
    used: AtomicUsize,
}

pub struct ByteLease {
    budget: ByteBudget,
    bytes: usize,
}

pub struct Budgeted<T> {
    pub value: T,
    lease: ByteLease,
}

impl<T> Budgeted<T> {
    pub fn into_parts(self) -> (T, ByteLease) {
        (self.value, self.lease)
    }
}

impl ByteBudget {
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Counter {
            limit,
            used: AtomicUsize::new(0),
        }))
    }

    pub fn used(&self) -> usize {
        self.0.used.load(Ordering::Acquire)
    }

    fn add(&self, bytes: usize) -> Result<(), McpTransportError> {
        self.0
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes)
                    .filter(|total| *total <= self.0.limit)
            })
            .map(|_| ())
            .map_err(|_| McpTransportError::Capacity)
    }

    pub fn acquire(&self, bytes: usize) -> Result<ByteLease, McpTransportError> {
        self.add(bytes)?;
        Ok(ByteLease {
            budget: self.clone(),
            bytes,
        })
    }

    pub fn retain<T>(&self, value: T, bytes: usize) -> Result<Budgeted<T>, McpTransportError> {
        Ok(Budgeted {
            value,
            lease: self.acquire(bytes)?,
        })
    }
}

impl ByteLease {
    pub fn retain<T>(self, value: T) -> Budgeted<T> {
        Budgeted { value, lease: self }
    }

    pub fn resize(&mut self, bytes: usize) -> Result<(), McpTransportError> {
        if bytes > self.bytes {
            self.budget.add(bytes - self.bytes)?;
        } else {
            self.budget
                .0
                .used
                .fetch_sub(self.bytes - bytes, Ordering::AcqRel);
        }
        self.bytes = bytes;
        Ok(())
    }
}

impl Drop for ByteLease {
    fn drop(&mut self) {
        self.budget.0.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

pub struct McpProxyBudget {
    pub ingress: ByteBudget,
    pub delivery: ByteBudget,
    pub retained: ByteBudget,
    workspaces: ByteBudget,
    callbacks: ByteBudget,
}

impl Default for McpProxyBudget {
    fn default() -> Self {
        Self::new(8, 1, 8 * MAX_MESSAGE_BYTES, 8 * MAX_MESSAGE_BYTES)
    }
}

impl McpProxyBudget {
    /// One budget is shared by every session in a daemon hub. Callback capacity is independent
    /// so outstanding initialize/tool requests cannot consume the space needed for their replies.
    pub fn new(workspaces: usize, callbacks: usize, delivery: usize, retained: usize) -> Self {
        Self {
            ingress: ByteBudget::new(delivery),
            delivery: ByteBudget::new(delivery),
            retained: ByteBudget::new(retained),
            workspaces: ByteBudget::new(workspaces.saturating_mul(EXCHANGE_WORKSPACE_BYTES)),
            callbacks: ByteBudget::new(callbacks.saturating_mul(EXCHANGE_WORKSPACE_BYTES)),
        }
    }

    pub(crate) fn workspace(&self, callback: bool) -> Result<ByteLease, McpTransportError> {
        if callback {
            &self.callbacks
        } else {
            &self.workspaces
        }
        .acquire(EXCHANGE_WORKSPACE_BYTES)
    }
}

/// Measure without allocating a second serialized body. Stop as soon as the frame limit is hit.
pub fn json_bytes(value: &Value) -> Result<usize, McpTransportError> {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            if self.0 > MAX_MESSAGE_BYTES {
                return Err(std::io::ErrorKind::OutOfMemory.into());
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(|_| McpTransportError::MessageTooLarge)?;
    Ok(counter.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_capacity_passes_and_excess_does_not_change_accounting() {
        let budget = ByteBudget::new(8);
        let mut first = budget.acquire(3).unwrap();
        let second = budget.acquire(5).unwrap();
        assert_eq!(budget.used(), 8);
        assert!(budget.acquire(1).is_err());
        assert!(first.resize(4).is_err());
        assert_eq!(budget.used(), 8);
        drop(second);
        first.resize(8).unwrap();
        first.resize(2).unwrap();
        assert_eq!(budget.used(), 2);
        drop(first);
        assert_eq!(budget.used(), 0);
        assert!(budget.acquire(usize::MAX).is_err());
    }

    #[test]
    fn callback_workspace_survives_full_ordinary_admission() {
        let budget = McpProxyBudget::new(1, 1, 16, 16);
        let ordinary = budget.workspace(false).unwrap();
        assert!(budget.workspace(false).is_err());
        let callback = budget.workspace(true).unwrap();
        assert!(budget.workspace(true).is_err());
        drop((ordinary, callback));
        assert!(budget.workspace(false).is_ok());
    }

    #[test]
    fn json_accounting_includes_encoding_and_rejects_oversized_frames() {
        let value = serde_json::json!({"value":"\n\"é"});
        assert_eq!(json_bytes(&value).unwrap(), value.to_string().len());
        assert_eq!(
            json_bytes(&Value::String("x".repeat(MAX_MESSAGE_BYTES - 2))).unwrap(),
            MAX_MESSAGE_BYTES
        );
        assert!(json_bytes(&Value::String("x".repeat(MAX_MESSAGE_BYTES - 1))).is_err());
    }
}
