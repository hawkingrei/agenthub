use std::collections::HashMap;
use std::io::Read;

use agenthub_agent_domain::loop_runtime::validate_loop_id;
use agenthub_agent_domain::loop_scheduling::LoopScheduleRequest;

use super::{ActorCommand, ActorOutputMode};

pub(super) fn parse_schedule_command(
    args: &[String],
    output_mode: &mut ActorOutputMode,
) -> anyhow::Result<ActorCommand> {
    let allowed: &[&str] = match args[0].as_str() {
        "loop-schedule" => &["--member-id", "--request-file"],
        "loop-schedules" => &["--member-id", "--after-registration-id", "--limit"],
        "loop-schedule-show" => &["--registration-id", "--after-firing-cursor", "--limit"],
        "loop-schedule-revoke" => &["--registration-id"],
        _ => anyhow::bail!("unknown schedule command"),
    };
    let mut values = HashMap::new();
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--json" {
            *output_mode = ActorOutputMode::Json;
            index += 1;
            continue;
        }
        anyhow::ensure!(
            allowed.contains(&flag),
            "unsupported schedule argument: {flag}"
        );
        anyhow::ensure!(!values.contains_key(flag), "duplicate argument: {flag}");
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
        if flag.ends_with("-id") {
            validate_loop_id(value)?;
        }
        values.insert(flag, value.as_str());
        index += 1;
    }
    let limit: u32 = values.get("--limit").copied().unwrap_or("64").parse()?;
    anyhow::ensure!(
        (1..=256).contains(&limit),
        "limit must be between 1 and 256"
    );
    let member_id = values.get("--member-id").map(|value| (*value).to_owned());
    let required = |flag| {
        values
            .get(flag)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("{flag} is required"))
    };
    match args[0].as_str() {
        "loop-schedule" => {
            let mut bytes = Vec::new();
            std::fs::File::open(required("--request-file")?)?
                .take(16_385)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= 16_384,
                "schedule request exceeds 16384 bytes"
            );
            let request: LoopScheduleRequest = serde_json::from_slice(&bytes)?;
            request.validate()?;
            Ok(ActorCommand::LoopSchedule { member_id, request })
        }
        "loop-schedules" => Ok(ActorCommand::LoopSchedules {
            member_id,
            after_registration_id: values
                .get("--after-registration-id")
                .map(|value| (*value).to_owned()),
            limit,
        }),
        "loop-schedule-show" => {
            let after_firing_cursor = values
                .get("--after-firing-cursor")
                .map(|value| value.parse::<i64>())
                .transpose()?;
            anyhow::ensure!(
                after_firing_cursor.is_none_or(|cursor| cursor >= 0),
                "invalid firing cursor"
            );
            Ok(ActorCommand::LoopScheduleShow {
                registration_id: required("--registration-id")?.into(),
                after_firing_cursor,
                limit,
            })
        }
        _ => Ok(ActorCommand::LoopScheduleRevoke {
            registration_id: required("--registration-id")?.into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_schedule_cli_bounds_documents_and_rejects_identity_overrides() {
        for command in [
            "loop-schedule",
            "loop-schedules",
            "loop-schedule-show",
            "loop-schedule-revoke",
        ] {
            for flag in ["--actor-id", "--run-id", "--activation-id", "--team-id"] {
                assert!(
                    parse_schedule_command(
                        &[command, flag, "forged"].map(String::from),
                        &mut ActorOutputMode::Default
                    )
                    .is_err()
                );
            }
            assert_eq!(
                crate::actor_cli::help::resolve_actor_help_topic(command).unwrap(),
                command
            );
            assert!(crate::actor_cli::help::actor_topic_usage(command).contains("Usage:"));
        }
        let path =
            std::env::temp_dir().join(format!("loop-schedule-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"{"source_key":"test","schedule":{"kind":"due","due_at":200}}"#,
        )
        .unwrap();
        let args = ["loop-schedule", "--request-file", path.to_str().unwrap()].map(String::from);
        assert!(matches!(
            parse_schedule_command(&args, &mut ActorOutputMode::Default).unwrap(),
            ActorCommand::LoopSchedule {
                member_id: None,
                ..
            }
        ));
        for body in [
            " ".repeat(16_385),
            r#"{"source_key":"test","actor_id":"forged","schedule":{"kind":"due","due_at":200}}"#
                .to_string(),
        ] {
            std::fs::write(&path, body).unwrap();
            assert!(parse_schedule_command(&args, &mut ActorOutputMode::Default).is_err());
        }
        std::fs::remove_file(path).unwrap();
        for args in [
            vec!["loop-schedules", "--limit", "257"],
            vec!["loop-schedules", "--limit", "1", "--limit", "2"],
            vec![
                "loop-schedule-show",
                "--registration-id",
                "one",
                "--after-firing-cursor",
                "-1",
            ],
            vec!["loop-schedule-revoke"],
        ] {
            assert!(
                parse_schedule_command(
                    &args.into_iter().map(String::from).collect::<Vec<_>>(),
                    &mut ActorOutputMode::Default
                )
                .is_err()
            );
        }
    }
}
