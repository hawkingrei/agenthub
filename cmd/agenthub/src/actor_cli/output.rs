use super::{ActorCommand, ActorOutputFormat, ActorOutputMode, ActorOutputPreference};
use anyhow::Context;

pub(super) fn resolve_actor_output_format(
    mode: ActorOutputMode,
    preference: ActorOutputPreference,
) -> ActorOutputFormat {
    match mode {
        ActorOutputMode::Json => ActorOutputFormat::Json,
        ActorOutputMode::Default => match preference {
            ActorOutputPreference::ToonPreferred => ActorOutputFormat::Toon,
            ActorOutputPreference::JsonPreferred => ActorOutputFormat::Json,
        },
    }
}

pub(super) fn actor_output_preference_for_command(command: &ActorCommand) -> ActorOutputPreference {
    match command {
        ActorCommand::Help { .. } => ActorOutputPreference::ToonPreferred,
        ActorCommand::TeamMembers { .. }
        | ActorCommand::TeamTasks { .. }
        | ActorCommand::Inbox { .. }
        | ActorCommand::TeamTaskCreate { .. }
        | ActorCommand::TeamTaskUpdate { .. }
        | ActorCommand::TimeTriggerList { .. }
        | ActorCommand::TimeTriggerSet { .. }
        | ActorCommand::TimeTriggerCancel { .. } => ActorOutputPreference::ToonPreferred,
        ActorCommand::Ack { .. }
        | ActorCommand::Send { .. }
        | ActorCommand::PermissionReviewRespond { .. } => ActorOutputPreference::JsonPreferred,
    }
}

fn encode_toon_output<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    toon_format::encode_default(value).context("failed to encode TOON output")
}

fn encode_json_output<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    serde_json::to_string(value).context("failed to encode JSON output")
}

pub(super) fn encode_actor_output<T: serde::Serialize>(
    value: &T,
    mode: ActorOutputMode,
    preference: ActorOutputPreference,
) -> anyhow::Result<String> {
    match resolve_actor_output_format(mode, preference) {
        ActorOutputFormat::Toon => encode_toon_output(value),
        ActorOutputFormat::Json => encode_json_output(value),
    }
}

pub(super) fn write_actor_output<T: serde::Serialize>(
    value: &T,
    mode: ActorOutputMode,
    preference: ActorOutputPreference,
) -> anyhow::Result<()> {
    let output = encode_actor_output(value, mode, preference)?;
    println!("{output}");
    Ok(())
}
