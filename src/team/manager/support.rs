use serde_json::Value;

use super::context_artifact_persistence::ContextArtifactPointer;
use super::memory_flush_finalize::build_context_artifact_pointer_payload;
use super::{
    TeamConversationRecord, TeamConversationStreamEvent, TeamManager, TeamRunRecord, TeamTaskRecord,
};

pub(super) fn maybe_attach_context_artifact_pointer(
    payload: &mut Value,
    pointer: Option<&ContextArtifactPointer>,
    offload_reason: Option<&str>,
) {
    let Some(payload_obj) = payload.as_object_mut() else {
        return;
    };
    if let Some(pointer) = pointer {
        payload_obj.insert(
            "artifact_pointer".to_string(),
            build_context_artifact_pointer_payload(pointer),
        );
        payload_obj.insert(
            "artifact_offload_status".to_string(),
            Value::String("persisted".to_string()),
        );
    } else if let Some(reason) = offload_reason {
        payload_obj.insert(
            "artifact_offload_status".to_string(),
            Value::String("skipped".to_string()),
        );
        payload_obj.insert(
            "artifact_offload_reason".to_string(),
            Value::String(reason.to_string()),
        );
    }
}

impl TeamManager {
    pub(super) async fn load_task_detail_for_team(
        &self,
        team_id: &str,
        task_id: &str,
    ) -> anyhow::Result<(
        TeamTaskRecord,
        TeamConversationRecord,
        Option<TeamRunRecord>,
    )> {
        let task = self.get_task(task_id).await?;
        if task.team_id != team_id {
            anyhow::bail!("task not found for team");
        }
        let conversation = self.get_task_conversation(task_id).await?;
        let latest_run = self.get_latest_run_for_task(team_id, task_id).await?;
        Ok((task, conversation, latest_run))
    }

    pub(crate) fn emit_conversation_event(&self, event: TeamConversationStreamEvent) {
        let _ = self.conversation_events.send(event);
    }
}
