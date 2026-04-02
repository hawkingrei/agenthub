# AgentHub Project Charter

This document records AgentHub goals, scope, architecture decisions, and development conventions as the baseline for future implementation and evolution.

## 1. Project Goals

AgentHub is a tool for remotely controlling AI Agents. It supports starting, managing, and interacting with agents in a web UI. Agents run in a specified path; the web UI can see output and interact. Agents can push messages after finishing. Unless the user closes a session, the agent must remain alive even if the web page is closed.

## 2. Scope and MVP Features

- Agent lifecycle management: create, start, stop, reconnect, destroy
- Real-time output and interaction: HTTP polling or SSE by default; WebSocket as an optional enhancement
- Admin console: agent list, status, logs, session details
- Authentication and security: username/password login, join/bootstrap flow, basic access control
- Persistence: SQLite stores sessions, agent configuration, and audit records
- Notifications: in-app notification when an agent completes (extendable to webhook/email)
  - Use browser Push API (extend to Webhook later)

## 3. Technical and Architecture Constraints

- Backend: Rust (single-process service)
- Build system: Bazel is a first-class build/test path; Rust changes should keep `bazel build //...` and `bazel test //...` viable
- Rust packaging policy: prefer workspace crates under `crates/` with clear functional ownership; avoid unrelated "grab-bag" crates
- Frontend: mainstream TS framework (default React + Vite SPA), static assets embedded in the Rust service
- Frontend UI standard: all new UI features and UI refactors must use the project UI library (`@mantine/core`) plus Tailwind CSS utility classes as the default styling path
- Frontend product standard: user-facing UI should be modern, simple, clean, easy to use, and designed for customization and long-term extensibility instead of one-off page-specific styling
- Frontend internationalization standard: user-facing surfaces should be designed for multilingual support from the start; avoid hard-coded single-language assumptions in layout, copy structure, and component APIs
- CSS guardrail: do not introduce new large handcrafted global CSS blocks; keep legacy `web/src/styles.css` changes limited to compatibility fixes during migration
- Maintainability policy: low-risk maintainability review suggestions should be implemented directly in the active change instead of being deferred by default
- Agent context/file-boundary policy: there is no hard single-file LOC cap, but code should be organized for token-efficient navigation; prefer cohesive files plus thin routing/index files, and split files when mixed responsibilities or sustained work would require reading large unrelated sections (rough warning threshold: ~800-1000 LOC or multi-domain ownership)
- Development policy: AgentHub follows TDD / test-first for non-trivial changes; add or update focused tests before or alongside implementation, and treat missing regression coverage as unfinished work
- Database: SQLite
- Deployment: single binary, no separate frontend deployment
- Agent execution: spawn subprocess under the user-specified path; closing the page must not stop the agent

## 4. Key Architecture Decisions

1) Frontend is a static SPA build (Vite), served by Rust as static files and API.
2) Agent output defaults to non-WS transport; WS is an optional enhancement (future bash sandbox streaming).
3) Agent lifecycle is managed by the backend process; sessions and runtime state are persisted to SQLite.
4) Login uses username/password with token-based auth; join/bootstrap remains available for initial setup flows.
5) ACP (Agent Control Protocol) renders structured agent output; history must be retained.
6) Rust code should be decomposed by domain into library crates (`crates/<domain>`), not by arbitrary file split.
7) `cmd/agenthub/src/main.rs` should stay a thin bootstrap entry; business logic should live in library crates and be composed in `src/app.rs` plus domain crates.
8) New Rust domain modules should define Bazel targets with boundaries aligned to crate boundaries (one domain crate, one Bazel package as default).

## 5. Directory Plan (adjustable)

```
agenthub/
  cmd/
    agenthub/
      Cargo.toml
      src/
        main.rs          # binary bootstrap only
  crates/
    acp/
    openapi/
    ...                 # domain-oriented libraries
  src/
    app.rs              # bootstrap composition
    actor_cli.rs        # CLI entry library module
    lib.rs              # library entry point
    api/
    agent/
    auth/
    db/
    ws/
  web/
    package.json
    src/
    dist/                # build output
  migrations/
  .info/               # local reference materials (papers, notes, external repos), ignored by git
  AGENTS.md
```

## 6. Security and Reliability Principles

- Default least privilege: agent can only access the specified path
- Strict input validation: validate all API parameters
- Session persistence: reconnectable, not auto-closed
- Audit logging: write key actions to database logs
  - device login audit, device revocation, path deletion must be recorded

## 7. Testing and Validation (initial suggestions)

- Test-first expectation: non-trivial behavior changes should start from a focused failing or regression test when practical, and the test should remain part of the merged change
- Username/password login and join/bootstrap flows
- ACP rendering and history replay
- WS reconnect and message integrity (optional)
- Long-running agents and resource cleanup
- SQLite transaction consistency and concurrent access

## 8. Future Extensions

- Notification channels: Web Push / Webhook / Email
- Multi-user / multi-tenant
- Agent plugins and execution sandbox
  - Bash sandbox streaming (enable WS)

## 9. Requirement Additions (latest context)

- Agents page:
  - Top form creates tasks; supports selecting workdir and worktree strategy
  - Below shows running and historical task cards
  - Cards provide "View execution" using ACP rendering (similar to Xcode run view)
- Admin config:
  - Per-agent "code mode" toggle
- Join/login:
  - Login requires username + password only; Display Name used only for registration/bootstrap
- Configuration:
  - Use a config file instead of environment variables
- ACP:
  - History must be retained and replayable
- Frontend implementation policy:
  - Follow-up UI work should be built with UI library components + Tailwind CSS utilities
  - UI shells and interaction patterns should favor compact, content-first layouts with restrained chrome so primary content keeps the largest share of the viewport
  - New frontend work should preserve straightforward theming/customization hooks and avoid hard-wiring page-specific visual decisions into low-level shared components
  - User-facing copy and component contracts should remain localization-ready: prefer reusable text keys/centralized copy paths and layouts that tolerate longer translated strings
  - Avoid introducing new parallel style systems or expanding legacy handcrafted global CSS except compatibility patches
  - All frontend changes must be validated with Chrome DevTools MCP before edits (baseline) and after edits (regression check), and responses should include the MCP verification result summary
- Team role workflow policy:
  - Leader acts as architect/reviewer and should not implement feature code directly
  - Leader owns technical research and option comparison (assumptions, trade-offs, risks) before delegation
  - `AGENTS.md` is the role-level index and routing table; detailed execution procedures live in role/feature `SKILL.md` files
  - Team workflow must be indexed in `AGENTS.md` and routed to concrete skills:
    - shared Team baseline (injected to both leader/worker at startup): `skills/team/AGENTS.md` -> skill `team-agents-index`
    - unified Team runtime template (used by both roles): `skills/team/TEAM_AGENTS.md`
    - leader role index (builtin generated and injected into runtime AGENTS): skill `team-leader-agents-index`
    - worker role index (builtin generated and injected into runtime AGENTS): skill `team-worker-agents-index`
    - phase planning/orchestration: `skills/team/team-leader-orchestrator.SKILL.md`
    - execution delivery: `skills/team/team-worker-executor.SKILL.md`
    - deliberation quality gate: `skills/team/team-deliberation-rules.SKILL.md`
    - actor mailbox protocol (`inbox`/`send`/`ack`): `skills/team/team-actor-mailbox.SKILL.md`
  - At role startup, agent should read `AGENTS.md` first, then load only the skills required for current phase/task
  - Leader runtime starts from an empty workspace and should maintain coordination context in `AGENTS.md`
  - Leader code review path should prefer `gh` (or explicit clone-only review workspaces)
  - Team collaboration follows six explicit phases: `team formation` -> `task analysis` -> `role assignment` -> `communication and collaboration` -> `consensus formation` -> `result integration`
  - Human inputs are goals/constraints via conversation; internal Team `task` objects are created by leader planning, not directly by human users
  - Team conversation delivery path: persist the message once in conversation, then forward via Team actor mailbox; mailbox transport reaches ACP upstream and MCP tools.
  - Team communication lane should use event bus as the realtime carrier for chat/timeline fan-out; mailbox remains the authoritative execution command path.
  - User-facing conversation input should not require user-supplied `run_id` or `from_actor_id`; backend must derive sender identity from session and resolve routing from `@member_id`.
  - `conversation_id` is the required human-facing scope key; `run_id` is execution-scoped and should be generated when execution starts.
  - `correlation_id` should link one intent chain across conversation events, mailbox commands, and run events.
  - Team role sessions must receive the canonical actor CLI capability and actor runtime env so `agenthub actor ...` mailbox/task coordination is available from the first turn.
  - Team mode should keep communication on the canonical actor CLI path (`inbox -> process -> ack -> send/report`) instead of ad-hoc shell text routing or MCP mailbox injection.
  - Mention routing contract: `@member_id` means explicit recipients only; no `@` means broadcast to all team members.
  - Mailbox address translation: during mailbox fan-out, recipient `to_actor_id` should be translated into `@member_id` mention context for agent-facing chat payloads.
  - Reply contract: agent replies in team conversation should include `@member_id` when targeting specific recipients; replies without `@` are treated as broadcast.
  - Human-facing team conversation replies must contain final reply content only; do not expose mailbox transport status, `current_phase`, or raw JSON envelope fields in visible chat text.
  - In shared group chat, workers may reply to human-visible conversation directly for implementation progress, facts, and scoped answers; leader remains owner of planning decisions and final synthesis.
  - Leader should keep the current phase and phase-transition condition in `AGENTS.md`
  - Leader must answer human planning questions directly and should not redirect human users to worker agents
  - On cold start, leader should check unfinished items in `TODO.md`; workers in concrete project workspaces should also check `.agenthubmemory/TODO.md` before new mailbox work
  - TODO lifecycle details (when to create, status transitions, completion guardrails) belong to role skills; `AGENTS.md` only keeps pointers and current progress index
  - Backend runtime enforces leader starts with `worktree_mode=use_existing` and an empty workspace
  - Each worker must execute in its own git worktree with a random feature branch, and periodically sync from `main`
  - Backend runtime enforces worker starts with `worktree_mode=create_worktree`, per-run isolated workdir, and random branch checkout
  - Workers may coordinate with peers when dependencies overlap, but status/evidence must still flow back to leader
  - Each agent (leader/worker) owns and updates its context state only in its own workspace-local `.cache/context` tree
  - Leader should use an empty workspace dedicated to context management and coordination artifacts (avoid feature-code edits in leader workspace)
  - `spec.members[].description` is the canonical member identity description and must map to `/api/agents/:id/.well-known/agent-card` response `description`
  - `AGENTS.md` should include identity-card ownership/update pointers; detailed identity update workflow stays in team role skills
- Context management policy (OpenClaw-inspired, AgentHub-adapted):
  - Team prompt assembly must separate a stable prefix from a dynamic tail.
  - Stable prefix should include role charter, tool schemas, and safety guardrails, and must avoid non-deterministic fields (timestamps, random IDs, unstable key ordering).
  - Dynamic tail should include current goal, next action, allowed-action gate, compact state summary, evidence pointers, and recent error notes.
  - Context memory is workspace-scoped: each agent writes under `<agent_workspace>/.cache/context/...` and must not write into another agent workspace.
  - Large observations must be offloaded to filesystem memory under `<agent_workspace>/.cache/context/run/<run_id>/...`; prompt context should keep short summaries plus file pointers.
  - Context artifacts (`decisions`, `errors`, `log`) should be append-only to preserve recovery trails.
  - Before context compaction, runtime should trigger a pre-compaction memory flush attempt and persist explicit flush outcome (`persisted` or `noop`) for auditability.
  - Worker/sub-agent runs should default to minimal prompt mode by omitting nonessential sections to control token budget.
  - Tool constraints should prefer an appended `Allowed actions` policy block over runtime mutation of tool schemas.
  - Context and memory sections in `AGENTS.md` should stay concise as index pointers; append implementation details to dedicated skills/docs.
  - Repository file boundaries should help agents load small relevant slices: prefer thin index/routing files plus cohesive implementation files over large grab-bag modules or many tiny files without clear ownership.
  - Team runtime coordination details live in `docs/features/actor-foundation.md` and the Team mailbox/role skills; `docs/features/team-mcp-enforcement.md` is historical background only.
- Rust crate decomposition policy (Bazel-oriented):
  - Prefer extracting domain libraries into `crates/<domain>` and keep crate APIs cohesive and stable
  - Do not split into tiny crates without clear domain boundaries or ownership
  - Cross-crate dependencies should point towards core domain crates (Dependency Inversion) and avoid cycles
  - For each new crate, add/adjust corresponding Bazel package and targets in the same change

## 10. TODO

- ACP stdio client: integrate with agenthub-codex-acp and improve permission UX
- ACP HTTP3 gateway (public endpoint)
- ACP permission UX optimization: modal confirmation
- ACP permission event push: WebSocket instead of polling
- Bazel-first Rust decomposition rollout: migrate legacy `src/*` domain modules into cohesive `crates/*` libraries with aligned Bazel package boundaries
- Worktree strategy implementation and UI
- Admin per-agent code mode toggle
- Unified config file loading and validation
- A2A multi-agent concurrency and ordering: globally ordered event stream (prefer DB auto-increment or a centralized sequence generator)

## 11. Documentation And Context Notes

- Every change must be documented in tracked docs.
- `docs/features/` stores stable feature design docs (background, scope, key decisions, validation).
- Feature doc filenames must be topic-oriented kebab-case (no date prefix), for example `agents-teams.md`, `frontend-design.md`.
- `docs/journal/` stores dated implementation logs and checkpoints; filenames must use `YYYY-MM-DD-topic.md`.
- Add follow-up and verification items to `docs/todo.md`.
- API naming conventions live in `docs/api_naming.md` and must be followed for AgentHub-owned payloads.
- `.info/` is for local, non-versioned research context (for example papers, clippings, reference implementations).
- Do not treat `.info/` as production truth. Promote adopted decisions into `docs/features/` and/or `docs/todo.md`.
- Optional local search backend: configure `ck` MCP server (`ck --serve`) in `~/.agenthub/mcp.json` for ACP semantic/hybrid search.

## 12. TODO Lifecycle And CI Verification Rules

- Keep `docs/todo.md` as the single verification backlog for implementation follow-ups.
- Add new verification items near the top of `docs/todo.md` so active work stays visible.
- Mark an item as done (`[x]`) only when evidence exists:
  - local/manual checks: include explicit validation steps in the related feature note;
  - CI checks: require successful workflow evidence in GitHub Actions logs.
- For CI items that explicitly mention both push and PR behavior, verify both event types before marking done.
- For CI verification evidence, record workflow name and run IDs in PR description (or issue comment) before merge.
- Remove superseded items from active `docs/todo.md` backlog and keep historical context in feature notes / merged PRs.

### CI Baseline (as of 2026-02-17)

- `Rust`: cargo check + coverage (`rust-cargo.lcov`) + Codecov flag `rust-cargo`.
- `Clippy`: independent `cargo clippy --workspace --all-targets -- -D warnings` gate.
- `Web`: lint + unit coverage + build + Codecov flag `web`.
- `Web E2E`: Playwright E2E coverage + Codecov flag `web-e2e`.
- `Bazel`: `bazel build //...` and `bazel test //...`.
- `User Docs`: Docusaurus docs install/build checks.
