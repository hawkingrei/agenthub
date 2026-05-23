use std::collections::HashMap;

use sqlx::Row;

use super::{
    ACTOR_MAIN_PEER_ID, TeamManager, TeamPendingActorUnreadRecord, parse_team_member_specs,
};

impl TeamManager {
    #[allow(dead_code)]
    pub async fn get_agent_session_status(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT status
            FROM agent_sessions
            WHERE id = ?1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|row| row.get("status")))
    }

    pub async fn get_live_member_session(
        &self,
        member_id: &str,
    ) -> anyhow::Result<Option<(String, String)>> {
        let row = sqlx::query(
            r#"
            SELECT id, status
            FROM agent_sessions
            WHERE agent_id = ?1
              AND ended_at IS NULL
            ORDER BY started_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(member_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(|row| (row.get("id"), row.get("status"))))
    }

    pub async fn list_actor_message_status_counts(
        &self,
        run_id: &str,
    ) -> anyhow::Result<HashMap<String, i64>> {
        let rows = sqlx::query(
            r#"
            SELECT status, COUNT(*) AS cnt
            FROM team_actor_messages
            WHERE run_id = ?1
            GROUP BY status
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.db)
        .await?;

        let mut counts = HashMap::with_capacity(rows.len());
        for row in rows {
            let status: String = row.get("status");
            let count: i64 = row.get("cnt");
            counts.insert(status, count);
        }
        Ok(counts)
    }

    pub async fn list_actor_pending_counts_by_actor(
        &self,
        run_id: &str,
    ) -> anyhow::Result<HashMap<String, i64>> {
        let rows = sqlx::query(
            r#"
            SELECT to_actor_id, COUNT(*) AS cnt
            FROM team_actor_messages
            WHERE run_id = ?1
              AND status = 'pending'
              AND handling_disposition = 'untriaged'
              AND to_peer_id = ?2
            GROUP BY to_actor_id
            "#,
        )
        .bind(run_id)
        .bind(ACTOR_MAIN_PEER_ID)
        .fetch_all(&self.db)
        .await?;

        let mut counts = HashMap::with_capacity(rows.len());
        for row in rows {
            let actor_id: String = row.get("to_actor_id");
            let count: i64 = row.get("cnt");
            counts.insert(actor_id, count);
        }
        Ok(counts)
    }

    pub async fn list_pending_actor_unread_counts(
        &self,
    ) -> anyhow::Result<Vec<TeamPendingActorUnreadRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT run_id, to_actor_id, COUNT(*) AS cnt
            FROM team_actor_messages
            WHERE status = 'pending'
              AND handling_disposition = 'untriaged'
              AND to_peer_id = ?1
            GROUP BY run_id, to_actor_id
            ORDER BY run_id ASC, to_actor_id ASC
            "#,
        )
        .bind(ACTOR_MAIN_PEER_ID)
        .fetch_all(&self.db)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(TeamPendingActorUnreadRecord {
                run_id: row.get("run_id"),
                actor_id: row.get("to_actor_id"),
                unread_count: row.get("cnt"),
            });
        }
        Ok(out)
    }

    pub async fn member_role_for_run(
        &self,
        run_id: &str,
        member_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let team_id =
            sqlx::query_scalar::<_, String>("SELECT team_id FROM team_runs WHERE id = ?1")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await?;
        let Some(team_id) = team_id else {
            return Ok(None);
        };
        let team = self.get_team(&team_id).await?;
        let role = parse_team_member_specs(&team.spec)?
            .into_iter()
            .find(|member| member.member_id == member_id)
            .map(|member| member.role);
        Ok(role)
    }
}
