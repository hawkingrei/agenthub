use super::context_artifact_persistence::ContextArtifactOwner;
use super::memory_flush_helpers::{
    MemoryFlushFinalizeContext, build_context_artifact_pointer_payload,
    build_memory_flush_observation, build_memory_flush_summary, finalize_memory_flush_failed_tx,
    finalize_memory_flush_noop_tx, load_memory_flush_checkpoint_event_id_tx,
    load_memory_flush_event_rows, load_memory_flush_team_id_tx, normalize_memory_flush_request,
    resolve_memory_flush_session_id_tx, safe_i64_len, upsert_memory_flush_checkpoint_tx,
};
use super::{
    MEMORY_FLUSH_ARTIFACT_KIND, TeamManager, TeamMemoryFlushRequest, TeamMemoryFlushResult,
};
use agenthub_text::truncate_chars;

impl TeamManager {
    pub async fn flush_run_context(
        &self,
        run_id: &str,
        request: TeamMemoryFlushRequest,
    ) -> anyhow::Result<TeamMemoryFlushResult> {
        let normalized = normalize_memory_flush_request(request)?;
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let team_id = load_memory_flush_team_id_tx(&mut tx, run_id).await?;

        let session_id = resolve_memory_flush_session_id_tx(
            &mut tx,
            run_id,
            normalized.member_id.as_str(),
            normalized.session_id.as_deref(),
        )
        .await?;

        let mut archive_events = Vec::new();
        let started_event = Self::append_run_event_tx(
            &mut tx,
            run_id,
            None,
            "memory_flush_started",
            now,
            &serde_json::json!({
                "team_id": team_id.as_str(),
                "run_id": run_id,
                "member_id": normalized.member_id.as_str(),
                "session_id": session_id.as_deref(),
                "trigger": normalized.trigger.as_str(),
                "ts": now,
            }),
        )
        .await?;
        archive_events.push(started_event);

        let Some(session_id) = session_id else {
            let context = MemoryFlushFinalizeContext {
                run_id,
                team_id: team_id.as_str(),
                member_id: normalized.member_id.as_str(),
                session_id: None,
                trigger: normalized.trigger.as_str(),
                now,
            };
            let (result, event) = finalize_memory_flush_failed_tx(
                &mut tx,
                &context,
                "session_mapping_missing",
                None,
                0,
                None,
            )
            .await?;
            tx.commit().await?;
            archive_events.push(event);
            self.spawn_archive_team_run_events(archive_events);
            return Ok(result);
        };

        let checkpoint_event_id = load_memory_flush_checkpoint_event_id_tx(
            &mut tx,
            run_id,
            normalized.member_id.as_str(),
            session_id.as_str(),
        )
        .await?;
        let event_rows = load_memory_flush_event_rows(
            &self.event_dbs,
            normalized.member_id.as_str(),
            session_id.as_str(),
            checkpoint_event_id,
            normalized.max_events,
        )
        .await?;

        if event_rows.is_empty() {
            let context = MemoryFlushFinalizeContext {
                run_id,
                team_id: team_id.as_str(),
                member_id: normalized.member_id.as_str(),
                session_id: Some(session_id.as_str()),
                trigger: normalized.trigger.as_str(),
                now,
            };
            let (result, event) = finalize_memory_flush_noop_tx(&mut tx, &context).await?;
            tx.commit().await?;
            archive_events.push(event);
            self.spawn_archive_team_run_events(archive_events);
            return Ok(result);
        }

        let observations = event_rows
            .iter()
            .map(build_memory_flush_observation)
            .collect::<Vec<_>>();
        let event_id_from = event_rows.first().map(|row| row.id).unwrap_or(0);
        let event_id_to = event_rows.last().map(|row| row.id).unwrap_or(0);
        let flushed_events = safe_i64_len(event_rows.len());
        let summary_text = build_memory_flush_summary(observations.as_slice());
        let flush_payload = serde_json::json!({
            "schema_version": 1,
            "team_id": team_id.as_str(),
            "run_id": run_id,
            "member_id": normalized.member_id.as_str(),
            "session_id": session_id.as_str(),
            "trigger": normalized.trigger.as_str(),
            "source_event_range": {
                "from_exclusive": checkpoint_event_id,
                "to_inclusive": event_id_to,
            },
            "summary_text": summary_text,
            "observations": observations,
            "created_at": now,
        });

        let pointer = match self
            .persist_context_artifact_tx(
                &mut tx,
                ContextArtifactOwner {
                    team_id: team_id.as_str(),
                    run_id,
                    member_id: normalized.member_id.as_str(),
                    session_id: Some(session_id.as_str()),
                },
                MEMORY_FLUSH_ARTIFACT_KIND,
                flush_payload,
                now,
            )
            .await
        {
            Ok(Some(pointer)) => pointer,
            Ok(None) => {
                let context = MemoryFlushFinalizeContext {
                    run_id,
                    team_id: team_id.as_str(),
                    member_id: normalized.member_id.as_str(),
                    session_id: Some(session_id.as_str()),
                    trigger: normalized.trigger.as_str(),
                    now,
                };
                let (result, event) = finalize_memory_flush_failed_tx(
                    &mut tx,
                    &context,
                    "agent_workdir_missing",
                    Some((event_id_from, event_id_to)),
                    flushed_events,
                    None,
                )
                .await?;
                tx.commit().await?;
                archive_events.push(event);
                self.spawn_archive_team_run_events(archive_events);
                return Ok(result);
            }
            Err(err) => {
                let context = MemoryFlushFinalizeContext {
                    run_id,
                    team_id: team_id.as_str(),
                    member_id: normalized.member_id.as_str(),
                    session_id: Some(session_id.as_str()),
                    trigger: normalized.trigger.as_str(),
                    now,
                };
                let (result, event) = finalize_memory_flush_failed_tx(
                    &mut tx,
                    &context,
                    "artifact_write_failed",
                    Some((event_id_from, event_id_to)),
                    flushed_events,
                    Some(truncate_chars(err.to_string().as_str(), 400)),
                )
                .await?;
                tx.commit().await?;
                archive_events.push(event);
                self.spawn_archive_team_run_events(archive_events);
                return Ok(result);
            }
        };

        upsert_memory_flush_checkpoint_tx(
            &mut tx,
            team_id.as_str(),
            run_id,
            normalized.member_id.as_str(),
            session_id.as_str(),
            event_id_to,
            now,
        )
        .await?;

        let pointer_payload = build_context_artifact_pointer_payload(&pointer);
        let persisted_event = Self::append_run_event_tx(
            &mut tx,
            run_id,
            None,
            "memory_flush_persisted",
            now,
            &serde_json::json!({
                "team_id": team_id.as_str(),
                "run_id": run_id,
                "member_id": normalized.member_id.as_str(),
                "session_id": session_id.as_str(),
                "trigger": normalized.trigger.as_str(),
                "artifact_pointer": pointer_payload.clone(),
                "artifact_size_bytes": pointer.artifact_size_bytes,
                "event_id_from": event_id_from,
                "event_id_to": event_id_to,
                "ts": now,
            }),
        )
        .await?;
        archive_events.push(persisted_event);

        tx.commit().await?;
        self.spawn_archive_team_run_events(archive_events);
        Ok(TeamMemoryFlushResult {
            status: "persisted".to_string(),
            run_id: run_id.to_string(),
            team_id,
            member_id: normalized.member_id,
            session_id: Some(session_id),
            trigger: normalized.trigger,
            reason: None,
            artifact_pointer: Some(pointer_payload),
            event_id_from: Some(event_id_from),
            event_id_to: Some(event_id_to),
            flushed_events,
        })
    }
}
