use super::{
    TeamActorMessageRecord, TeamManager, TeamRunEventRecord, mailbox, parse_run_event_row,
    parse_team_actor_message_row,
};
use agenthub_message_store::{IndexFreshness, keys};
use sqlx::{QueryBuilder, Sqlite};

impl TeamManager {
    pub async fn list_run_events(
        &self,
        run_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamRunEventRecord>> {
        match self
            .try_list_run_events_from_index(run_id, limit, before_id)
            .await
        {
            Ok(Some(events)) => return Ok(events),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    ?error,
                    run_id,
                    "falling back to SQLite after run event index read failed"
                );
            }
        }
        self.list_run_events_from_sqlite(run_id, limit, before_id)
            .await
    }

    async fn list_run_events_from_sqlite(
        &self,
        run_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<TeamRunEventRecord>> {
        let rows = if let Some(before_id) = before_id {
            sqlx::query(
                r#"
                SELECT id, run_id, step_id, event_type, ts, payload_json
                FROM team_run_events
                WHERE run_id = ?1 AND id < ?2
                ORDER BY id DESC
                LIMIT ?3
                "#,
            )
            .bind(run_id)
            .bind(before_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, run_id, step_id, event_type, ts, payload_json
                FROM team_run_events
                WHERE run_id = ?1
                ORDER BY id DESC
                LIMIT ?2
                "#,
            )
            .bind(run_id)
            .bind(limit)
            .fetch_all(&self.db)
            .await?
        };

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            events.push(parse_run_event_row(&row)?);
        }
        events.reverse();
        Ok(events)
    }

    async fn try_list_run_events_from_index(
        &self,
        run_id: &str,
        limit: i64,
        before_id: Option<i64>,
    ) -> anyhow::Result<Option<Vec<TeamRunEventRecord>>> {
        let Some(index) = self.message_index.as_deref() else {
            return Ok(None);
        };

        let authority_max = self.max_run_event_id_for_page(run_id, before_id).await?;
        if authority_max == 0 {
            return Ok(Some(Vec::new()));
        }
        let freshness = agenthub_message_store::check_index_freshness(
            index,
            "team_run_events",
            authority_max as u64,
        )?;
        if !matches!(freshness, IndexFreshness::Fresh { .. }) {
            self.schedule_index_read_repair("team_run_events", authority_max as u64, freshness);
            return Ok(None);
        }

        let refs = index.scan_prefix(&keys::run_prefix(run_id))?;
        let mut ids = Vec::new();
        for message_ref in refs {
            if message_ref.source_kind != "team_run_events" {
                continue;
            }
            let Some(event_id) =
                run_event_id_from_delivery_id(run_id, message_ref.message_id.as_str())
            else {
                return Ok(None);
            };
            if before_id.is_some_and(|before_id| event_id >= before_id) {
                continue;
            }
            ids.push(event_id);
        }
        let limit = limit.max(1) as usize;
        if ids.len() > limit {
            ids = ids.split_off(ids.len() - limit);
        }
        if ids
            != self
                .expected_run_event_ids(run_id, limit, before_id)
                .await?
        {
            return Ok(None);
        }
        self.load_run_events_by_ids(run_id, &ids).await.map(Some)
    }

    async fn expected_run_event_ids(
        &self,
        run_id: &str,
        limit: usize,
        before_id: Option<i64>,
    ) -> anyhow::Result<Vec<i64>> {
        let mut builder =
            QueryBuilder::<Sqlite>::new("SELECT id FROM team_run_events WHERE run_id = ");
        builder.push_bind(run_id);
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        builder.push(" ORDER BY id DESC LIMIT ");
        builder.push_bind(limit as i64);
        let mut ids = builder.build_query_scalar().fetch_all(&self.db).await?;
        ids.reverse();
        Ok(ids)
    }

    async fn max_run_event_id_for_page(
        &self,
        run_id: &str,
        before_id: Option<i64>,
    ) -> anyhow::Result<i64> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT COALESCE(MAX(id), 0) FROM team_run_events WHERE run_id = ",
        );
        builder.push_bind(run_id);
        if let Some(before_id) = before_id {
            builder.push(" AND id < ");
            builder.push_bind(before_id);
        }
        Ok(builder.build_query_scalar().fetch_one(&self.db).await?)
    }

    async fn load_run_events_by_ids(
        &self,
        run_id: &str,
        ids: &[i64],
    ) -> anyhow::Result<Vec<TeamRunEventRecord>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT id, run_id, step_id, event_type, ts, payload_json FROM team_run_events WHERE run_id = ",
        );
        builder.push_bind(run_id);
        builder.push(" AND id IN (");
        let mut separated = builder.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.db).await?;
        if rows.len() != ids.len() {
            anyhow::bail!(
                "run event index referenced {} rows for run {}, but SQLite hydrated {}",
                ids.len(),
                run_id,
                rows.len()
            );
        }

        let mut by_id = std::collections::HashMap::with_capacity(rows.len());
        for row in rows {
            let event = parse_run_event_row(&row)?;
            by_id.insert(event.event_id, event);
        }

        let mut events = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(event) = by_id.remove(id) else {
                anyhow::bail!("run event index referenced row {id} that SQLite did not return");
            };
            events.push(event);
        }
        Ok(events)
    }

    pub async fn list_actor_messages_for_run(
        &self,
        run_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamActorMessageRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                run_id,
                from_actor_id,
                from_peer_id,
                to_actor_id,
                to_peer_id,
                channel,
                transport,
                route_json,
                payload_json,
                message_kind,
                handling_disposition,
                handled_by_actor_id,
                handled_at,
                status,
                created_at,
                delivered_at
            FROM team_actor_messages
            WHERE run_id = ?1
            ORDER BY id DESC
            LIMIT ?2
            "#,
        )
        .bind(run_id)
        .bind(limit.max(1))
        .fetch_all(&self.db)
        .await?;
        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(parse_team_actor_message_row(&row)?);
        }
        mailbox::enrich_actor_messages(&self.db, &mut messages).await?;
        Ok(messages)
    }
}

fn run_event_id_from_delivery_id(run_id: &str, delivery_id: &str) -> Option<i64> {
    delivery_id
        .strip_prefix(&format!("team_run_event:{run_id}:"))?
        .parse()
        .ok()
}
