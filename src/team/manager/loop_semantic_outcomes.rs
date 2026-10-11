use agenthub_agent_domain::loop_runtime::{
    LoopOutcome, LoopOutcomeKind, LoopReservation, LoopSourceReferences, LoopWaitReason,
};
use agenthub_agent_domain::loop_scheduling::{LoopRegistrationInput, LoopSchedule};
use agenthub_db::loop_runtime::{LoopStore, LoopStoreError};
use agenthub_rara::SemanticGuardDecision;
use serde_json::json;
use sqlx::Row;

use super::TeamManager;
use crate::team::TeamTaskNoteCreateInput;

impl TeamManager {
    /// Called only for a persisted, normally finished guard turn matching the admitted input ACK.
    /// An existing actor finish always wins, including one with an identical outcome kind.
    pub(crate) async fn finish_native_semantic_guard(
        &self,
        reservation: &LoopReservation,
        decision: &SemanticGuardDecision,
        task_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        decision.validate()?;
        let (kind, question) = match decision {
            SemanticGuardDecision::Compatible {} => return Ok(false),
            SemanticGuardDecision::Mismatch { .. } => (LoopOutcomeKind::NoActionableWork, None),
            SemanticGuardDecision::NeedsClarification { question, .. } => {
                (LoopOutcomeKind::Waiting, Some(question))
            }
        };
        let activation = reservation
            .activation_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let session = reservation
            .session_id
            .as_deref()
            .ok_or(LoopStoreError::InvalidState)?;
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await?;
        let row = sqlx::query(
            "SELECT a.outcome_json, f.owner_id AS finish_owner FROM loop_activations a \
             LEFT JOIN loop_finish_receipts f ON f.activation_id = a.id AND f.generation = a.generation \
             WHERE a.id = ? AND a.actor_id = ? AND a.team_id = ? AND a.generation = ? AND a.session_id = ?",
        )
        .bind(activation).bind(&reservation.actor_id).bind(&reservation.team_id)
        .bind(reservation.generation).bind(session).fetch_optional(&mut *tx).await?
        .ok_or(LoopStoreError::StaleLease)?;
        if row.try_get::<Option<String>, _>("outcome_json")?.is_some() {
            anyhow::ensure!(
                row.try_get::<Option<&str>, _>("finish_owner")? == Some(&reservation.owner_id),
                LoopStoreError::StaleLease
            );
            tx.commit().await?;
            return Ok(false);
        }
        LoopStore::finish_in_transaction(
            &mut tx,
            reservation,
            &LoopOutcome {
                kind,
                wait_reason: question.map(|_| LoopWaitReason::Input),
                task_note_id: None,
                continuation: None,
            },
            now,
        )
        .await?;
        let message = if let Some(question) = question {
            let task_id = if let Some(task_id) = task_id {
                // The pinned destination must still be live work in this activation's scope.
                let valid: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM team_tasks t JOIN loop_trigger_sources s \
                     ON json_extract(s.input_json, '$.references.task_id') = t.id \
                     WHERE t.id = ? AND t.team_id = ? AND s.team_id = t.team_id \
                     AND s.actor_id = ? AND s.activation_id = ? \
                     AND NOT EXISTS(SELECT 1 FROM loop_revoked_sources r WHERE r.trigger_id = s.id))",
                ).bind(task_id).bind(&reservation.team_id).bind(&reservation.actor_id).bind(activation)
                    .fetch_one(&mut *tx).await?;
                anyhow::ensure!(valid, LoopStoreError::ScopeMismatch);
                task_id.to_owned()
            } else {
                Self::ensure_shared_thread_target_tx(
                    &mut tx,
                    &reservation.team_id,
                    &reservation.actor_id,
                )
                .await?
                .0
            };
            let key = format!(
                "native-clarification:{activation}:{}",
                reservation.generation
            );
            let (conversation, message, created) = self
                .insert_task_conversation_message_in_tx(
                    &mut tx,
                    &task_id,
                    &TeamTaskNoteCreateInput {
                        from_actor_id: &reservation.actor_id,
                        to_actor_id: None,
                        route: "group_chat",
                        payload: json!({
                            "text":question,
                            "native_clarification":{
                                "activation_id":activation,
                                "generation":reservation.generation,
                                "local_session_id":session,
                            },
                        }),
                        idempotency_key: Some(&key),
                    },
                )
                .await?;
            // A new root gives this wait an exact reply target; prior thread traffic cannot fire it.
            LoopStore::register_schedule_tx(
                &mut tx,
                &LoopRegistrationInput {
                    actor_id: reservation.actor_id.clone(),
                    team_id: reservation.team_id.clone(),
                    source_key: key,
                    schedule: LoopSchedule::ThreadReply {
                        root_message_id: message.message_id,
                        after_message_id: message.message_id,
                        repeat: false,
                    },
                    work_task_id: None,
                    references: LoopSourceReferences {
                        scheduling_actor_id: Some(reservation.actor_id.clone()),
                        scheduling_activation_id: Some(activation.into()),
                        ..Default::default()
                    },
                },
                now,
            )
            .await?;
            Some((conversation, message, created))
        } else {
            None
        };
        tx.commit().await?;
        if let Some((conversation, message, true)) = message {
            self.spawn_archive_task_conversation_message(&conversation, &message);
            self.emit_task_conversation_message_created(&conversation, &message);
        }
        Ok(true)
    }
}
