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
- CSS guardrail: do not introduce new large handcrafted global CSS blocks; keep legacy `web/src/styles.css` changes limited to compatibility fixes during migration
- Maintainability policy: low-risk maintainability review suggestions should be implemented directly in the active change instead of being deferred by default
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
7) `src/main.rs` should stay a thin bootstrap entry; business logic should live in library crates and be composed in `src/app.rs` or domain crates.
8) New Rust domain modules should define Bazel targets with boundaries aligned to crate boundaries (one domain crate, one Bazel package as default).

## 5. Directory Plan (adjustable)

```
agenthub/
  crates/
    acp/
    openapi/
    ...                 # domain-oriented libraries
  src/
    app.rs              # bootstrap composition
    lib.rs              # library entry point
    main.rs
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
  AGENTS.md
```

## 6. Security and Reliability Principles

- Default least privilege: agent can only access the specified path
- Strict input validation: validate all API parameters
- Session persistence: reconnectable, not auto-closed
- Audit logging: write key actions to database logs
  - device login audit, device revocation, path deletion must be recorded

## 7. Testing and Validation (initial suggestions)

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
  - Avoid introducing new parallel style systems or expanding legacy handcrafted global CSS except compatibility patches
  - All frontend changes must be validated with Chrome DevTools MCP before edits (baseline) and after edits (regression check), and responses should include the MCP verification result summary
- Team role workflow policy:
  - Leader acts as architect/reviewer and should not implement feature code directly
  - Leader owns technical research and option comparison (assumptions, trade-offs, risks) before delegation
  - Leader runtime starts from an empty workspace and should maintain coordination context in `AGENTS.md`
  - Leader code review path should prefer `gh` (or explicit clone-only review workspaces)
  - Backend runtime enforces leader starts with `worktree_mode=use_existing` and an empty workspace
  - Each worker must execute in its own git worktree with a random feature branch, and periodically sync from `main`
  - Backend runtime enforces worker starts with `worktree_mode=create_worktree`, per-run isolated workdir, and random branch checkout
  - Workers may coordinate with peers when dependencies overlap, but status/evidence must still flow back to leader
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

- Every change must be documented.
- Add a TODO entry in `docs/todo.md` for follow-up or verification items.
- Add a feature note under `docs/features/` describing background, scope, key decisions, and validation.
- Feature notes should use `YYYY-MM-DD-topic.md` naming for easy lookup.
- API naming conventions live in `docs/api_naming.md` and must be followed for all AgentHub-owned payloads.

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

## 13. Change Log

### 2026-02-21

- Added Bazel-first Rust packaging constraints: domain-oriented crate decomposition, thin bootstrap entrypoint policy, and crate-to-Bazel boundary alignment guidance.
- Added explicit requirement/policy notes for future Rust migrations from `src/*` modules into cohesive `crates/*` libraries.

### 2026-02-05

- ACP session view switched to "bottom-stacked + scrollable", auto-stick to bottom on new messages (do not force when user scrolls up)
- ACP container height and scrolling constraints fixed (`output-body`/`acp`/`acp-conversation` all use flex constraints)
- ACP thought block collapse rule: only collapse after first agent_message appears
- User input de-dup: introduce `message_id` to avoid duplicates from WS + HTTP
- Session filter relaxed: allow messages missing `session_id` into the current view
- Mobile UX: Agents panel becomes overlay drawer with backdrop; smaller input and button sizes
- Agents collapse toggle moved next to Output title to ensure it is always reachable
- Tighten UI whitespace: reduce top padding in `acp-head` and set `align-items: flex-start`
- New style guard test: `tests/web_assets.rs` validates ACP container key styles
- Output history changed to "scroll up to load": when near top, fetch earlier records from DB (remove fixed Recent tab)
- Conversation view changed to waterfall order: `agent_thought` becomes `agent_thinking` bubble, inserted by `seq` between user and AI replies
- `agent_thinking` collapses by default after completion; stays expanded while thinking
- Local input events now get global `seq` and participate in ordering to avoid merge and ordering issues
- A2A requirement added: global ordering across multiple agents must be strictly replayable
- Output scrolling strategy: `output-body` fixed height and no own scroll; ACP/Terminal scroll independently
- Output history auto-fill: when current session has too few messages, auto-page earlier records
- Default active running agent: on first load, select the first running agent if none selected (otherwise the first in list)
- Input dock fixed at Output bottom (`.input.docked` uses `margin-top: auto`)
