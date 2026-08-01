use agenthub_team_actor::{
    ACTOR_MAIN_PEER_ID, ActorInboxRequest, ActorInboxResponse, ActorServiceError,
    ListActorInboxQuery,
};

use super::mailbox::{SqlActorMailboxStore, map_actor_service_error, required_trimmed_field};
use super::mailbox_service::TeamActorMailboxService;
use crate::team::TeamActorMessageStatus;

impl TeamActorMailboxService {
    pub(super) async fn actor_inbox_impl(
        &self,
        request: ActorInboxRequest,
    ) -> Result<ActorInboxResponse, ActorServiceError> {
        let run_id = required_trimmed_field(&request.run_id, "run_id")?;
        let actor_id = required_trimmed_field(&request.actor_id, "actor_id")?;
        let limit = request.limit.unwrap_or(50).clamp(1, 1000);
        let include_delivered = request
            .states
            .as_ref()
            .is_some_and(|states| states.contains(&TeamActorMessageStatus::Delivered));
        let states = request
            .states
            .unwrap_or_else(|| vec![TeamActorMessageStatus::Pending]);
        let snapshot = SqlActorMailboxStore {
            db: self.manager.db.clone(),
            message_archive: None,
            message_index: self.manager.message_index.clone(),
            read_repair: self.manager.read_repair.clone(),
        }
        .read_inbox_snapshot(&ListActorInboxQuery {
            run_id: run_id.to_string(),
            actor_id: actor_id.to_string(),
            peer_id: ACTOR_MAIN_PEER_ID.to_string(),
            limit,
            after_id: request.cursor,
            include_delivered,
        })
        .await
        .map_err(|err| map_actor_service_error(anyhow::Error::new(err)))?;
        let messages = snapshot
            .messages
            .into_iter()
            .filter(|message| states.contains(&message.status))
            .collect::<Vec<_>>();
        let next_cursor = messages.last().map(|message| message.message_id);

        Ok(ActorInboxResponse {
            messages,
            next_cursor,
            pending_count: snapshot.pending_count,
        })
    }
}
