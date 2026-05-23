use std::time::Duration;

const IDLE_GC_TIMEOUT_SECONDS: u64 = 5 * 60;

pub(super) fn build_agent_event_idle_gc(
    config: &agenthub_config::AppConfig,
    event_dbs: &agenthub_db::AgentEventDbRouter,
) -> Option<agenthub_db::AgentEventIdleGc> {
    config.history_event_retention_days().map(|retention_days| {
        let vacuum_on_cleanup = config.history_vacuum_on_cleanup();
        let delete_batch_size = config.history_delete_batch_size();
        tracing::info!(
            "history gc configured with idle trigger: retention_days={} idle_timeout_seconds={} batch_size={} vacuum_on_cleanup={}",
            retention_days,
            IDLE_GC_TIMEOUT_SECONDS,
            delete_batch_size,
            vacuum_on_cleanup,
        );
        agenthub_db::AgentEventIdleGc::new(
            event_dbs.clone(),
            retention_days,
            vacuum_on_cleanup,
            delete_batch_size,
            Duration::from_secs(IDLE_GC_TIMEOUT_SECONDS),
        )
    })
}
