use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use super::{
    TeamExecutionClaimRecord, TeamManager, TeamTaskStatus, TeamspaceInviteRecord,
    TeamspaceMemberRecord, hex_encode,
};

const TEAMSPACE_ROLE_VALUES: [&str; 4] = ["owner", "planner", "contributor", "observer"];

fn normalize_teamspace_role(role: &str) -> anyhow::Result<&str> {
    let role = role.trim();
    if TEAMSPACE_ROLE_VALUES.contains(&role) {
        Ok(role)
    } else {
        anyhow::bail!("invalid Teamspace role")
    }
}

fn invite_token_digest(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex_encode(&hasher.finalize())
}

fn parse_member(row: &sqlx::sqlite::SqliteRow) -> TeamspaceMemberRecord {
    TeamspaceMemberRecord {
        team_id: row.get("team_id"),
        user_id: row.get("user_id"),
        role: row.get("role"),
        created_by_user_id: row.get("created_by_user_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn parse_invite(row: &sqlx::sqlite::SqliteRow) -> TeamspaceInviteRecord {
    TeamspaceInviteRecord {
        id: row.get("id"),
        team_id: row.get("team_id"),
        role: row.get("role"),
        created_by_user_id: row.get("created_by_user_id"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        accepted_at: row.get("accepted_at"),
        revoked_at: row.get("revoked_at"),
    }
}

async fn append_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    team_id: &str,
    actor_user_id: Option<&str>,
    event_kind: &str,
    subject_kind: &str,
    subject_id: &str,
    detail: serde_json::Value,
    created_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO team_audit_events (
            team_id, actor_user_id, event_kind, subject_kind, subject_id, detail_json, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(team_id)
    .bind(actor_user_id)
    .bind(event_kind)
    .bind(subject_kind)
    .bind(subject_id)
    .bind(detail.to_string())
    .bind(created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl TeamManager {
    pub async fn handoff_task_execution(
        &self,
        task_id: &str,
        next_member_id: &str,
        actor_user_id: &str,
        reason: &str,
    ) -> anyhow::Result<super::TeamTaskRecord> {
        let task = self.get_task(task_id).await?;
        if task.status != TeamTaskStatus::InProgress {
            anyhow::bail!("only in_progress tasks can be handed off")
        }
        let team = self.get_team(&task.team_id).await?;
        let next_member_id = next_member_id.trim();
        if next_member_id.is_empty()
            || !super::collect_team_member_ids(&team.spec)
                .iter()
                .any(|member_id| member_id == next_member_id)
        {
            anyhow::bail!("handoff target must be a configured Team member")
        }
        if task.assigned_member_id.as_deref() == Some(next_member_id) {
            anyhow::bail!("handoff target already owns the task")
        }

        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let active_claim = sqlx::query(
            r#"
            SELECT owner_member_id, expires_at FROM team_execution_claims
            WHERE entity_kind = 'task' AND entity_id = ?1 AND released_at IS NULL
            "#,
        )
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?;
        let previous_owner = task.assigned_member_id.clone();
        if let Some(claim) = active_claim {
            let claim_owner: String = claim.get("owner_member_id");
            let expires_at: i64 = claim.get("expires_at");
            if expires_at > now && previous_owner.as_deref() != Some(claim_owner.as_str()) {
                anyhow::bail!("task claim does not match its assigned owner")
            }
            sqlx::query(
                "UPDATE team_execution_claims SET released_at = ?1 WHERE entity_kind = 'task' AND entity_id = ?2 AND released_at IS NULL",
            )
            .bind(now)
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        }
        let updated = sqlx::query(
            r#"
            UPDATE team_tasks
            SET assigned_member_id = ?1, updated_at = ?2
            WHERE id = ?3
              AND status = 'in_progress'
              AND assigned_member_id IS ?4
            "#,
        )
        .bind(next_member_id)
        .bind(now)
        .bind(task_id)
        .bind(previous_owner.as_deref())
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            anyhow::bail!("task changed during handoff")
        }
        append_audit_event(
            &mut tx,
            &task.team_id,
            Some(actor_user_id),
            "task_execution.handed_off",
            "task",
            task_id,
            serde_json::json!({"previous_owner": previous_owner, "next_owner": next_member_id, "reason": reason.trim()}),
            now,
        )
        .await?;
        tx.commit().await?;
        self.get_task(task_id).await
    }

    async fn claim_execution_entity(
        &self,
        entity_kind: &str,
        entity_id: &str,
        team_id: &str,
        owner_member_id: &str,
        lease_seconds: i64,
    ) -> anyhow::Result<TeamExecutionClaimRecord> {
        let now = Utc::now().timestamp();
        let expires_at = now + lease_seconds.clamp(1, 3600);
        let mut tx = self.db.begin().await?;
        let current = sqlx::query(
            r#"
            SELECT lease_generation, expires_at, released_at
            FROM team_execution_claims
            WHERE entity_kind = ?1 AND entity_id = ?2
            "#,
        )
        .bind(entity_kind)
        .bind(entity_id)
        .fetch_optional(&mut *tx)
        .await?;
        let generation = if let Some(current) = current {
            let released_at: Option<i64> = current.get("released_at");
            if released_at.is_none() && current.get::<i64, _>("expires_at") > now {
                anyhow::bail!("execution entity already has an active claim")
            }
            current.get::<i64, _>("lease_generation") + 1
        } else {
            1
        };
        sqlx::query(
            r#"
            INSERT INTO team_execution_claims (
                entity_kind, entity_id, team_id, owner_member_id, lease_generation, claimed_at, expires_at, released_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)
            ON CONFLICT(entity_kind, entity_id) DO UPDATE SET
                team_id = excluded.team_id, owner_member_id = excluded.owner_member_id,
                lease_generation = excluded.lease_generation, claimed_at = excluded.claimed_at,
                expires_at = excluded.expires_at, released_at = NULL
            "#,
        )
        .bind(entity_kind)
        .bind(entity_id)
        .bind(team_id)
        .bind(owner_member_id)
        .bind(generation)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        append_audit_event(
            &mut tx,
            team_id,
            None,
            "execution.claimed",
            entity_kind,
            entity_id,
            serde_json::json!({"owner_member_id": owner_member_id, "lease_generation": generation}),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(TeamExecutionClaimRecord {
            entity_kind: entity_kind.to_string(),
            entity_id: entity_id.to_string(),
            team_id: team_id.to_string(),
            owner_member_id: owner_member_id.to_string(),
            lease_generation: generation,
            claimed_at: now,
            expires_at,
        })
    }

    pub async fn is_teamspace_member(&self, team_id: &str, user_id: &str) -> anyhow::Result<bool> {
        let found = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM team_members WHERE team_id = ?1 AND user_id = ?2 AND revoked_at IS NULL",
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(found.is_some())
    }

    pub async fn teamspace_role_for_user(
        &self,
        team_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "SELECT role FROM team_members WHERE team_id = ?1 AND user_id = ?2 AND revoked_at IS NULL",
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_optional(&self.db)
        .await
        .map_err(Into::into)
    }

    pub async fn list_teamspace_members(
        &self,
        team_id: &str,
    ) -> anyhow::Result<Vec<TeamspaceMemberRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT team_id, user_id, role, created_by_user_id, created_at, updated_at
            FROM team_members
            WHERE team_id = ?1 AND revoked_at IS NULL
            ORDER BY created_at ASC, user_id ASC
            "#,
        )
        .bind(team_id)
        .fetch_all(&self.db)
        .await?;
        Ok(rows.iter().map(parse_member).collect())
    }

    pub async fn revoke_teamspace_member(
        &self,
        team_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let member = sqlx::query(
            r#"
            SELECT role FROM team_members
            WHERE team_id = ?1 AND user_id = ?2 AND revoked_at IS NULL
            "#,
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("active Teamspace member not found"))?;
        let role: String = member.get("role");
        if role == "owner" {
            anyhow::bail!("Teamspace owners cannot be revoked")
        }
        let revoked = sqlx::query(
            r#"
            UPDATE team_members
            SET revoked_at = ?1, updated_at = ?1
            WHERE team_id = ?2 AND user_id = ?3 AND revoked_at IS NULL
            "#,
        )
        .bind(now)
        .bind(team_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        if revoked.rows_affected() != 1 {
            anyhow::bail!("Teamspace member changed during revocation")
        }
        append_audit_event(
            &mut tx,
            team_id,
            Some(actor_user_id),
            "team_member.revoked",
            "team_member",
            user_id,
            serde_json::json!({"role": role}),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_teams_for_user(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<super::TeamDefinitionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT td.id, td.name, td.description, td.spec_json, td.owner_user_id, td.created_at, td.updated_at
            FROM team_definitions AS td
            LEFT JOIN team_members AS tm
              ON tm.team_id = td.id AND tm.user_id = ?1 AND tm.revoked_at IS NULL
            WHERE td.owner_user_id = ?1 OR tm.user_id IS NOT NULL
            ORDER BY td.created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&self.db)
        .await?;
        rows.iter().map(super::parse_team_definition_row).collect()
    }

    pub async fn create_teamspace_invite(
        &self,
        team_id: &str,
        role: &str,
        created_by_user_id: &str,
        expires_at: i64,
    ) -> anyhow::Result<(TeamspaceInviteRecord, String)> {
        let role = normalize_teamspace_role(role)?;
        let now = Utc::now().timestamp();
        if expires_at <= now {
            anyhow::bail!("invite expiry must be in the future")
        }
        let id = Uuid::new_v4().to_string();
        let raw_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let digest = invite_token_digest(&raw_token);
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO team_invites (
                id, team_id, token_digest, role, created_by_user_id, created_at, expires_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(team_id)
        .bind(digest)
        .bind(role)
        .bind(created_by_user_id)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        append_audit_event(
            &mut tx,
            team_id,
            Some(created_by_user_id),
            "team_invite.created",
            "team_invite",
            &id,
            serde_json::json!({"role": role, "expires_at": expires_at}),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok((
            TeamspaceInviteRecord {
                id,
                team_id: team_id.to_string(),
                role: role.to_string(),
                created_by_user_id: created_by_user_id.to_string(),
                created_at: now,
                expires_at,
                accepted_at: None,
                revoked_at: None,
            },
            raw_token,
        ))
    }

    pub async fn accept_teamspace_invite(
        &self,
        raw_token: &str,
        user_id: &str,
    ) -> anyhow::Result<TeamspaceMemberRecord> {
        let digest = invite_token_digest(raw_token.trim());
        let invite_id = sqlx::query_scalar::<_, String>(
            r#"
            SELECT id FROM team_invites
            WHERE token_digest = ?1 AND accepted_at IS NULL AND revoked_at IS NULL
              AND expires_at > ?2
            "#,
        )
        .bind(digest)
        .bind(Utc::now().timestamp())
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invite is invalid, expired, revoked, or already used"))?;
        self.accept_teamspace_invite_by_id(&invite_id, user_id)
            .await
    }

    pub async fn active_teamspace_invite_id(&self, raw_token: &str) -> anyhow::Result<String> {
        let digest = invite_token_digest(raw_token.trim());
        let now = Utc::now().timestamp();
        sqlx::query_scalar::<_, String>(
            r#"
            SELECT id FROM team_invites
            WHERE token_digest = ?1 AND accepted_at IS NULL AND revoked_at IS NULL
              AND expires_at > ?2
            "#,
        )
        .bind(digest)
        .bind(now)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invite is invalid, expired, revoked, or already used"))
    }

    pub async fn accept_teamspace_invite_by_id(
        &self,
        invite_id: &str,
        user_id: &str,
    ) -> anyhow::Result<TeamspaceMemberRecord> {
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT id, team_id, role, created_by_user_id, created_at, expires_at, accepted_at, revoked_at
            FROM team_invites
            WHERE id = ?1 AND accepted_at IS NULL
              AND revoked_at IS NULL
              AND expires_at > ?2
            "#,
        )
        .bind(invite_id)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("invite is invalid, expired, revoked, or already used"))?;
        let invite = parse_invite(&row);

        let consumed = sqlx::query(
            r#"
            UPDATE team_invites
            SET accepted_by_user_id = ?1, accepted_at = ?2
            WHERE id = ?3 AND accepted_at IS NULL AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(now)
        .bind(&invite.id)
        .execute(&mut *tx)
        .await?;
        if consumed.rows_affected() != 1 {
            anyhow::bail!("invite was consumed concurrently")
        }

        sqlx::query(
            r#"
            INSERT INTO team_members (
                team_id, user_id, role, created_by_user_id, created_at, updated_at, revoked_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
            ON CONFLICT(team_id, user_id) DO UPDATE SET
                role = excluded.role,
                updated_at = excluded.updated_at,
                revoked_at = NULL
            "#,
        )
        .bind(&invite.team_id)
        .bind(user_id)
        .bind(&invite.role)
        .bind(&invite.created_by_user_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        append_audit_event(
            &mut tx,
            &invite.team_id,
            Some(user_id),
            "team_invite.accepted",
            "team_member",
            user_id,
            serde_json::json!({"invite_id": invite.id, "role": invite.role}),
            now,
        )
        .await?;
        tx.commit().await?;

        Ok(TeamspaceMemberRecord {
            team_id: invite.team_id,
            user_id: user_id.to_string(),
            role: invite.role,
            created_by_user_id: Some(invite.created_by_user_id),
            created_at: now,
            updated_at: now,
        })
    }

    #[allow(dead_code)]
    pub async fn claim_task_execution(
        &self,
        task_id: &str,
        owner_member_id: &str,
        lease_seconds: i64,
    ) -> anyhow::Result<TeamExecutionClaimRecord> {
        let task = self.get_task(task_id).await?;
        if task.status != TeamTaskStatus::InProgress {
            anyhow::bail!("only in_progress tasks can be claimed")
        }
        if task.assigned_member_id.as_deref() != Some(owner_member_id) {
            anyhow::bail!("task claim owner must match assigned_member_id")
        }
        self.claim_execution_entity(
            "task",
            task_id,
            &task.team_id,
            owner_member_id,
            lease_seconds,
        )
        .await
    }

    pub async fn claim_step_execution(
        &self,
        step_id: &str,
        lease_seconds: i64,
    ) -> anyhow::Result<TeamExecutionClaimRecord> {
        let step = self.get_step(step_id).await?;
        let run = self.get_run(&step.run_id).await?;
        self.claim_execution_entity(
            "step",
            step_id,
            &run.team_id,
            &step.member_id,
            lease_seconds,
        )
        .await
    }

    pub async fn release_step_execution(&self, step_id: &str) -> anyhow::Result<()> {
        let step = self.get_step(step_id).await?;
        let run = self.get_run(&step.run_id).await?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(
            "UPDATE team_execution_claims SET released_at = ?1 WHERE entity_kind = 'step' AND entity_id = ?2 AND released_at IS NULL",
        )
        .bind(now)
        .bind(step_id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 1 {
            append_audit_event(
                &mut tx,
                &run.team_id,
                None,
                "execution.released",
                "step",
                step_id,
                serde_json::json!({"owner_member_id": step.member_id, "status": super::team_step_status_to_str(&step.status)}),
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
