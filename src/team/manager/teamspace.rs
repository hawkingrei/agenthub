use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use super::{
    TeamExecutionClaimRecord, TeamGoalForkRecord, TeamGoalLeaseRecord, TeamManager,
    TeamTaskNoteCreateInput, TeamTaskRecord, TeamTaskStatus, TeamspaceInviteRecord,
    TeamspaceMemberRecord, hex_encode,
};

const TEAMSPACE_ROLE_VALUES: [&str; 4] = ["owner", "planner", "contributor", "observer"];
const TEAM_ACTIVE_GOAL_LIMIT: i64 = 3;
const MEMBER_ACTIVE_GOAL_LIMIT: i64 = 1;
const TEAM_ACTIVE_FORK_LIMIT: i64 = 2;
const GOAL_FORK_QUESTION_MAX_CHARS: usize = 1_024;
const GOAL_FORK_ACCEPTANCE_CRITERIA_MAX_CHARS: usize = 2_048;
const GOAL_FORK_RESULT_MAX_BYTES: usize = 64 * 1_024;
const GOAL_FORK_RESULT_SCHEMA_TYPE: &str = "object";

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

#[allow(clippy::too_many_arguments)]
pub(super) async fn append_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    team_id: &str,
    actor_user_id: Option<&str>,
    event_kind: &str,
    subject_kind: &str,
    subject_id: &str,
    detail: serde_json::Value,
    created_at: i64,
) -> anyhow::Result<()> {
    // Delegates to the shared ControlStore primitive (docs/features/control-store.md) instead of
    // issuing this INSERT locally, so this Teamspace-originated audit path and any future authority
    // write that adopts `agenthub_db::control_store::record_audit_event` write through the same code.
    agenthub_db::control_store::record_audit_event(
        tx,
        agenthub_db::control_store::AuditEvent {
            team_id,
            actor_user_id,
            event_kind,
            subject_kind,
            subject_id,
            detail,
            created_at,
        },
    )
    .await?;
    Ok(())
}

fn parse_goal_lease(row: &sqlx::sqlite::SqliteRow) -> TeamGoalLeaseRecord {
    TeamGoalLeaseRecord {
        task_id: row.get("task_id"),
        team_id: row.get("team_id"),
        owner_member_id: row.get("owner_member_id"),
        lease_generation: row.get("lease_generation"),
        started_at: row.get("started_at"),
        expires_at: row.get("expires_at"),
        released_at: row.get("released_at"),
        release_reason: row.get("release_reason"),
    }
}

fn parse_goal_fork(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<TeamGoalForkRecord> {
    let result_schema_json: String = row.get("result_schema_json");
    Ok(TeamGoalForkRecord {
        id: row.get("id"),
        parent_task_id: row.get("parent_task_id"),
        parent_lease_generation: row.get("parent_lease_generation"),
        question: row.get("question"),
        acceptance_criteria: row.get("acceptance_criteria"),
        result_schema: serde_json::from_str(&result_schema_json)?,
        profile: row.get("profile"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        completed_at: row.get("completed_at"),
        result: row
            .try_get::<Option<String>, _>("result_json")?
            .map(|value| serde_json::from_str(&value))
            .transpose()?,
    })
}

async fn claim_task_goal_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    task: &TeamTaskRecord,
    owner_member_id: &str,
    generation: i64,
    expires_at: i64,
    now: i64,
) -> anyhow::Result<TeamGoalLeaseRecord> {
    let current = sqlx::query(
        r#"
        SELECT task_id, team_id, owner_member_id, lease_generation, started_at, expires_at,
               released_at, release_reason
        FROM team_goal_leases
        WHERE task_id = ?1
        "#,
    )
    .bind(&task.id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(current) = current {
        let current = parse_goal_lease(&current);
        if current.released_at.is_none() && current.expires_at > now {
            anyhow::bail!("task already has an active goal")
        }
        if generation <= current.lease_generation {
            anyhow::bail!("goal lease generation must advance")
        }
    }

    let active_team_goals = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_goal_leases WHERE team_id = ?1 AND released_at IS NULL AND expires_at > ?2",
    )
    .bind(&task.team_id)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;
    if active_team_goals >= TEAM_ACTIVE_GOAL_LIMIT {
        anyhow::bail!("Team active goal capacity is exhausted")
    }
    let active_member_goals = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_goal_leases WHERE team_id = ?1 AND owner_member_id = ?2 AND released_at IS NULL AND expires_at > ?3",
    )
    .bind(&task.team_id)
    .bind(owner_member_id)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;
    if active_member_goals >= MEMBER_ACTIVE_GOAL_LIMIT {
        anyhow::bail!("member already has an active goal")
    }

    sqlx::query(
        r#"
        INSERT INTO team_goal_leases (
            task_id, team_id, owner_member_id, lease_generation, started_at, expires_at,
            released_at, release_reason
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)
        ON CONFLICT(task_id) DO UPDATE SET
            team_id = excluded.team_id,
            owner_member_id = excluded.owner_member_id,
            lease_generation = excluded.lease_generation,
            started_at = excluded.started_at,
            expires_at = excluded.expires_at,
            released_at = NULL,
            release_reason = NULL
        "#,
    )
    .bind(&task.id)
    .bind(&task.team_id)
    .bind(owner_member_id)
    .bind(generation)
    .bind(now)
    .bind(expires_at)
    .execute(&mut **tx)
    .await?;
    append_audit_event(
        tx,
        &task.team_id,
        None,
        "goal.claimed",
        "task",
        &task.id,
        serde_json::json!({"owner_member_id": owner_member_id, "lease_generation": generation}),
        now,
    )
    .await?;
    Ok(TeamGoalLeaseRecord {
        task_id: task.id.clone(),
        team_id: task.team_id.clone(),
        owner_member_id: owner_member_id.to_string(),
        lease_generation: generation,
        started_at: now,
        expires_at,
        released_at: None,
        release_reason: None,
    })
}

pub(super) async fn release_task_goal_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    team_id: &str,
    task_id: &str,
    reason: &str,
    now: i64,
) -> anyhow::Result<bool> {
    sqlx::query(
        "UPDATE team_execution_claims SET released_at = ?1 WHERE entity_kind = 'task' AND entity_id = ?2 AND released_at IS NULL",
    )
    .bind(now)
    .bind(task_id)
    .execute(&mut **tx)
    .await?;
    let result = sqlx::query(
        "UPDATE team_goal_leases SET released_at = ?1, release_reason = ?2 WHERE task_id = ?3 AND released_at IS NULL",
    )
    .bind(now)
    .bind(reason)
    .bind(task_id)
    .execute(&mut **tx)
    .await?;
    if result.rows_affected() == 1 {
        append_audit_event(
            tx,
            team_id,
            None,
            "goal.released",
            "task",
            task_id,
            serde_json::json!({"reason": reason}),
            now,
        )
        .await?;
        return Ok(true);
    }
    Ok(false)
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
        agenthub_db::control_store::require_guarded_write_applied(updated.rows_affected())
            .map_err(|_| anyhow::anyhow!("task changed during handoff"))?;
        release_task_goal_in_tx(&mut tx, &task.team_id, task_id, "handoff", now).await?;
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
        let generation = match &current {
            Some(current) => {
                let released_at: Option<i64> = current.get("released_at");
                if released_at.is_none() && current.get::<i64, _>("expires_at") > now {
                    anyhow::bail!("execution entity already has an active claim")
                }
                agenthub_db::control_store::next_fencing_generation(Some(
                    current.get("lease_generation"),
                ))
            }
            None => agenthub_db::control_store::next_fencing_generation(None),
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
        agenthub_db::control_store::require_guarded_write_applied(revoked.rows_affected())
            .map_err(|_| anyhow::anyhow!("Teamspace member changed during revocation"))?;
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
            WHERE td.owner_user_id IS NULL OR td.owner_user_id = ?1 OR tm.user_id IS NOT NULL
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
        agenthub_db::control_store::require_guarded_write_applied(consumed.rows_affected())
            .map_err(|_| anyhow::anyhow!("invite was consumed concurrently"))?;

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
        let now = Utc::now().timestamp();
        let expires_at = now + lease_seconds.clamp(1, 3600);
        let mut tx = self.db.begin().await?;
        let task_is_still_claimable = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT 1
            FROM team_tasks
            WHERE id = ?1 AND team_id = ?2 AND status = 'in_progress'
              AND assigned_member_id = ?3
            "#,
        )
        .bind(task_id)
        .bind(&task.team_id)
        .bind(owner_member_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if !task_is_still_claimable {
            anyhow::bail!("task changed before its execution claim was reserved")
        }

        let current_goal = sqlx::query(
            "SELECT lease_generation, expires_at, released_at FROM team_goal_leases WHERE task_id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?;
        let current_claim = sqlx::query(
            "SELECT lease_generation, expires_at, released_at FROM team_execution_claims WHERE entity_kind = 'task' AND entity_id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?;
        for current in [&current_goal, &current_claim].into_iter().flatten() {
            let released_at: Option<i64> = current.get("released_at");
            if released_at.is_none() && current.get::<i64, _>("expires_at") > now {
                anyhow::bail!("execution entity already has an active claim")
            }
        }
        let generation = current_goal
            .as_ref()
            .map(|row| row.get::<i64, _>("lease_generation"))
            .into_iter()
            .chain(
                current_claim
                    .as_ref()
                    .map(|row| row.get::<i64, _>("lease_generation")),
            )
            .max()
            .unwrap_or(0)
            + 1;
        claim_task_goal_in_tx(&mut tx, &task, owner_member_id, generation, expires_at, now).await?;
        sqlx::query(
            r#"
            INSERT INTO team_execution_claims (
                entity_kind, entity_id, team_id, owner_member_id, lease_generation, claimed_at, expires_at, released_at
            ) VALUES ('task', ?1, ?2, ?3, ?4, ?5, ?6, NULL)
            ON CONFLICT(entity_kind, entity_id) DO UPDATE SET
                team_id = excluded.team_id, owner_member_id = excluded.owner_member_id,
                lease_generation = excluded.lease_generation, claimed_at = excluded.claimed_at,
                expires_at = excluded.expires_at, released_at = NULL
            "#,
        )
        .bind(task_id)
        .bind(&task.team_id)
        .bind(owner_member_id)
        .bind(generation)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        append_audit_event(
            &mut tx,
            &task.team_id,
            None,
            "execution.claimed",
            "task",
            task_id,
            serde_json::json!({"owner_member_id": owner_member_id, "lease_generation": generation}),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(TeamExecutionClaimRecord {
            entity_kind: "task".to_string(),
            entity_id: task_id.to_string(),
            team_id: task.team_id,
            owner_member_id: owner_member_id.to_string(),
            lease_generation: generation,
            claimed_at: now,
            expires_at,
        })
    }

    #[allow(dead_code)]
    pub async fn get_task_goal_lease(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<TeamGoalLeaseRecord>> {
        let row = sqlx::query(
            r#"
            SELECT task_id, team_id, owner_member_id, lease_generation, started_at, expires_at,
                   released_at, release_reason
            FROM team_goal_leases
            WHERE task_id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.as_ref().map(parse_goal_lease))
    }

    pub async fn create_goal_fork(
        &self,
        task_id: &str,
        question: &str,
        acceptance_criteria: &str,
        result_schema: serde_json::Value,
        expires_at: i64,
        actor_user_id: Option<&str>,
    ) -> anyhow::Result<TeamGoalForkRecord> {
        let now = Utc::now().timestamp();
        let question = question.trim();
        let acceptance_criteria = acceptance_criteria.trim();
        if question.is_empty() || acceptance_criteria.is_empty() || expires_at <= now {
            anyhow::bail!("fork question, acceptance criteria, and future expiry are required")
        }
        if question.chars().count() > GOAL_FORK_QUESTION_MAX_CHARS
            || acceptance_criteria.chars().count() > GOAL_FORK_ACCEPTANCE_CRITERIA_MAX_CHARS
        {
            anyhow::bail!("fork question or acceptance criteria exceeds its size limit")
        }
        if result_schema != serde_json::json!({"type": GOAL_FORK_RESULT_SCHEMA_TYPE}) {
            anyhow::bail!("fork result schema must be the v1 object schema")
        }

        let id = Uuid::new_v4().to_string();
        let mut tx = self.db.begin().await?;
        let lease_row = sqlx::query(
            r#"
            SELECT task_id, team_id, owner_member_id, lease_generation, started_at, expires_at,
                   released_at, release_reason
            FROM team_goal_leases
            WHERE task_id = ?1 AND released_at IS NULL AND expires_at > ?2
            "#,
        )
        .bind(task_id)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("fork parent must have an active goal"))?;
        let lease = parse_goal_lease(&lease_row);
        let expires_at = expires_at.min(lease.expires_at);
        if expires_at <= now {
            anyhow::bail!("fork parent lease expires too soon")
        }
        let active_forks = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM team_goal_forks AS fork
            JOIN team_goal_leases AS goal ON goal.task_id = fork.parent_task_id
            WHERE goal.team_id = ?1
              AND goal.lease_generation = fork.parent_lease_generation
              AND goal.released_at IS NULL
              AND goal.expires_at > ?2
              AND fork.completed_at IS NULL
              AND fork.expires_at > ?2
            "#,
        )
        .bind(&lease.team_id)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        if active_forks >= TEAM_ACTIVE_FORK_LIMIT {
            anyhow::bail!("Team active fork capacity is exhausted")
        }
        let result_schema_json = result_schema.to_string();
        sqlx::query(
            r#"
            INSERT INTO team_goal_forks (
                id, parent_task_id, parent_lease_generation, question, acceptance_criteria,
                result_schema_json, profile, created_at, expires_at, completed_at, result_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'read_only', ?7, ?8, NULL, NULL)
            "#,
        )
        .bind(&id)
        .bind(task_id)
        .bind(lease.lease_generation)
        .bind(question)
        .bind(acceptance_criteria)
        .bind(&result_schema_json)
        .bind(now)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        append_audit_event(
            &mut tx,
            &lease.team_id,
            actor_user_id,
            "goal_fork.created",
            "goal_fork",
            &id,
            serde_json::json!({
                "parent_task_id": task_id,
                "parent_lease_generation": lease.lease_generation,
                "expires_at": expires_at,
                "profile": "read_only",
            }),
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(TeamGoalForkRecord {
            id,
            parent_task_id: task_id.to_string(),
            parent_lease_generation: lease.lease_generation,
            question: question.to_string(),
            acceptance_criteria: acceptance_criteria.to_string(),
            result_schema,
            profile: "read_only".to_string(),
            created_at: now,
            expires_at,
            completed_at: None,
            result: None,
        })
    }

    pub async fn complete_goal_fork(
        &self,
        fork_id: &str,
        result: serde_json::Value,
        actor_user_id: Option<&str>,
    ) -> anyhow::Result<TeamGoalForkRecord> {
        let now = Utc::now().timestamp();
        if !result.is_object() {
            anyhow::bail!("fork result must match the v1 object schema")
        }
        if result.to_string().len() > GOAL_FORK_RESULT_MAX_BYTES {
            anyhow::bail!("fork result exceeds its size limit")
        }
        let result = super::redact_sensitive_json(&result);
        let result_json = result.to_string();
        let mut tx = self.db.begin().await?;
        let fork_row = sqlx::query(
            r#"
            SELECT id, parent_task_id, parent_lease_generation, question, acceptance_criteria,
                   result_schema_json, profile, created_at, expires_at, completed_at, result_json
            FROM team_goal_forks
            WHERE id = ?1
            "#,
        )
        .bind(fork_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("fork is missing"))?;
        let fork = parse_goal_fork(&fork_row)?;
        if fork.completed_at.is_some() || fork.expires_at <= now {
            anyhow::bail!("fork is completed or expired")
        }
        let active_goal = sqlx::query(
            r#"
            SELECT team_id, owner_member_id
            FROM team_goal_leases
            WHERE task_id = ?1 AND lease_generation = ?2
              AND released_at IS NULL AND expires_at > ?3
            "#,
        )
        .bind(&fork.parent_task_id)
        .bind(fork.parent_lease_generation)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("fork parent goal is no longer active"))?;
        let team_id: String = active_goal.get("team_id");
        let owner_member_id: String = active_goal.get("owner_member_id");
        let updated = sqlx::query(
            r#"
            UPDATE team_goal_forks
            SET completed_at = ?1, result_json = ?2
            WHERE id = ?3 AND completed_at IS NULL AND expires_at > ?1
              AND EXISTS (
                  SELECT 1 FROM team_goal_leases
                  WHERE task_id = team_goal_forks.parent_task_id
                    AND lease_generation = team_goal_forks.parent_lease_generation
                    AND released_at IS NULL AND expires_at > ?1
              )
            "#,
        )
        .bind(now)
        .bind(&result_json)
        .bind(fork_id)
        .execute(&mut *tx)
        .await?;
        agenthub_db::control_store::require_guarded_write_applied(updated.rows_affected())
            .map_err(|_| anyhow::anyhow!("fork changed before completion"))?;
        let result_text = result
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&result_json)
            .to_string();
        let from_actor_id = format!("goal-fork:{fork_id}");
        let idempotency_key = format!("goal-fork-result:{fork_id}");
        let note = TeamTaskNoteCreateInput {
            from_actor_id: &from_actor_id,
            to_actor_id: Some(&owner_member_id),
            route: "task_note",
            payload: serde_json::json!({
                "kind": "result",
                "text": result_text,
                "goal_fork_id": fork_id,
                "parent_lease_generation": fork.parent_lease_generation,
                "result": result,
            }),
            idempotency_key: Some(&idempotency_key),
        };
        let appended_note = self
            .insert_task_conversation_message_in_tx(&mut tx, &fork.parent_task_id, &note)
            .await?;
        append_audit_event(
            &mut tx,
            &team_id,
            actor_user_id,
            "goal_fork.completed",
            "goal_fork",
            fork_id,
            serde_json::json!({
                "parent_task_id": fork.parent_task_id,
                "parent_lease_generation": fork.parent_lease_generation,
            }),
            now,
        )
        .await?;
        let row = sqlx::query(
            "SELECT id, parent_task_id, parent_lease_generation, question, acceptance_criteria, result_schema_json, profile, created_at, expires_at, completed_at, result_json FROM team_goal_forks WHERE id = ?1",
        )
        .bind(fork_id)
        .fetch_one(&mut *tx)
        .await?;
        let completed = parse_goal_fork(&row)?;
        tx.commit().await?;
        let (conversation, message, created) = appended_note;
        if created {
            self.spawn_archive_task_conversation_message(&conversation, &message);
            self.emit_task_conversation_message_created(&conversation, &message);
        }
        Ok(completed)
    }

    pub async fn get_goal_fork(&self, fork_id: &str) -> anyhow::Result<TeamGoalForkRecord> {
        let row = sqlx::query(
            "SELECT id, parent_task_id, parent_lease_generation, question, acceptance_criteria, result_schema_json, profile, created_at, expires_at, completed_at, result_json FROM team_goal_forks WHERE id = ?1",
        )
        .bind(fork_id)
        .fetch_one(&self.db)
        .await?;
        parse_goal_fork(&row)
    }

    pub async fn list_goal_forks(
        &self,
        task_id: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamGoalForkRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, parent_task_id, parent_lease_generation, question, acceptance_criteria,
                   result_schema_json, profile, created_at, expires_at, completed_at, result_json
            FROM team_goal_forks
            WHERE parent_task_id = ?1
            ORDER BY created_at ASC, id ASC
            LIMIT ?2
            "#,
        )
        .bind(task_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.db)
        .await?;
        rows.iter().map(parse_goal_fork).collect()
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
