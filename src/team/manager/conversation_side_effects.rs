use super::{
    MESSAGE_ARCHIVE_APPEND_TIMEOUT, TeamManager, team_conversation_message_archive_document,
};
use crate::team::{
    TeamConversationMessageRecord, TeamConversationRecord, TeamConversationStreamEvent,
};

impl TeamManager {
    pub(super) fn emit_task_conversation_message_created(
        &self,
        conversation: &TeamConversationRecord,
        message: &TeamConversationMessageRecord,
    ) {
        self.emit_conversation_event(TeamConversationStreamEvent {
            team_id: conversation.team_id.clone(),
            task_id: message.task_id.clone(),
            conversation_id: conversation.id.clone(),
            message_id: Some(message.message_id),
            source: "conversation_message".to_string(),
        });
    }

    pub(super) fn spawn_archive_task_conversation_message(
        &self,
        conversation: &TeamConversationRecord,
        message: &TeamConversationMessageRecord,
    ) {
        let Some(archive) = self.message_archive.as_ref() else {
            return;
        };
        let archive = archive.clone();
        let document = team_conversation_message_archive_document(conversation, message);
        let conversation_id = message.conversation_id.clone();
        let message_id = message.message_id;

        tokio::spawn(async move {
            match tokio::time::timeout(
                MESSAGE_ARCHIVE_APPEND_TIMEOUT,
                archive.append_documents(&[document]),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(
                        error = ?error,
                        conversation_id = %conversation_id,
                        message_id,
                        "failed to dual-write team conversation message to archive"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        conversation_id = %conversation_id,
                        message_id,
                        timeout_ms = MESSAGE_ARCHIVE_APPEND_TIMEOUT.as_millis(),
                        "timed out dual-writing team conversation message to archive"
                    );
                }
            }
        });
    }
}
