use super::{
    ACTOR_HELP_TOPIC_ACK, ACTOR_HELP_TOPIC_INBOX, ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND,
    ACTOR_HELP_TOPIC_SEND, ACTOR_HELP_TOPICS,
};
use crate::actor_runtime_env::{
    ACTOR_RUNTIME_ACTOR_ID_ENV, ACTOR_RUNTIME_AGENT_ID_ENV, ACTOR_RUNTIME_CHANNEL_ENV,
    ACTOR_RUNTIME_CURRENT_RUN_ID_ENV, ACTOR_RUNTIME_TEAM_ID_ENV,
};

pub(super) fn is_help_flag(arg: &str) -> bool {
    matches!(arg.trim(), "--help" | "-h")
}

pub(super) fn is_help_subcommand(arg: &str) -> bool {
    arg.trim() == "help"
}

fn normalize_help_topic(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

pub(super) fn resolve_actor_help_topic(raw: &str) -> anyhow::Result<&'static str> {
    let normalized = normalize_help_topic(raw);
    anyhow::ensure!(
        !normalized.is_empty(),
        "actor help topic must be a non-empty string"
    );
    let mut matches = ACTOR_HELP_TOPICS
        .iter()
        .copied()
        .filter(|topic| {
            let topic_normalized = normalize_help_topic(topic);
            topic_normalized == normalized || topic_normalized.starts_with(&normalized)
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [topic] => Ok(*topic),
        [] => Err(anyhow::anyhow!(
            "unknown actor help topic '{}'; try one of: {}",
            raw.trim(),
            ACTOR_HELP_TOPICS.join(", ")
        )),
        _ => Err(anyhow::anyhow!(
            "ambiguous actor help topic '{}'; matches: {}",
            raw.trim(),
            matches.join(", ")
        )),
    }
}

pub(super) fn actor_usage() -> String {
    format!(
        "Usage:\n  agenthub actor help [topic]\n  agenthub actor [--json] team-members [--team-id <team_id>] [--run-id <run_id>]\n  agenthub actor [--json] team-tasks [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--status <all|open|in_progress|in_review|completed|canceled>] [--include-shared-thread]\n  agenthub actor [--json] team-task-create --title <title> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--status <open|in_progress|in_review|completed|canceled>] [--topic <topic>] [--context-json <json>]\n  agenthub actor [--json] team-task-update --task-id <task_id> [--status <open|in_progress|in_review|completed|canceled>] [--assigned-member-id <member_id> | --unassign] [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] inbox [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--after-id <id>] [--include-delivered] [--auto-ack]\n  agenthub actor [--json] ack --message-id <id> [--message-id <id> ...] [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] send (--to-actor-id <actor_id> | --to-agent-id <agent_id> | --channel-id <channel_id>) (--text <markdown> | --text-file <path> | --payload-json <json> | --payload-file <path>) [--run-id <run_id>] [--from-actor-id <actor_id> | --from-agent-id <agent_id>] [--channel <name>] [--transport <local|remote>] [--route-json <json>] [--idempotency-key <key>] [--allow-duplicate]\n  agenthub actor [--json] time-trigger-set --delay-seconds <seconds> --message <text> [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] time-trigger-list [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>]\n  agenthub actor [--json] time-trigger-cancel --trigger-id <trigger_id> [--actor-id <actor_id> | --agent-id <agent_id>]\n  agenthub actor [--json] permission-review-respond --permission-id <id> [--permission-id <id> ...] [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--option-id <option_id> | --outcome cancelled]\n\nQuick start:\n  agenthub actor inbox\n  agenthub actor ack --message-id <id>\n  agenthub actor send --to-actor-id <actor_id> --text-file <path>\n\nHelp:\n  `agenthub actor help inbox`\n  `agenthub actor help perm`\n  `agenthub actor ack --help`\n  Topic matching is fuzzy for help only; command execution remains strict.\n\nOutput:\n  Read-heavy results (`team-members`, `team-tasks`, `inbox`, `time-trigger-list`) default to TOON on stdout.\n  Human-oriented task and trigger confirmations (`team-task-create`, `team-task-update`, `time-trigger-set`, `time-trigger-cancel`) default to TOON on stdout.\n  Machine-oriented confirmations (`ack`, `send`, `permission-review-respond`) default to compact JSON for script compatibility.\n  `--json` forces JSON output for all structured success results.\n\nMailbox note:\n  `actor inbox` is read-only by default. Use `actor ack` to mark consumed messages delivered, or pass `--auto-ack` explicitly when you want inbox reads to consume pending messages.\n\nEnvironment fallback:\n  {}\n  {}\n  {}\n  {}\n  {}\n",
        ACTOR_RUNTIME_TEAM_ID_ENV,
        ACTOR_RUNTIME_CURRENT_RUN_ID_ENV,
        ACTOR_RUNTIME_ACTOR_ID_ENV,
        ACTOR_RUNTIME_AGENT_ID_ENV,
        ACTOR_RUNTIME_CHANNEL_ENV,
    )
}

pub(super) fn actor_topic_usage(topic: &str) -> String {
    match topic {
        ACTOR_HELP_TOPIC_INBOX => "Usage:\n  agenthub actor inbox [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--after-id <id>] [--include-delivered] [--auto-ack]\n\nExamples:\n  agenthub actor inbox\n  agenthub actor inbox --include-delivered\n  agenthub actor inbox --auto-ack\n\nNotes:\n  `actor inbox` is read-only by default.\n  Use `--auto-ack` only when you want inbox reads to consume pending messages in bulk.\n  In team runtime, omitting `--run-id` and `--actor-id` uses actor runtime env fallback.\n".to_string(),
        ACTOR_HELP_TOPIC_ACK => "Usage:\n  agenthub actor ack --message-id <id> [--message-id <id> ...] [--run-id <run_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n\nExamples:\n  agenthub actor ack --message-id 123\n  agenthub actor ack --message-id 123 --message-id 124\n  agenthub actor ack --run-id <run_id> --actor-id <actor_id> --message-id 123\n\nNotes:\n  `actor ack` marks mailbox messages delivered after you have processed them.\n  Repeating `--message-id` processes the acknowledgements sequentially and returns a JSON array.\n  In team runtime, omitting `--run-id` and `--actor-id` uses actor runtime env fallback.\n".to_string(),
        ACTOR_HELP_TOPIC_SEND => "Usage:\n  agenthub actor send (--to-actor-id <actor_id> | --to-agent-id <agent_id> | --channel-id <channel_id>) (--text <markdown> | --text-file <path> | --payload-json <json> | --payload-file <path>) [--run-id <run_id>] [--from-actor-id <actor_id> | --from-agent-id <agent_id>] [--channel <name>] [--transport <local|remote>] [--route-json <json>] [--idempotency-key <key>] [--allow-duplicate]\n\nExamples:\n  agenthub actor send --to-actor-id reviewer --text-file .agenthubmemory/mailbox/outbox/review.md\n  agenthub actor send --channel-id all --text-file .agenthubmemory/mailbox/outbox/channel-update.md\n  agenthub actor send --to-actor-id reviewer --payload-file .agenthubmemory/mailbox/outbox/review.json\n\nNotes:\n  Prefer `--text-file` for multi-line or reusable mailbox messages so `agenthub` stays at the command prefix.\n  Use `--text` only for short inline coordination notes.\n  Use `--payload-json` or `--payload-file` only for structured machine-readable envelopes.\n".to_string(),
        ACTOR_HELP_TOPIC_PERMISSION_REVIEW_RESPOND => "Usage:\n  agenthub actor permission-review-respond --permission-id <id> [--permission-id <id> ...] [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--option-id <option_id> | --outcome cancelled]\n\nExamples:\n  agenthub actor permission-review-respond --permission-id <id> --option-id allow\n  agenthub actor permission-review-respond --permission-id <id> --option-id approved-for-session\n  agenthub actor permission-review-respond --permission-id <id-1> --permission-id <id-2> --option-id allow\n  agenthub actor permission-review-respond --permission-id <id> --outcome cancelled\n\nNotes:\n  This command is for the currently assigned reviewer only.\n  Repeating `--permission-id` processes the requests sequentially and returns a JSON array.\n  Persistent/session approvals are selected with the request-specific `--option-id` value; `--outcome` currently supports only `cancelled`.\n  In team runtime, review writes should go through local authority internal gRPC instead of direct sqlite writes.\n".to_string(),
        "team-members" => {
            "Usage:\n  agenthub actor team-members [--team-id <team_id>] [--run-id <run_id>]\n"
                .to_string()
        }
        "team-tasks" => "Usage:\n  agenthub actor team-tasks [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>] [--status <all|open|in_progress|in_review|completed|canceled>] [--include-shared-thread]\n".to_string(),
        "team-task-create" => "Usage:\n  agenthub actor team-task-create --title <title> [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>] [--status <open|in_progress|in_review|completed|canceled>] [--topic <topic>] [--context-json <json>]\n".to_string(),
        "team-task-update" => "Usage:\n  agenthub actor team-task-update --task-id <task_id> [--status <open|in_progress|in_review|completed|canceled>] [--assigned-member-id <member_id> | --unassign] [--team-id <team_id>] [--actor-id <actor_id> | --agent-id <agent_id>]\n".to_string(),
        "time-trigger-set" => "Usage:\n  agenthub actor time-trigger-set --delay-seconds <seconds> --message <text> [--actor-id <actor_id> | --agent-id <agent_id>]\n".to_string(),
        "time-trigger-list" => "Usage:\n  agenthub actor time-trigger-list [--actor-id <actor_id> | --agent-id <agent_id>] [--limit <n>]\n".to_string(),
        "time-trigger-cancel" => "Usage:\n  agenthub actor time-trigger-cancel --trigger-id <trigger_id> [--actor-id <actor_id> | --agent-id <agent_id>]\n".to_string(),
        _ => actor_usage(),
    }
}
