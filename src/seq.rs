use std::sync::atomic::{AtomicI64, Ordering};

use chrono::Utc;

static LAST_SEQ: AtomicI64 = AtomicI64::new(0);

pub fn next_seq() -> i64 {
    let mut candidate = Utc::now().timestamp_micros();
    loop {
        let last = LAST_SEQ.load(Ordering::Relaxed);
        if candidate <= last {
            candidate = last + 1;
        }
        if LAST_SEQ
            .compare_exchange(last, candidate, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return candidate;
        }
    }
}
