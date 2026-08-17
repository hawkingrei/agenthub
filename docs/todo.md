# TODO

Active backlog only. Keep this file small and current.

## Release And Packaging

- [ ] `P0` Restore Homebrew channel parity before advertising it as a current complete install. The
  `linkerdog/homebrew-tap` formula still points to `v0.0.7` and installs the legacy
  `agenthub-codex-acp` helper, while current GitHub artifacts publish `agenthub-acp`. Update and
  validate the formula against the intended formal release, including both binary versions and
  `brew services` startup. Evidence:
  [journal/2026-08-13-user-documentation-release-readiness.md](journal/2026-08-13-user-documentation-release-readiness.md).
- [ ] `P1` Replace the temporary security pins for
  `hawkingrei/codex@6ca61345ceb09d76edc3db8c4eb55df18a10888a` and
  `hawkingrei/symposium-acp@c731bb045d1375af48b0446af728aea52503b30b` with upstream stable
  releases that resolve the same Dependabot package set, then rerun the AgentHub ACP Cargo and Bazel
  compatibility suites. Do not move back while the default ACP runtime graph contains an affected
  `gix`, `hickory-proto`, `jsonwebtoken`, `opentelemetry_sdk`, `rmcp`, or `tar` release. Evidence:
  [journal/2026-08-12-dependabot-security-remediation.md](journal/2026-08-12-dependabot-security-remediation.md).
- [ ] `P1` Verify preview release partial-asset behavior: semver release run `29194967848` for `v0.0.11` proves Linux `x86_64` / `aarch64` release builds used `release-vendored-openssl`, avoided the stale cross-sysroot OpenSSL panic, and published canonical `agenthub` / `agenthub-acp` assets plus Debian packages. Remaining evidence is a preview release run showing successful binary assets publish when one release matrix target fails. Record the preview workflow run ID and release URL in [journal/2026-04-20-release-vendored-openssl-and-partial-assets.md](journal/2026-04-20-release-vendored-openssl-and-partial-assets.md).
- [ ] `P1` Define and enforce the Linux runtime baseline for official artifacts: the x86_64 prebuild from workflow run `31259043337` starts on Ubuntu 24.04 but requires `GLIBC_2.38` / `GLIBC_2.39` and does not start on Ubuntu 22.04. Pin the supported glibc floor, build against a compatible sysroot, and add a published-binary startup smoke on the oldest supported Linux distribution. Evidence: [journal/2026-08-08-object-store-s3-release-enablement.md](journal/2026-08-08-object-store-s3-release-enablement.md).

## Team Workspace Browser Matrix

Stable contracts:

- [features/frontend-design.md](features/frontend-design.md)
- [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- [features/team-channels-threads.md](features/team-channels-threads.md)
- [features/agents-teams.md](features/agents-teams.md)
- [features/teams-collaboration-playbook.md](features/teams-collaboration-playbook.md)

Matrix to keep current on `agenthub.hawkingrei.com` and in PR browser evidence:

- [ ] `P1` Conversation and composer polish: channel/thread composers and the ACP input dock share the same lightweight input language; ACP, thread, and mailbox bubbles share neutral base styling while retaining their local layout and interaction behavior. Remaining work is authenticated deployed-browser evidence for the full workspace flow. Existing notes: [journal/2026-04-24-team-conversation-slock-polish.md](journal/2026-04-24-team-conversation-slock-polish.md), [journal/2026-08-01-team-conversation-style-convergence.md](journal/2026-08-01-team-conversation-style-convergence.md).
- [ ] `P2` PWA installability: local app-router coverage now proves shell fallbacks, `sw.js`, and `manifest.webmanifest` stay `no-cache`, hashed `/assets/*` stay immutable, and missing asset paths do not fall back to HTML. `/pwa-192.png` and `/pwa-512.png` were solid navy placeholder squares with no logo, then were mistakenly regenerated from `slock-icon.png` -- a third-party OAuth provider's mark, not AgentHub's own -- so every installed/bookmarked instance showed another product's logo. Both now use a neutral "A" monogram placeholder in the app's own color palette instead; still needs a real, designed AgentHub brand mark. Evidence: [journal/2026-08-16-pwa-icon-branding-fix.md](journal/2026-08-16-pwa-icon-branding-fix.md), [journal/2026-08-17-pwa-icon-borrowed-slock-mark-fix.md](journal/2026-08-17-pwa-icon-borrowed-slock-mark-fix.md). 2026-07-19 production-domain retries at 07:37 UTC, 09:47 UTC, 11:15 UTC, 12:43 UTC, 13:04 UTC, 13:23 UTC, 13:44 UTC, and 14:12 UTC, plus a 2026-07-20 20:08 UTC retry, for `/workspace/teams`, `/sw.js`, `/manifest.webmanifest`, and a missing `/assets/*` probe still returned Cloudflare `502`, which only proves the deployed entrypoint was unavailable. Remaining work is deployed-domain verification that manifest/service-worker remain installable without stale shell caching and that CDN/proxy headers preserve the same contract after the entrypoint is healthy. Stable contract: [features/web-static-assets-and-pwa.md](features/web-static-assets-and-pwa.md); notes: [journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md](journal/2026-04-03-pwa-install-and-team-permission-card-collapse.md), [journal/2026-07-19-pwa-cache-control-router-guard.md](journal/2026-07-19-pwa-cache-control-router-guard.md).

## Team Workspace Architecture

- [ ] `P1` Implement Teamspace multi-user collaboration: add invite-only membership for local
  accounts, Teamspace-scoped server-side authorization, immutable invite-token handling, and audit
  records. Preserve the single-owner execution invariant: every executable Task and Step has one
  responsible member and one active fenced claim; split parallel work into dependency-linked Tasks
  rather than multi-assignee work. Stable contract: [features/teamspace-multi-user.md](features/teamspace-multi-user.md).
- [ ] `P1` Complete goal/fork control: task claims now atomically create durable, generation-fenced
  goal leases with Team/member capacity limits; terminal completion, cancellation, and handoff
  release reservations while retaining audit history. Read-only forks are now Team-bounded,
  generation-fenced, exposed through authorized list/create/complete APIs, and return immutable
  result evidence to the parent Task. Remaining work is conflict escalation, lease renewal/recovery,
  workbench UI state, and adapter-enforced no-write policy. Forks must never write workspaces or
  perform external mutations; informational requests must not preempt an active goal. Stable
  contract: [features/team-goal-fork-control.md](features/team-goal-fork-control.md).
- [ ] `P1` Phase 1/2 unified workspace shell follow-up: PR #891 merged the Team route facade, shared-lens cleanup, channel-scoped member profile routes, compatibility-only legacy query handling, shared workspace shell primitives, and three-zone Team workbench composition without collapsing Team task-first semantics or turning `thread` into a top-level lens. Desktop task-detail preview now reuses the shared split-pane primitive instead of staying modal-only. Remaining work is deeper shell reuse for other persistent Team context docks and deployed browser evidence across Team navigation surfaces. Stable contract: [features/workspace-unified-ia.md](features/workspace-unified-ia.md); notes: [journal/2026-04-18-workspace-shell-route-phase1.md](journal/2026-04-18-workspace-shell-route-phase1.md), [journal/2026-04-18-workspace-shell-phases-1-3-convergence.md](journal/2026-04-18-workspace-shell-phases-1-3-convergence.md), and [journal/2026-07-20-workspace-task-detail-triad.md](journal/2026-07-20-workspace-task-detail-triad.md).
- [ ] `P1` Complete explicit Team adoption extensions: PR #992 added stopped-only `move existing agent to Team` beside configuration-only copy. Remaining work is opt-in workspace-content copy and memory/context seeding with provenance, exclusion manifests, idempotent retry, focused backend/web tests, and browser evidence. Do not copy sessions, credentials, caches, or mutable source context by default. Stable contract: [features/team-agent-adoption.md](features/team-agent-adoption.md); notes: [journal/2026-05-03-team-agent-adoption-contract.md](journal/2026-05-03-team-agent-adoption-contract.md).
- [ ] `P2` Frontend performance hardening for Team and ACP-heavy pages: reduce avoidable rerenders, keep long lists and live surfaces responsive, and evaluate virtualization/stick-to-bottom behavior for extremely long histories. Stable contracts: [features/frontend-design.md](features/frontend-design.md) and [features/acp-runtime.md](features/acp-runtime.md).

## Team Runtime And Task Model

Stable contracts:

- [features/agents-teams.md](features/agents-teams.md)
- [features/team-execution-vocabulary.md](features/team-execution-vocabulary.md)
- [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)

- [ ] `P1` Verify remote Team direct-mailbox routing on real multi-node teams: after the local API regression and routing fix in [journal/2026-05-26-team-remote-direct-mailbox-routing.md](journal/2026-05-26-team-remote-direct-mailbox-routing.md), confirm direct single-member delivery still preserves mention metadata plus summary/`detail_ref` payloads when the recipient agent is remote and transport falls back to p2p relay in a real multi-node rollout. Existing notes: [journal/2026-03-26-team-direct-mailbox-summary-first.md](journal/2026-03-26-team-direct-mailbox-summary-first.md).
- [ ] `P2` Verify Team agent self-maintenance and deferred follow-up flows: `profile_patch_proposal`, `agent_time_trigger_*`, and operator-controlled `agent_loop` should behave consistently without blocking normal task progress.

## Frontend UI/UX

- [ ] `P2` Close out the remaining findings from the 2026-08-16 code-only UI/UX review: `team_markdown.ts`
  duplicates `markdown.ts`'s LRU cache/`sanitizeHref`/autolink logic instead of sharing it; `InputDock`
  and `TeamMessageComposer` use two different color-token systems and send-button shapes, contradicting
  [features/frontend-design.md](features/frontend-design.md)'s "one composer language" rule; channel-feed
  author name renders heavier than message body text where the spec requires lighter; `rich_text_classes.ts`
  styles both `.md-*` semantic classes and raw tag selectors in parallel; muted text tokens sit at the
  WCAG AA 4.5:1 floor and are used at 10-11px sizes with no large-text relief; `min-w-[220px]` fixed-width
  blocks inside popovers/menus can force horizontal scroll on narrow viewports; `prefers-reduced-motion`
  is described in the spec as implemented but has zero implementation anywhere in `web/src`; most
  `admin_page_sections.tsx` buttons still lack a loading state beyond the destructive ones already fixed;
  `acp_tool_fold.tsx`'s `IntersectionObserver` auto-collapse silently closes a manually-opened tool card
  when it scrolls out of view. Evidence: [journal/2026-08-16-frontend-uiux-review-round1-fixes.md](journal/2026-08-16-frontend-uiux-review-round1-fixes.md).

## Backend Correctness

- [ ] `P1` Close out the remaining findings from the 2026-08-16 code-only Rust backend review: the
  remote relay worker's `std::sync::Mutex::lock().expect(...)` calls mean any panic while one is held
  permanently and silently poisons the lock, killing remote message relay for the process's lifetime
  with no restart or alerting; `update_team_task`'s gRPC handler accepts a non-object `context_json`
  (only validates it's *valid* JSON, unlike its `context_merge_json` sibling which checks the shape),
  which plants a landmine that panics the *next*, unrelated run-status-changing request touching that
  task at `run_task_status_sync.rs`'s `.as_object_mut().expect(...)`; no timeout on the child-process
  stdin write path, so a hung child can block that agent's stdin lock indefinitely; several silently-
  swallowed DB/parse errors that leave no log trace (corrupt JSON resets linked-task-sync context to
  `{}`, a failed startup `safe_paths` seed insert is invisible). Three findings from the same review are
  fixed separately: `safe_paths` workdir enforcement, see
  [journal/2026-08-16-safe-paths-workdir-enforcement.md](journal/2026-08-16-safe-paths-workdir-enforcement.md);
  the unbounded `TeamRemoteRelayAdapter.grpc_client_cache` leak and its token-staleness correctness gap,
  see [journal/2026-08-16-grpc-relay-client-cache-ttl.md](journal/2026-08-16-grpc-relay-client-cache-ttl.md);
  the internal gRPC bootstrap-token comparison's timing side-channel, see
  [journal/2026-08-16-bootstrap-token-constant-time-compare.md](journal/2026-08-16-bootstrap-token-constant-time-compare.md).
  Evidence for the rest: this review round has no dedicated journal entry yet (findings were reported
  inline, not yet written up) -- write one when starting on the next item from this list.
- [ ] `P2` Close out the remaining finding from the 2026-08-17 code-only Team-subsystem review:
  `message_index_projection.rs` silently coerces unparseable `payload_json`/`input_json` to
  `Value::Null`/defaults during index rebuild with no logging, hiding real data corruption as an empty
  message. Six findings from the same review are fixed separately: goal-lease CAS hardening across
  `task_updates.rs`/`teamspace.rs`/`run_task_status_sync.rs`, see
  [journal/2026-08-17-goal-lease-cas-hardening.md](journal/2026-08-17-goal-lease-cas-hardening.md);
  permission-review reviewer-target consistency (removing the idle-unaware fallback resolver), see
  [journal/2026-08-17-permission-review-reviewer-target-consistency.md](journal/2026-08-17-permission-review-reviewer-target-consistency.md);
  `context_json`/`context_merge_json` shape validation, see
  [journal/2026-08-17-task-context-json-shape-validation.md](journal/2026-08-17-task-context-json-shape-validation.md);
  the client-spoofable `requires_user_visible_reply` suppression via `source_kind`, see
  [journal/2026-08-17-reply-obligation-client-suppression-fix.md](journal/2026-08-17-reply-obligation-client-suppression-fix.md);
  the pair-only, thread-unaware reply-obligation credit matching that let a reply in one conversation
  incorrectly close an unrelated obligation in another, see
  [journal/2026-08-17-reply-obligation-thread-scoped-matching.md](journal/2026-08-17-reply-obligation-thread-scoped-matching.md);
  and `reassign_reply_required_message`'s missing CAS guard and `idempotency_key = NULL` reassignment
  insert (transfer/escalate/takeover in `mailbox_service_escalation.rs`), see
  [journal/2026-08-17-mailbox-reassignment-cas-and-idempotency.md](journal/2026-08-17-mailbox-reassignment-cas-and-idempotency.md).

## Message Storage

- [x] `P1` Complete the RocksDB `cf_index` authority-derived repair path, per [features/message-storage-tiering.md](features/message-storage-tiering.md). SQLite authority rows now rebuild the conversation, actor mailbox, run-event, and agent-event projections; guarded ordered reads compare high-water marks and exact SQLite page IDs, fall back on any gap, and queue a startup worker for asynchronous repair. Orphan/prune helpers remain diagnostics and explicit maintenance only. Keep normal SQLite bodies readable until a later authority-cutover decision. Notes: [journal/2026-06-10-message-store-foundation-crate.md](journal/2026-06-10-message-store-foundation-crate.md).
- [x] `P1` Stage SQLite retirement by responsibility, not by a flag flip: Phase 1 `cf_body` dual-write, durable SQLite outbox, startup drainer, and SQLite compatibility reads are landed; `cf_index` is now a rebuildable, guarded delivery projection. Dual-read comparison and full rebuild/backup-restore recovery evidence are now landed for the `cf_body`/`cf_index` layer: a disaster-recovery test proves a RocksDB checkpoint restores correctly after the source store is deleted entirely and passes the same integrity gates (`check_index_refs_have_bodies`/`check_index_refs_have_authority`) an operator would run, and a simulated total `cf_index` loss is fully rebuilt from SQLite authority alone with zero gaps/orphans. The transactional `ControlStore` contract (conditional updates, uniqueness, audit, per-entity rollback) is now defined and its Phase 1 foundation (`crates/agenthub-db/src/control_store.rs`) is landed; SQLite remains the control-plane authority (no engine change, per the spec's Non-Goals). Phase 3 backfill is done for every call site identified when the spec was written: `teamspace.rs`'s CAS guards, generation-fencing, and audit writes; `conversation_idempotency.rs`'s and `mailbox_threads.rs`'s idempotency-replay decisions; a third, previously-undiscovered duplicate matcher in `src/api/teams/errors.rs`; and the `SQLITE_CONSTRAINT_UNIQUE_CODE` redeclarations in `manager_consts.rs` and (dead code) `src/api/teams.rs` are gone. `channel_mutations.rs`'s bootstrap-uniqueness matcher was deliberately left alone (different, OR-of-two-conditions shape). All behavior-preserving; all existing tests pass unchanged. Remaining work is Phase 2 only: new control-plane authority code (Teamspace multi-user membership, goal/fork conflict escalation, future permission tables) routes through `ControlStore` from the start instead of hand-rolling a new guard/matcher -- an ongoing discipline applied as that work lands, not a standalone task. RocksDB indexes and LanceDB archives must not become control-plane authority by implication. Stable contracts: [features/message-storage-tiering.md](features/message-storage-tiering.md), [features/control-store.md](features/control-store.md); evidence: [journal/2026-06-10-message-store-foundation-crate.md](journal/2026-06-10-message-store-foundation-crate.md).

## Maintenance Rules

- Keep only open work here. Remove completed items after evidence lands in a journal, PR, or canonical feature spec.
- Prefer canonical feature specs in [features/](features/) over stale micro-journal references whenever the contract is already stable.
- Collapse duplicated verification bullets into one umbrella matrix when they describe the same rollout surface.
