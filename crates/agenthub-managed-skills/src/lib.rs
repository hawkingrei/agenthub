use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

const AGENTS_SKILLS_DIR: &str = ".agents/skills";
const AGENTHUB_SKILLS_NAMESPACE: &str = "agenthub-runtime";
const SKILL_DOC_NAME: &str = "SKILL.md";

const TEAM_AGENTS_INDEX_TEXT: &str =
    include_str!("../../../skills/team/team-agents-index.SKILL.md");
const TEAM_LEADER_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-leader-orchestrator.SKILL.md");
const TEAM_WORKER_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-worker-executor.SKILL.md");
const TEAM_DELIBERATION_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-deliberation-rules.SKILL.md");
const TEAM_ACTOR_MAILBOX_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-actor-mailbox.SKILL.md");
const TEAM_TASK_LIFECYCLE_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-task-lifecycle.SKILL.md");

static MANAGED_SKILL_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagedSkillKind {
    TeamAgentsIndex,
    TeamLeaderAgentsIndex,
    TeamWorkerAgentsIndex,
    TeamLeaderOrchestrator,
    TeamWorkerExecutor,
    TeamTaskLifecycle,
    TeamDeliberationRules,
    TeamActorMailbox,
    ActorRuntime,
}

impl ManagedSkillKind {
    pub const ALL: [Self; 9] = [
        Self::TeamAgentsIndex,
        Self::TeamLeaderAgentsIndex,
        Self::TeamWorkerAgentsIndex,
        Self::TeamLeaderOrchestrator,
        Self::TeamWorkerExecutor,
        Self::TeamTaskLifecycle,
        Self::TeamDeliberationRules,
        Self::TeamActorMailbox,
        Self::ActorRuntime,
    ];

    fn relative_dir(self) -> &'static str {
        match self {
            Self::TeamAgentsIndex => "team/team-agents-index",
            Self::TeamLeaderAgentsIndex => "team/team-leader-agents-index",
            Self::TeamWorkerAgentsIndex => "team/team-worker-agents-index",
            Self::TeamLeaderOrchestrator => "team/team-leader-orchestrator",
            Self::TeamWorkerExecutor => "team/team-worker-executor",
            Self::TeamTaskLifecycle => "team/team-task-lifecycle",
            Self::TeamDeliberationRules => "team/team-deliberation-rules",
            Self::TeamActorMailbox => "team/team-actor-mailbox",
            Self::ActorRuntime => "runtime/actor-runtime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSkillDoc {
    pub kind: ManagedSkillKind,
    pub name: &'static str,
    pub path: PathBuf,
    pub contents: String,
}

pub fn managed_skills_root(home_dir: Option<&Path>) -> Result<PathBuf> {
    let home_dir = resolve_home_dir(home_dir)?;
    Ok(home_dir
        .join(AGENTS_SKILLS_DIR)
        .join(AGENTHUB_SKILLS_NAMESPACE))
}

pub fn managed_skill_doc_path(kind: ManagedSkillKind, home_dir: Option<&Path>) -> Result<PathBuf> {
    Ok(managed_skills_root(home_dir)?
        .join(kind.relative_dir())
        .join(SKILL_DOC_NAME))
}

pub fn managed_skill_doc(
    kind: ManagedSkillKind,
    home_dir: Option<&Path>,
) -> Result<ManagedSkillDoc> {
    Ok(ManagedSkillDoc {
        kind,
        name: managed_skill_name(kind),
        path: managed_skill_doc_path(kind, home_dir)?,
        contents: managed_skill_contents(kind),
    })
}

pub fn install_managed_skills(home_dir: Option<&Path>) -> Result<Vec<PathBuf>> {
    let Some(home_dir) = resolve_home_dir_optional(home_dir) else {
        return Ok(Vec::new());
    };
    let mut installed = Vec::with_capacity(ManagedSkillKind::ALL.len());
    for kind in ManagedSkillKind::ALL {
        let doc = managed_skill_doc(kind, Some(home_dir.as_path()))?;
        let parent = doc
            .path
            .parent()
            .context("managed skill document missing parent directory")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create managed skill directory {}", parent.display()))?;
        write_managed_skill_file(&doc.path, doc.contents.as_bytes())?;
        installed.push(doc.path);
    }
    Ok(installed)
}

fn write_managed_skill_file(path: &Path, contents: &[u8]) -> Result<()> {
    if std::fs::read(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }

    let parent = path
        .parent()
        .context("managed skill document missing parent directory")?;
    let (temp_path, mut file) = create_temp_skill_file(parent, path)?;
    {
        file.write_all(contents)
            .with_context(|| format!("write temp managed skill {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("sync temp managed skill {}", temp_path.display()))?;
    }

    match std::fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            #[cfg(windows)]
            {
                if path.exists() {
                    std::fs::remove_file(path)
                        .with_context(|| format!("remove managed skill {}", path.display()))?;
                    std::fs::rename(&temp_path, path).with_context(|| {
                        format!(
                            "replace managed skill {} from {}",
                            path.display(),
                            temp_path.display()
                        )
                    })?;
                    return Ok(());
                }
            }

            let _ = std::fs::remove_file(&temp_path);
            Err(rename_err).with_context(|| {
                format!(
                    "replace managed skill {} from {}",
                    path.display(),
                    temp_path.display()
                )
            })
        }
    }
}

fn unique_temp_path(parent: &Path, target_path: &Path) -> PathBuf {
    let basename = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(SKILL_DOC_NAME);
    let counter = MANAGED_SKILL_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    parent.join(format!(
        ".{basename}.{}.{}.{}.tmp",
        std::process::id(),
        nonce,
        counter
    ))
}

pub fn resolve_home_dir(home_dir: Option<&Path>) -> Result<PathBuf> {
    let Some(home_dir) = resolve_home_dir_optional(home_dir) else {
        bail!("HOME is not set; unable to resolve ~/.agents/skills");
    };
    Ok(home_dir)
}

fn resolve_home_dir_optional(home_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(home_dir) = home_dir {
        return Some(home_dir.to_path_buf());
    }

    #[cfg(windows)]
    {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }

    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn create_temp_skill_file(parent: &Path, target_path: &Path) -> Result<(PathBuf, std::fs::File)> {
    for _ in 0..8 {
        let temp_path = unique_temp_path(parent, target_path);
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create temp managed skill {}", temp_path.display()));
            }
        }
    }

    bail!(
        "failed to allocate temp managed skill path for {}",
        target_path.display()
    );
}

pub fn managed_skill_name(kind: ManagedSkillKind) -> &'static str {
    match kind {
        ManagedSkillKind::TeamAgentsIndex => "team-agents-index",
        ManagedSkillKind::TeamLeaderAgentsIndex => "team-leader-agents-index",
        ManagedSkillKind::TeamWorkerAgentsIndex => "team-worker-agents-index",
        ManagedSkillKind::TeamLeaderOrchestrator => "team-leader-orchestrator",
        ManagedSkillKind::TeamWorkerExecutor => "team-worker-executor",
        ManagedSkillKind::TeamTaskLifecycle => "team-task-lifecycle",
        ManagedSkillKind::TeamDeliberationRules => "team-deliberation-rules",
        ManagedSkillKind::TeamActorMailbox => "team-actor-mailbox",
        ManagedSkillKind::ActorRuntime => "agenthub-actor-runtime",
    }
}

pub fn managed_skill_contents(kind: ManagedSkillKind) -> String {
    match kind {
        ManagedSkillKind::TeamAgentsIndex => TEAM_AGENTS_INDEX_TEXT.to_string(),
        ManagedSkillKind::TeamLeaderAgentsIndex => team_role_agents_index_skill_doc(true),
        ManagedSkillKind::TeamWorkerAgentsIndex => team_role_agents_index_skill_doc(false),
        ManagedSkillKind::TeamLeaderOrchestrator => TEAM_LEADER_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamWorkerExecutor => TEAM_WORKER_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamTaskLifecycle => TEAM_TASK_LIFECYCLE_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamDeliberationRules => TEAM_DELIBERATION_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamActorMailbox => TEAM_ACTOR_MAILBOX_SKILL_TEXT.to_string(),
        ManagedSkillKind::ActorRuntime => actor_runtime_skill_doc(),
    }
}

fn team_role_agents_index_skill_doc(is_leader: bool) -> String {
    let (name, title, role_label, role_core_skill, memory_rule, responsibilities) = if is_leader {
        (
            "team-leader-agents-index",
            "Team Leader AGENTS Index",
            "leader",
            "team-leader-orchestrator",
            "Keep leader durable memory lightweight; empty coordination workspaces normally do not need `.agenthubmemory/`.",
            [
                "Maintain leader workspace `AGENTS.md` as the coordination index.",
                "Keep current phase, transition condition, assignment map, and integration checklist concise.",
                "Keep human-facing planning decisions in leader index records.",
                "Keep `team-task-lifecycle` active whenever leader is creating, reviewing, or closing canonical Team tasks.",
            ],
        )
    } else {
        (
            "team-worker-agents-index",
            "Team Worker AGENTS Index",
            "worker",
            "team-worker-executor",
            "Maintain project-local durable memory under `.agenthubmemory/` when operating inside a concrete repository.",
            [
                "Maintain worker workspace `AGENTS.md` as the execution index.",
                "Keep assignment scope, acceptance criteria, evidence pointers, and blockers concise.",
                "Keep worker updates routed to leader unless explicit escalation policy applies.",
                "Keep `team-task-lifecycle` active whenever worker execution must advance a leader-owned Team task toward review.",
            ],
        )
    };
    let responsibilities = responsibilities
        .iter()
        .map(|line| format!("- {line}\n"))
        .collect::<String>();
    format!(
        r#"---
name: {name}
description: {role_label} runtime AGENTS index for AgentHub Team sessions.
---

# {title}

Use this skill as the {role_label}-specific AGENTS index initializer.

Primary references:

- Shared baseline: `skills/team/AGENTS.md`
- Unified runtime template: `skills/team/TEAM_AGENTS.md`

## Responsibilities

{responsibilities}- {memory_rule}
- Keep the active skill set minimal and phase-scoped.

## Startup Checklist

1. Read shared baseline (`skills/team/AGENTS.md`).
2. Initialize or refresh workspace `AGENTS.md` from `skills/team/TEAM_AGENTS.md`.
3. Set `role={role_label}` and keep `Active Skills` minimal:
   - `{role_core_skill}` (role execution skill)
   - `team-actor-mailbox`
   - add `team-task-lifecycle` only when canonical Team task state must change
   - add `team-deliberation-rules` only when option comparison or consensus work is active
4. Check `TODO.md` before mailbox rounds.
"#,
        name = name,
        title = title,
        role_label = role_label,
        responsibilities = responsibilities,
        memory_rule = memory_rule,
        role_core_skill = role_core_skill,
    )
}

fn actor_runtime_skill_doc() -> String {
    r#"---
name: agenthub-actor-runtime
description: Runtime coordination contract for AgentHub actor sessions.
---

# AgentHub Actor Runtime Skill

You are running inside an AgentHub actor session.

Use this skill together with the runtime context block that AgentHub injects
before each prompt. The context block carries the current `team_id`,
`current_run_id`, `actor_id`, `default_channel`, `actor_cli_path`, and
optional continuity summary for this specific session.

Use the concrete `actor_cli_path` from the runtime context block for runtime
coordination.

Team mailbox commands:

1. Pull inbox:
   `<actor_cli_path> actor inbox --run-id "<run-id>" --limit 20`
2. Acknowledge a message after processing:
   `<actor_cli_path> actor ack --run-id "<run-id>" --message-id 123`
3. Send a local direct message:
   `<actor_cli_path> actor send --run-id "<run-id>" --to-actor-id "worker" --text "Please review this patch.\n\n- verify API shape\n- call out blockers"`
4. Send a channel message:
   `<actor_cli_path> actor send --run-id "<run-id>" --channel-id "all" --text "@worker Please review this patch.\n\n- verify API shape\n- call out blockers"`
5. Send a remote direct message:
   `<actor_cli_path> actor send --run-id "<run-id>" --to-actor-id "remote-worker" --transport remote --route-json '{"endpoint":"https://..."}' --text "Please review this patch.\n\n- verify API shape\n- call out blockers"`
6. Send an urgent human notification:
   `<actor_cli_path> actor send --run-id "<run-id>" --to-actor-id "user" --text "Urgent: permission review timed out. Please check Channel for details."`
7. Force duplicate delivery when business logic requires repeated send:
   `<actor_cli_path> actor send --run-id "<run-id>" --to-actor-id "worker" --allow-duplicate --text "Reminder:\n\n- update the test evidence\n- reply when done"`
8. Use explicit idempotency key when coordinating retries across workers:
   `<actor_cli_path> actor send --run-id "<run-id>" --to-actor-id "worker" --idempotency-key "stable-key" --text "Reminder:\n\n- update the test evidence\n- reply when done"`

Team context commands:

9. Inspect live team runtime status, roster, identity-card descriptions, and optional run step overlay:
   `<actor_cli_path> actor team-members`
10. When you need step-level overlay for a specific run:
   `<actor_cli_path> actor team-members --run-id "<run-id>"`

Protocol rules:

- Always pull inbox before starting a new coordination step.
- In each turn, the first mailbox action must be `actor inbox` before planning/coding.
- Treat `actor inbox` output as a live unread snapshot: it now includes `pending_count` alongside the fetched messages.
- Mailbox nudges are token-efficient by default: only direct `agent -> agent` sends and leader-authored channel `@member_id` mentions trigger immediate ACP hints.
- Other unread mailbox traffic may surface later as one compact unread summary after roughly 3 minutes of ACP output silence; if unread count is `0`, no reminder is sent.
- Before routing work based on teammate assumptions, inspect `actor team-members`.
- Treat `actor team-members` as the single Team context snapshot command: it returns runtime summary, roster/card data, per-member `pending_inbox_count`, and optional run overlay.
- Treat the runtime context block as the canonical source for the current actor identity and default run scope.
- If inbox has pending items, process and `actor ack` them before emitting final result.
- Acknowledge each consumed message exactly once.
- Keep payload JSON compact and deterministic.
- Prefer `actor send --text` for markdown-rich messages; it preserves formatting better than wrapping prose inside structured fields.
- For group chat / channel sends, use `channel_id`; the message will still fan out to all relevant teammates even when `@member_id` appears in the text.
- Treat `@member_id` in channel text as mention metadata for receivers, not as a routing override.
- Use `to_actor_id = "user"` or `user:<id>` only when you intentionally want a human notification.
- Use `channel` only when a non-default channel is required.
- By default, `actor send` auto-generates an idempotency key from message fields to prevent duplicate delivery on retries.
- Reuse the same payload and routing fields when retrying; changing payload under the same idempotency key will be rejected.
- Use `allow_duplicate=true` only when you intentionally need repeated delivery of equivalent payloads.
- Use `payload` only when the receiver genuinely needs machine-readable fields such as `status`, `evidence`, or workflow metadata.
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::{
        ManagedSkillDoc, ManagedSkillKind, SKILL_DOC_NAME, install_managed_skills,
        managed_skill_doc,
        managed_skill_doc_path, managed_skills_root,
    };

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn managed_skill_paths_live_under_agents_skills_namespace() {
        let home = std::env::temp_dir().join("agenthub-managed-skills-home");
        let root = managed_skills_root(Some(home.as_path())).expect("resolve managed root");
        assert!(root.ends_with(".agents/skills/agenthub-runtime"));

        let path = managed_skill_doc_path(ManagedSkillKind::TeamActorMailbox, Some(home.as_path()))
            .expect("resolve managed skill path");
        assert!(path.ends_with(format!(
            ".agents/skills/agenthub-runtime/team/team-actor-mailbox/{SKILL_DOC_NAME}"
        )));
    }

    #[test]
    fn managed_skill_docs_include_expected_frontmatter() {
        let home = std::env::temp_dir().join("agenthub-managed-skills-doc-home");
        for kind in ManagedSkillKind::ALL {
            let doc =
                managed_skill_doc(kind, Some(home.as_path())).expect("build managed skill doc");
            assert_frontmatter_has_name_and_description(&doc);
            if kind == ManagedSkillKind::ActorRuntime {
                assert!(doc.contents.contains("Runtime coordination contract"));
                assert!(doc.contents.contains("`actor_cli_path`"));
                assert!(doc.contents.contains("<actor_cli_path> actor inbox"));
            }
        }
    }

    fn assert_frontmatter_has_name_and_description(doc: &ManagedSkillDoc) {
        assert!(
            doc.contents.starts_with(&format!("---\nname: {}\n", doc.name)),
            "managed skill '{}' is missing name front matter",
            doc.name
        );
        let frontmatter_end = doc
            .contents
            .find("\n---\n")
            .expect("managed skill front matter terminator");
        let frontmatter = &doc.contents[..frontmatter_end];
        assert!(
            frontmatter.contains("\ndescription: "),
            "managed skill '{}' is missing description front matter",
            doc.name
        );
    }

    #[test]
    fn install_managed_skills_writes_all_skill_documents() {
        let root = std::env::temp_dir().join(format!(
            "agenthub-managed-skills-install-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        let installed =
            install_managed_skills(Some(root.as_path())).expect("install managed skills");
        assert_eq!(installed.len(), ManagedSkillKind::ALL.len());
        for path in installed {
            assert!(path.exists(), "expected {} to exist", path.display());
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(not(windows))]
    #[test]
    fn install_managed_skills_without_home_dir_is_noop() {
        let _guard = env_lock().lock().expect("lock env");
        let previous_home = std::env::var_os("HOME");
        // SAFETY: the test serializes environment mutation through `env_lock`.
        unsafe {
            std::env::remove_var("HOME");
        }

        let installed = install_managed_skills(None).expect("skip install without home");
        assert!(installed.is_empty());

        // SAFETY: the test serializes environment mutation through `env_lock`.
        unsafe {
            if let Some(home) = previous_home {
                std::env::set_var("HOME", home);
            }
        }
    }
}
