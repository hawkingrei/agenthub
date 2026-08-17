use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PermissionReviewCandidate {
    pub(super) actor_id: String,
    pub(super) dispatch_status: &'static str,
    pub(super) idle_dispatch_status: &'static str,
}

fn permission_review_candidate(
    actor_id: impl Into<String>,
    dispatch_status: &'static str,
    idle_dispatch_status: &'static str,
) -> PermissionReviewCandidate {
    PermissionReviewCandidate {
        actor_id: actor_id.into(),
        dispatch_status,
        idle_dispatch_status,
    }
}

fn worker_permission_review_candidates(
    spec: &Value,
    coordinator_member_id: &str,
    requester_actor_id: &str,
) -> Vec<PermissionReviewCandidate> {
    collect_subordinate_reviewers(spec, coordinator_member_id, requester_actor_id)
        .into_iter()
        .map(|actor_id| {
            permission_review_candidate(actor_id, "worker_dispatched", "worker_idle_dispatched")
        })
        .collect()
}

pub(super) fn collect_team_permission_review_candidates(
    spec: &Value,
    requester_actor_id: &str,
    requester_role: &str,
) -> anyhow::Result<Vec<PermissionReviewCandidate>> {
    let requester_role = requester_role.trim();
    let coordinator_member_id = team_coordinator_member_id(spec)
        .ok_or_else(|| anyhow::anyhow!("team has no coordinator configured"))?;
    let requester_is_coordinator = requester_actor_id == coordinator_member_id
        || requester_role.eq_ignore_ascii_case("coordinator");
    let mut candidates =
        worker_permission_review_candidates(spec, coordinator_member_id, requester_actor_id);

    if requester_is_coordinator {
        if candidates.is_empty() {
            return Err(anyhow::anyhow!(
                "team has no subordinate reviewer configured"
            ));
        }
        return Ok(candidates);
    }

    if coordinator_member_id != requester_actor_id {
        candidates.push(permission_review_candidate(
            coordinator_member_id,
            "coordinator_dispatched",
            "coordinator_idle_dispatched",
        ));
    }
    if candidates.is_empty() {
        return Err(anyhow::anyhow!(
            "team has no non-requester reviewer configured"
        ));
    }
    Ok(candidates)
}

fn team_coordinator_member_id(spec: &Value) -> Option<&str> {
    let spec_obj = spec.as_object()?;
    let members = spec_obj.get("members")?.as_array()?;
    let member_ids = members
        .iter()
        .filter_map(member_id_from_spec)
        .collect::<Vec<_>>();
    spec_obj
        .get("coordinator_member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && member_ids.contains(value))
        .or_else(|| {
            members
                .iter()
                .filter(|member| {
                    member
                        .get("role")
                        .and_then(Value::as_str)
                        .is_some_and(|role| role.trim().eq_ignore_ascii_case("coordinator"))
                })
                .find_map(member_id_from_spec)
        })
        .or_else(|| {
            spec_obj
                .get("entrypoint")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && member_ids.contains(value))
        })
        .or_else(|| member_ids.first().copied())
}

fn collect_subordinate_reviewers(
    spec: &Value,
    coordinator_member_id: &str,
    requester_actor_id: &str,
) -> Vec<String> {
    let Some(members) = spec.get("members").and_then(Value::as_array) else {
        return Vec::new();
    };
    let workers = members
        .iter()
        .filter_map(|member| {
            let member_id = member_id_from_spec(member)?;
            let role = member
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase);
            Some((member_id, role))
        })
        .filter_map(|(member_id, role)| {
            if member_id == requester_actor_id || member_id == coordinator_member_id {
                return None;
            }
            if role.as_deref() == Some("worker") {
                return Some(member_id.to_string());
            }
            None
        })
        .collect::<Vec<_>>();
    if !workers.is_empty() {
        return workers;
    }
    members
        .iter()
        .filter_map(|member| {
            let member_id = member_id_from_spec(member)?;
            if member_id == requester_actor_id || member_id == coordinator_member_id {
                return None;
            }
            Some(member_id.to_string())
        })
        .collect()
}

fn member_id_from_spec(member: &Value) -> Option<&str> {
    member
        .as_object()?
        .get("member_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
