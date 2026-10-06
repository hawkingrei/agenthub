//! Opt-in acceptance with synthetic work and an explicitly configured model provider.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, ensure};

use super::*;

mod semantic;
mod team_cycle;

pub(super) async fn configure(fixture: &mut Fixture) -> anyhow::Result<()> {
    let source = std::env::var_os("AGENTHUB_RARA_PROVIDER_CONFIG")
        .context("explicit private provider configuration path required")?;
    let provider = std::fs::read(source).context("read explicit provider configuration")?;
    std::fs::set_permissions(&fixture.directory, std::fs::Permissions::from_mode(0o700))?;
    let state = fixture.directory.join("native-state");
    std::fs::create_dir_all(&state)?;
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700))?;
    let path = state.join("config.json");
    if path.exists() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(&provider)?;
    let binary = std::env::var("AGENTHUB_RARA_TEST_BINARY").context("qualified native binary")?;
    let control =
        crate::agenthub_binary::resolve_agenthub_binary_path().context("control binary path")?;
    std::fs::write(
        fixture.directory.join("native-settings.json"),
        json!({"binary":binary,"control":control}).to_string(),
    )?;
    let wrapper = fixture.directory.join("native-runtime");
    let script = if wrapper.exists() {
        std::fs::read_to_string(&wrapper)?
    } else {
        WRAPPER.to_owned()
    };
    // The production Team launcher already disables ambient extensions and memory.
    std::fs::write(&wrapper, script)?;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))?;
    let mut config = (*fixture.state.agents.loop_app_config).clone();
    config.rara = Some(agenthub_config::RaraConfig {
        binary: Some(wrapper.to_string_lossy().into_owned()),
        startup_timeout_seconds: Some(30),
        shutdown_timeout_seconds: Some(5),
        ..Default::default()
    });
    fixture.state.agents = Arc::new((*fixture.state.agents).clone().with_loop_app_config(config));
    sqlx::query("UPDATE agents SET command = 'rara', args = '[]', runtime_model = NULL WHERE id IN ('planner', 'worker')")
        .execute(&fixture.state.db).await?;
    Ok(())
}

pub(super) async fn set_member(
    fixture: &Fixture,
    actor: &str,
    fields: Value,
) -> anyhow::Result<()> {
    let mut team = fixture.state.teams.get_team(&fixture.team_id).await?;
    let member = team.spec["members"]
        .as_array_mut()
        .context("Team members")?
        .iter_mut()
        .find(|member| member["member_id"] == actor)
        .context("configured member")?;
    member
        .as_object_mut()
        .context("member fields")?
        .extend(fields.as_object().context("updated fields")?.clone());
    sqlx::query("UPDATE team_definitions SET spec_json = ? WHERE id = ?")
        .bind(team.spec.to_string())
        .bind(&fixture.team_id)
        .execute(&fixture.state.db)
        .await?;
    Ok(())
}

pub(super) async fn execute(
    fixture: &Fixture,
    reservation: LoopReservation,
    allowed_command: Option<&str>,
) -> anyhow::Result<LoopActivation> {
    let id = reservation.activation_id.as_deref().context("activation")?;
    let actor = &reservation.actor_id;
    let execution = fixture
        .state
        .agents
        .execute_loop_activation(fixture.state.teams.clone(), reservation.clone());
    tokio::pin!(execution);
    let mut approvals = 0;
    let pending = approve_pending(fixture, id, actor, allowed_command, &mut approvals);
    tokio::time::timeout(Duration::from_secs(180), async {
        tokio::select! {
            result = &mut execution => result,
            result = pending => result,
        }
    })
    .await
    .context("configured activation timed out")??;
    ensure!(approvals == usize::from(allowed_command.is_some()));
    let store = LoopStore::new(fixture.state.db.clone());
    ensure!(store.reservation(&fixture.team_id, actor).await?.is_none());
    ensure!(!fixture.state.agents.inner.read().await.contains_key(actor));
    let activation = store
        .activation(&fixture.team_id, id)
        .await?
        .context("finished activation")?;
    ensure!(
        activation.state == LoopActivationState::Finished,
        "activation did not finish canonically"
    );
    let history = fixture
        .state
        .agents
        .runtime_history(
            actor,
            activation.session_id.as_deref().context("local session")?,
            100,
            None,
        )
        .await?
        .context("retained runtime history")?;
    ensure!(history.closed && !history.streams_truncated);
    ensure!(
        history
            .streams
            .iter()
            .all(|stream| stream.cursor.gap.is_none())
    );
    ensure!(
        history
            .receipts
            .iter()
            .filter(|receipt| receipt.kind == RuntimeRequestKind::GuardedPrompt
                && receipt.status == RuntimeRequestStatus::Accepted)
            .count()
            == 1
    );
    Ok(activation)
}

async fn approve_pending(
    fixture: &Fixture,
    id: &str,
    actor: &str,
    allowed_command: Option<&str>,
    approvals: &mut usize,
) -> anyhow::Result<()> {
    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let permission: Option<String> = sqlx::query_scalar(
            "SELECT p.id FROM acp_permission_requests p JOIN loop_activations a ON a.session_id = p.session_id WHERE a.id = ? AND p.agent_id = ? AND p.status = 'pending'",
        ).bind(id).bind(actor).fetch_optional(&fixture.state.db).await?;
        if let Some(permission) = permission {
            let command = allowed_command.context("unexpected shell approval")?;
            ensure!(*approvals == 0, "unexpected repeated shell approval");
            verify_command(fixture, id, &permission, command).await?;
            let result = fixture
                .state
                .agents
                .permissions
                .respond(
                    &permission,
                    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new("once")),
                    Some("once".into()),
                    Some("fixture-operator".into()),
                )
                .await?;
            ensure!(result == crate::acp::AcpPermissionRespondResult::Applied);
            *approvals += 1;
        }
    }
}

async fn verify_command(
    fixture: &Fixture,
    activation: &str,
    id: &str,
    command: &str,
) -> anyhow::Result<()> {
    let record = fixture
        .state
        .agents
        .permissions
        .get(id)
        .await?
        .context("approval missing")?;
    let call = record.tool_call.context("approval tool details missing")?;
    let input = &call["rawInput"];
    ensure!(
        input["command"] == command,
        "unexpected command; refusing approval"
    );
    ensure!(input["program"].is_null());
    ensure!(input["args"].as_array().is_none_or(Vec::is_empty));
    ensure!(
        input["env"]
            .as_object()
            .is_none_or(serde_json::Map::is_empty)
    );
    ensure!(input["allow_net"] != true && input["run_in_background"] != true);
    ensure!(input["sandbox_permissions"] == "require_escalated");
    if let Some(cwd) = input["cwd"].as_str() {
        let launch = LoopStore::new(fixture.state.db.clone())
            .activation(&fixture.team_id, activation)
            .await?
            .and_then(|activation| activation.launch)
            .context("pinned launch workspace")?;
        ensure!(
            cwd == "." || std::fs::canonicalize(cwd)? == std::fs::canonicalize(launch.workspace)?
        );
    }
    Ok(())
}
