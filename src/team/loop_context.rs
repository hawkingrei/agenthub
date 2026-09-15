use std::future::Future;

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct LoopWorkSourceDetail {
    pub source: agenthub_agent_domain::loop_runtime::LoopTriggerRecord,
    pub conversation_message: Option<super::TeamConversationMessageRecord>,
    pub mailbox_message: Option<super::TeamActorMessageRecord>,
}

/// Provenance is installed by authenticated request entrypoints, never by message payloads.
#[derive(Clone, Default)]
pub(crate) struct LoopSchedulingContext {
    pub actor_id: Option<String>,
    pub activation_id: Option<String>,
    pub user_id: Option<String>,
}

tokio::task_local! {
    static SCHEDULING: LoopSchedulingContext;
}

pub(crate) async fn with_scheduling_context<T>(
    context: LoopSchedulingContext,
    operation: impl Future<Output = T>,
) -> T {
    SCHEDULING.scope(context, operation).await
}

pub(crate) fn scheduling_context() -> LoopSchedulingContext {
    SCHEDULING.try_with(Clone::clone).unwrap_or_default()
}
