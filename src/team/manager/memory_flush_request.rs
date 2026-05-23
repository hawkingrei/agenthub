use super::{MEMORY_FLUSH_MAX_EVENTS_DEFAULT, MEMORY_FLUSH_MAX_EVENTS_MAX, TeamMemoryFlushRequest};

#[derive(Debug, Clone)]
pub(super) struct NormalizedMemoryFlushRequest {
    pub(super) member_id: String,
    pub(super) session_id: Option<String>,
    pub(super) trigger: String,
    pub(super) max_events: i64,
}

pub(super) fn normalize_memory_flush_request(
    request: TeamMemoryFlushRequest,
) -> anyhow::Result<NormalizedMemoryFlushRequest> {
    let member_id = request.member_id.trim().to_string();
    if member_id.is_empty() {
        return Err(anyhow::anyhow!("member_id is required"));
    }
    Ok(NormalizedMemoryFlushRequest {
        member_id,
        session_id: request.session_id,
        trigger: normalize_memory_flush_trigger(request.trigger.as_str()).to_string(),
        max_events: normalize_memory_flush_max_events(request.max_events),
    })
}

fn normalize_memory_flush_trigger(raw: &str) -> &'static str {
    match raw.trim() {
        "soft_threshold" => "soft_threshold",
        "hard_error" => "hard_error",
        _ => "manual",
    }
}

fn normalize_memory_flush_max_events(raw: Option<i64>) -> i64 {
    raw.unwrap_or(MEMORY_FLUSH_MAX_EVENTS_DEFAULT)
        .clamp(1, MEMORY_FLUSH_MAX_EVENTS_MAX)
}
