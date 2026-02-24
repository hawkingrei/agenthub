# Team Runtime Role Skill And Actor Identity Spec

## Problem

Team execution depends on role-aware instructions (`leader`, `worker`, deliberation rules), but behavior drift appears when:
- runtime skill injection is inconsistent across Team vs single-mode sessions
- skill bootstrap paths are outside ACP `safe_paths`
- cold-start and TODO lifecycle rules are not explicit or are duplicated across docs
- actor runtime naming at tool boundaries (`actor_id`) diverges from operator mental model (`agent_id`)
- mailbox payloads do not explicitly distinguish `human` and `agent` identity kinds

This spec defines one stable technical contract for Team role skills, actor identity aliasing, and mailbox identity-kind projection.

## Scope

- Team role skill set and injection boundaries
- single-mode isolation behavior
- skill bootstrap/install policy for local single-node environments
- cold-start and per-agent TODO lifecycle guidance in role skills
- AGENTS index vs SKILL detail ownership boundary
- actor runtime CLI/MCP alias contract (`agent_id` aliases)
- mailbox identity-kind fields (`from_actor_kind`, `to_actor_kind`)

## Non-Goals

- backend enforcement of TODO file existence/content
- replacing Team mailbox/task orchestration protocols
- changing Team role skill IDs
- removing run-scoped mailbox partition semantics (`run_id`)
- replacing storage schema `actor_id` with `agent_id`

## Architecture

### 1) Skill Sources

- Built-in runtime skill:
  - `agenthub-actor-runtime`
  - source: `crates/agenthub-acp/src/actor_runtime_skill.rs`
- Built-in Team role skills:
  - `team-leader-orchestrator`
  - `team-worker-executor`
  - `team-deliberation-rules`
  - source loader: `crates/agenthub-acp/src/team_role_skills.rs`
- Optional global skills:
  - source: `~/.agenthub/skills.json`
  - loader: `crates/agenthub-acp/src/lib.rs`

### 2) Injection Pipeline

ACP session startup behavior:
1. Load MCP servers and global skills.
2. Remove reserved Team role skills from global list.
3. If actor context carries Team role (`leader` or `worker`), inject built-in Team role skills for that role.
4. Inject actor runtime skill.
5. De-duplicate by skill name/path.
6. Attach skill blocks to every prompt request.

Primary files:
- `crates/agenthub-acp/src/lib.rs`
- `crates/agenthub-acp/src/team_role_skills.rs`
- `crates/agenthub-acp-core/src/lib.rs`

### 3) Single-Mode Isolation

- Team role skills are reserved runtime-only skills.
- Single-mode/manual ACP sessions must not inherit Team role skills from global `skills.json`.
- Team role skill injection only occurs when actor context explicitly carries supported role metadata.

### 4) Bootstrap and Install Path Policy

- Canonical bootstrap script:
  - `scripts/setup_team_skills.sh`
- Default behavior:
  - copy Team skills into `~/.agenthub/worktrees/team-skills`
  - write resulting paths into `~/.agenthub/skills.json`
- Rationale:
  - default path is typically within configured ACP safe paths, reducing silent skill skips
- Compatibility switch:
  - `--use-repo-skill-paths` (no copy, direct repository paths)

### 5) Role Guidance Contracts

Role skills must provide:
- six-phase collaboration model
  - `team formation`
  - `task analysis`
  - `role assignment`
  - `communication and collaboration`
  - `consensus formation`
  - `result integration`
- cold-start TODO-first checks:
  - `TODO.md`
  - `.cache/context/todo.md`
- per-agent TODO lifecycle:
  - states: `pending`, `in_progress`, `completed`, `blocked`
  - exactly one `in_progress` per agent at a time
  - `completed` only after acceptance evidence
  - stale TODO entries should be compacted to keep one authoritative active item
- leader communication boundary:
  - leader answers human planning questions directly (no worker redirection)
- leader planning quality gate:
  - `Decision Complete`: delegation plan leaves zero implementation judgment calls
  - `Explore Before Asking`: discoverable repo/system facts are explored before user questions
  - unknowns are split into discoverable facts vs preference/tradeoff decisions
  - delegation starts only after a checklist confirms objective, scope boundaries, approach, acceptance criteria, test strategy, and risk/rollback notes
- routing key policy:
  - teammate routing should use stable `spec.members[].member_id`
  - avoid opaque runtime UUID/process identifiers in coordination artifacts
- finalization policy by mode:
  - persistent team mode keeps members alive after response
  - one-shot/non-interactive mode requires graceful member shutdown + cleanup before final response

### 6) Role Responsibility Contract

- Leader role:
  - acts as architect/reviewer/synthesizer
  - owns decomposition, assignment, acceptance criteria, risk decisions, and final integration
  - should not implement feature code directly by default
  - may apply minimal emergency fixes only when worker execution is blocked or explicitly requested by human
- Worker role:
  - acts as implementation executor
  - owns coding/testing/evidence delivery for delegated tasks
  - routes planning decisions back through leader

### 7) AGENTS vs SKILL Ownership

- `AGENTS.md` is the index/routing artifact:
  - objective
  - current phase
  - active skill pointers
  - progress index
- detailed procedures must live in `SKILL.md` files.

### 8) Actor Namespace And Identity Layers

- Namespace key (mailbox partition): `run_id` (required)
- Actor identifier (sender/receiver): `actor_id` with `agent_id` aliases at runtime tool boundary
- Identity kind projection for UX/policy: `agent | human`

This separation keeps run isolation/replay deterministic while aligning operator terminology.

### 9) Actor CLI/MCP Alias Surface

Runtime tools accept additive `agent_id` aliases while preserving existing `actor_id` flags:

- `agenthub actor inbox`
  - `--agent-id` alias for `--actor-id`
- `agenthub actor ack`
  - `--agent-id` alias for `--actor-id`
- `agenthub actor send`
  - `--from-agent-id` alias for `--from-actor-id`
  - `--to-agent-id` alias for `--to-actor-id`
- `agenthub actor-mcp`
  - `--agent-id` alias for `--actor-id`

Environment alias:
- `AGENTHUB_ACTOR_AGENT_ID` (alongside `AGENTHUB_ACTOR_ID`)

## Contracts

### Role Skill IDs (stable)

- `team-leader-orchestrator`
- `team-worker-executor`
- `team-deliberation-rules`

### Profile Patch Surface

Runtime customization can append prompt/skills per member via profile patch proposal:
- `prompt_append`
- `skills_add`

Primary file:
- `src/api/teams.rs`

### Actor Runtime Alias Contract

- `actor_id` inputs remain valid for backward compatibility.
- `agent_id` flags/env are additive aliases at CLI/MCP boundaries only.
- `run_id` remains required for inbox/ack/send partitioning and idempotency.

Primary files:
- `src/actor_cli.rs`
- `src/actor_mcp.rs`

### Mailbox Identity Kind Contract

Mailbox message payload includes:
- `from_actor_kind: "agent" | "human"`
- `to_actor_kind: "agent" | "human"`

Kind is inferred by backend from actor identity conventions (`user` / `human` aliases map to `human`).

Primary files:
- `crates/agenthub-team-actor/src/message.rs`
- `src/team/manager/codec.rs`
- `src/team/manager/mailbox.rs`
- `src/api/openapi/spec.rs`
- `web/src/api.ts`

## Validation Matrix

Minimum checks for this feature area:
- Rust:
  - `cargo test -p agenthub-acp team_role_skills`
  - `cargo test -p agenthub-acp actor_runtime_skill`
  - `cargo test parse_inbox_accepts_agent_id_alias_flag`
  - `cargo test parse_inbox_uses_agent_id_env_fallback`
  - `cargo test parse_send_accepts_agent_id_alias_flags`
  - `cargo test parse_actor_mcp_context_uses_agent_id_env_alias`
  - `cargo test parse_actor_mcp_context_accepts_agent_id_flag`
  - `cargo test -p agenthub-team-actor`
  - `cargo test openapi -- --nocapture`
- Team prompt defaults:
  - `cargo test -p agenthub-team-prompts`
- Web defaults/build:
  - `pnpm -C web run build`
- Bootstrap script:
  - `bash -n scripts/setup_team_skills.sh`
  - dry-run against temp `skills.json`

## Operational Notes

- Existing teams with persisted custom prompts may not fully reflect new guidance until prompt/skill refresh.
- Team role instructions are runtime-injected for Team actor sessions; this limits config drift from global `skills.json`.
- If a skill file path is outside ACP safe paths, that skill is skipped by runtime loader.
- Operator-facing guidance should prefer `agent_id` naming, but runtime diagnostics should continue to display canonical partition key `run_id`.

## Open Risks

- TODO lifecycle is guidance-level today; runtime does not enforce state transitions.
- Operators may still override skill paths into disallowed locations and observe partial injection.
- Multiple historical notes can reintroduce drift unless compaction policy is followed.
- Human identity-kind inference is convention-based; stricter typed identity may be needed for multi-tenant policy hardening.

## Superseded Notes

This spec consolidates and supersedes:
- `docs/features/2026-02-17-team-skills-bootstrap-script.md`
- `docs/features/2026-02-18-team-deliberation-rules-skill.md`
- `docs/features/2026-02-19-team-role-skill-acp-auto-injection.md`
- `docs/features/2026-02-20-team-single-node-skill-bootstrap.md`
- `docs/features/2026-02-22-team-role-skill-single-mode-isolation.md`
- `docs/features/2026-02-23-team-cold-start-skill-and-ui-playbook.md`
- `docs/features/2026-02-24-actor-agent-id-alias.md`
