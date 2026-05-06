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
const TEAM_COORDINATOR_SKILL_TEXT: &str =
    include_str!("../../../skills/team/team-coordinator-orchestrator.SKILL.md");
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
    TeamCoordinatorAgentsIndex,
    TeamWorkerAgentsIndex,
    TeamCoordinatorOrchestrator,
    TeamWorkerExecutor,
    TeamTaskLifecycle,
    TeamDeliberationRules,
    TeamActorMailbox,
    ActorRuntime,
}

impl ManagedSkillKind {
    pub const ALL: [Self; 9] = [
        Self::TeamAgentsIndex,
        Self::TeamCoordinatorAgentsIndex,
        Self::TeamWorkerAgentsIndex,
        Self::TeamCoordinatorOrchestrator,
        Self::TeamWorkerExecutor,
        Self::TeamTaskLifecycle,
        Self::TeamDeliberationRules,
        Self::TeamActorMailbox,
        Self::ActorRuntime,
    ];

    fn relative_dir(self) -> &'static str {
        match self {
            Self::TeamAgentsIndex => "team/team-agents-index",
            Self::TeamCoordinatorAgentsIndex => "team/team-coordinator-agents-index",
            Self::TeamWorkerAgentsIndex => "team/team-worker-agents-index",
            Self::TeamCoordinatorOrchestrator => "team/team-coordinator-orchestrator",
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
        ManagedSkillKind::TeamCoordinatorAgentsIndex => "team-coordinator-agents-index",
        ManagedSkillKind::TeamWorkerAgentsIndex => "team-worker-agents-index",
        ManagedSkillKind::TeamCoordinatorOrchestrator => "team-coordinator-orchestrator",
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
        ManagedSkillKind::TeamCoordinatorAgentsIndex => team_role_agents_index_skill_doc(true),
        ManagedSkillKind::TeamWorkerAgentsIndex => team_role_agents_index_skill_doc(false),
        ManagedSkillKind::TeamCoordinatorOrchestrator => TEAM_COORDINATOR_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamWorkerExecutor => TEAM_WORKER_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamTaskLifecycle => TEAM_TASK_LIFECYCLE_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamDeliberationRules => TEAM_DELIBERATION_SKILL_TEXT.to_string(),
        ManagedSkillKind::TeamActorMailbox => TEAM_ACTOR_MAILBOX_SKILL_TEXT.to_string(),
        ManagedSkillKind::ActorRuntime => actor_runtime_skill_doc(),
    }
}

fn team_role_agents_index_skill_doc(is_coordinator: bool) -> String {
    let (name, title, role_label, role_core_skill, memory_rule, responsibilities) =
        if is_coordinator {
            (
                "team-coordinator-agents-index",
                "Team Coordinator AGENTS Index",
                "coordinator",
                "team-coordinator-orchestrator",
                "Keep coordinator durable memory lightweight; empty coordination workspaces normally do not need `.agenthubmemory/`.",
                [
                    "Maintain coordinator workspace `AGENTS.md` as the coordination index.",
                    "Keep current phase, transition condition, assignment map, and integration checklist concise.",
                    "Keep human-facing planning decisions in coordinator index records.",
                    "Keep `team-task-lifecycle` active whenever coordinator is creating, reviewing, or closing canonical Team tasks.",
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
                    "Keep coordinator as the default single-owner update route, but use shared channel directly when important issues need team-wide discussion or visibility.",
                    "Keep `team-task-lifecycle` active whenever worker execution must advance a coordinator-owned Team task toward review.",
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
`current_run_id`, `actor_id`, `default_channel`, and
optional compact continuity summary for this specific session.
Treat deeper continuity detail as pointer-backed runtime state: inspect
persisted artifacts or replay state when you need more history, instead of
expecting large inline continuity payloads in the prompt itself.

Team mailbox commands:

1. Accept pending inbox work:
   `agenthub actor receive --run-id "<run-id>" --limit 20`
2. Inspect mailbox without mutating delivery state:
   `agenthub actor inbox --run-id "<run-id>" --limit 20 --include-delivered`
3. Send a local direct message:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "worker" --text "Please review this patch.\n\n- verify API shape\n- call out blockers"`
4. Send a channel message:
   `agenthub actor send --run-id "<run-id>" --channel-id "all" --text "@worker Please review this patch.\n\n- verify API shape\n- call out blockers"`
5. Open a thread for detailed context rooted in an existing channel message:
   `agenthub actor team-thread-open --run-id "<run-id>" --shared --root-message-id "<message-id>"`
6. Reply inside that thread when the topic needs logs, evidence, or detailed follow-up:
   `agenthub actor team-thread-reply --run-id "<run-id>" --shared --root-message-id "<message-id>" --text-file .agenthubmemory/mailbox/outbox/thread-reply.md`
7. Send a remote direct message:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "remote-worker" --transport remote --route-json '{"endpoint":"https://..."}' --text "Please review this patch.\n\n- verify API shape\n- call out blockers"`
8. Send an urgent human notification:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "user" --text "Urgent: permission review timed out. Please check Channel for details."`
9. Force duplicate delivery when business logic requires repeated send:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "worker" --allow-duplicate --text "Reminder:\n\n- update the test evidence\n- reply when done"`
10. Use explicit idempotency key when coordinating retries across workers:
   `agenthub actor send --run-id "<run-id>" --to-actor-id "worker" --idempotency-key "stable-key" --text "Reminder:\n\n- update the test evidence\n- reply when done"`

Team context commands:

11. Inspect live team runtime status, roster, identity-card descriptions, and optional run step overlay:
   `agenthub actor team-members`
12. When you need step-level overlay for a specific run:
   `agenthub actor team-members --run-id "<run-id>"`

Protocol rules:

- Always accept mailbox work before starting a new coordination step.
- In each turn, the first mailbox action should be `actor receive` before planning/coding.
- Treat `actor receive` as the normal accept-and-consume path for pending mailbox work.
- Treat `actor inbox` output as a read-only unread snapshot: it includes `pending_count` alongside the fetched messages.
- Mailbox nudges are token-efficient by default: only direct `agent -> agent` sends and coordinator-authored channel `@member_id` mentions trigger immediate ACP hints.
- Other unread mailbox traffic may surface later as one compact unread summary after roughly 3 minutes of ACP output silence; if unread count is `0`, no reminder is sent.
- Before routing work based on teammate assumptions, inspect `actor team-members`.
- Treat `actor team-members` as the single Team context snapshot command: it returns runtime summary, roster/card data, per-member `pending_inbox_count`, and optional run overlay.
- Treat the runtime context block as the canonical source for the current actor identity and default run scope.
- Use `actor inbox` only for inspection/debugging or historical mailbox review.
- Keep `actor ack` for repair, recovery, or manual compensation flows.
- When ACP approval offers the same least-privilege scope with different approval persistence
  options (for example, one-time vs reusable), choose the shortest duration that still avoids
  unnecessary repeated prompts.
- For frequently repeated trusted command families such as actor (`agenthub actor`), prefer a
  session-scoped reusable approval when available; otherwise choose the least broad reusable option
  offered so the session does not churn on identical prompts.
- For `actor permission-review-respond`, choose any allow/session/persistent approval by passing the
  concrete request-provided `--option-id`; do not invent `--outcome always`.
- `actor permission-review-respond --outcome` currently supports only `cancelled`.
- Keep payload JSON compact and deterministic.
- Prefer `actor send --text` for markdown-rich messages; it preserves formatting better than wrapping prose inside structured fields.
- For group chat / channel sends, use `channel_id`; the message will still fan out to all relevant teammates even when `@member_id` appears in the text.
- Treat `@member_id` in channel text as mention metadata for receivers, not as a routing override.
- Keep channel root messages summary-first. Use `team-thread-open` and `team-thread-reply` for the full context of one rooted topic.
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
        managed_skill_doc, managed_skill_doc_path, managed_skills_root,
    };

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_test_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
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
                assert!(!doc.contents.contains("`actor_cli_path`"));
                assert!(doc.contents.contains("`agenthub actor receive"));
                assert!(doc.contents.contains("compact continuity summary"));
                assert!(doc.contents.contains("persisted artifacts or replay state"));
            }
        }
    }

    #[test]
    fn worker_managed_skills_allow_shared_channel_discussion_for_important_matters() {
        let home = unique_test_temp_dir("agenthub-managed-skills-worker-route");

        let worker_index = managed_skill_doc(
            ManagedSkillKind::TeamWorkerAgentsIndex,
            Some(home.as_path()),
        )
        .expect("build worker index doc");
        assert!(worker_index.contents.contains("shared channel directly"));
        assert!(worker_index.contents.contains("important issues"));

        let worker_executor =
            managed_skill_doc(ManagedSkillKind::TeamWorkerExecutor, Some(home.as_path()))
                .expect("build worker executor doc");
        assert!(
            worker_executor
                .contents
                .contains("shared-channel discussion")
        );
        assert!(worker_executor.contents.contains("team-wide review"));
        assert!(worker_executor.contents.contains("explicitly `@member_id`"));
        assert!(
            worker_executor
                .contents
                .contains("important findings, risks, tradeoffs, or decisions")
        );
    }

    #[test]
    fn managed_skills_describe_channel_thread_context_split() {
        let home = unique_test_temp_dir("agenthub-managed-skills-channel-thread");

        let shared_index =
            managed_skill_doc(ManagedSkillKind::TeamAgentsIndex, Some(home.as_path()))
                .expect("build shared team index doc");
        assert!(
            shared_index
                .contents
                .contains("channel root messages are summary-first")
        );
        assert!(
            shared_index
                .contents
                .contains("thread replies are the full-context lane")
        );
        assert!(
            shared_index
                .contents
                .contains("agenthub actor team-thread-open")
        );
        assert!(
            shared_index
                .contents
                .contains("agenthub actor team-thread-reply")
        );

        let mailbox = managed_skill_doc(ManagedSkillKind::TeamActorMailbox, Some(home.as_path()))
            .expect("build mailbox doc");
        assert!(mailbox.contents.contains("team-thread-open"));
        assert!(mailbox.contents.contains("team-thread-reply"));
        assert!(
            mailbox
                .contents
                .contains("open the thread before treating the root message")
        );

        let runtime = managed_skill_doc(ManagedSkillKind::ActorRuntime, Some(home.as_path()))
            .expect("build actor runtime doc");
        assert!(runtime.contents.contains("agenthub actor team-thread-open"));
        assert!(
            runtime
                .contents
                .contains("agenthub actor team-thread-reply")
        );
        assert!(
            runtime
                .contents
                .contains("Keep channel root messages summary-first")
        );

        let coordinator = managed_skill_doc(
            ManagedSkillKind::TeamCoordinatorOrchestrator,
            Some(home.as_path()),
        )
        .expect("build coordinator doc");
        assert!(coordinator.contents.contains("thread-scoped deep context"));

        let worker = managed_skill_doc(ManagedSkillKind::TeamWorkerExecutor, Some(home.as_path()))
            .expect("build worker doc");
        assert!(
            worker
                .contents
                .contains("turning the root channel lane into a context dump")
        );
    }

    fn assert_frontmatter_has_name_and_description(doc: &ManagedSkillDoc) {
        assert!(
            doc.contents
                .starts_with(&format!("---\nname: {}\n", doc.name)),
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
