# Summary

The `codex` dependency -- pinned via git rev to a private fork, `hawkingrei/codex`, branch
`agenthub-security-deps` -- had been stuck roughly 5 months behind real upstream `openai/codex`.
The fork branch was built by adding two custom commits (a security-dependency patch and a Bazel
external-crate-rename fix) directly on top of the fork's own `main`, but that `main` was never
resynced with upstream after the patches landed, so every dependent crate in agenthub was
building against a codex snapshot from 2026-03-27 while upstream had moved 4619 commits further.
Resynced the fork to current upstream, rebased both patches on top, and adapted agenthub's own
ACP adapter code (`agenthub-codex-acp`) for the real API drift that surfaced.

# Scope

**Fork resync** (`hawkingrei/codex`):
- Confirmed the fork's `main` was a clean fast-forward ancestor of upstream `openai/codex`'s
  `main` (no divergent history), so a plain `git rebase --onto upstream/main origin/main
  agenthub-security-deps` was attempted first -- it tried to replay 4067 commits instead of the
  expected 2, because `agenthub-security-deps`'s deeper history contains commits that are
  content-identical to, but SHA-different from, what's now on the fork's `main` (an artifact of
  a prior sync/rebase pass). Aborted and instead cherry-picked just the 2 real patch commits
  (`fix(deps): patch AgentHub security dependencies`, `fix(bazel): honor renamed external
  crates`) onto a fresh branch cut from `upstream/main`.
- The security-deps patch conflicted on 3 files: `codex-rs/Cargo.toml` (upstream had
  independently moved `rmcp` from `=3.0.0` to `=3.1.2`; kept upstream's newer version alongside
  the patch's own `reqwest-otel` addition), `codex-rs/Cargo.lock`, and `MODULE.bazel.lock` (both
  regenerated from the resolved `Cargo.toml`/`MODULE.bazel` rather than merged by hand -- a full
  `cargo generate-lockfile` was tried first and over-resolved unrelated transitive deps to
  versions requiring a newer Rust toolchain than the repo pins; a plain `cargo check` against the
  reset-to-upstream lockfile added only the two new entries needed). The Bazel fix's target file
  (`defs.bzl`) auto-merged cleanly.
- Pushed the result as a new branch, `agenthub-security-deps-2026-08`, rather than force-pushing
  over the existing `agenthub-security-deps` branch -- non-destructive, and Cargo/Bazel pin by
  commit SHA rather than branch name anyway.
- Repointed all 20 git-rev references across `Cargo.toml`, `crates/agenthub-acp-adapter/Cargo.toml`,
  `agenthub-codex-acp/Cargo.toml`, and `MODULE.bazel` to the new commit. `cargo update -p
  tonic-prost-build --precise 0.14.3` was needed alongside the workspace lockfile update: the new
  codex snapshot's `codex-code-mode-protocol` hard-pins `=0.14.3`, conflicting with agenthub's own
  already-resolved `0.14.6` (compatible with agenthub's own `"0.14"` requirement, so this is a
  safe downgrade within the same requirement, not a version bump).

**API-drift adaptation** (`agenthub-codex-acp`), found via `cargo check` after the pin bump:
- `Op::UserInput` was replaced by `Op::TurnInput { request: Box<TurnInputRequest>, mode:
  TurnInputMode, reply: oneshot::Sender<CodexResult<TurnInputSubmission>> }`, a genuine
  architecture change: turn submission now routes through Core's start-or-steer machinery and
  reports its outcome via an embedded reply channel, rather than being pure fire-and-forget.
  Constructs `Op::TurnInput` with `TurnInputMode::StartOrSteer` (matching the old `Op::UserInput`
  semantics) and a fresh reply channel whose receiver is deliberately dropped unused at every
  call site -- callers never awaited a submission result for plain user input before (outcomes
  were always observed through the subsequent event stream instead), so this preserves behavior
  exactly. `agenthub-codex-acp`'s own `CodexThreadImpl` trait is shared by two backends -- an
  in-process Core thread and an RPC-backed `AppServerCodexThread` -- and a `oneshot::Sender` can't
  cross an RPC boundary; `AppServerCodexThread` already had its own independent steer-vs-start
  decision logic (`try_steer_submission`/`prepare_submission_start`, driving `TurnStart`/`TurnSteer`
  RPC calls), so its adaptation was just extracting the equivalent fields from the new
  `TurnInputRequest` shape rather than any redesign of that logic.
- Rollout-history types (`RolloutItem`, `InitialHistory`, `ResumedHistory`, `CompactedItem`)
  moved from `codex_protocol::protocol` to a new `codex-history` crate; `RolloutItem::ResponseItem`
  now wraps a `ResponseItemEnvelope` (adds an optional harness-metadata field) instead of a bare
  `ResponseItem`. `repair_rollout_items` (operates on `Vec<RolloutItem>`) was adapted in place,
  threading metadata through on retained items. A second repair function,
  `repair_response_item_history` (operates on bare `Vec<ResponseItem>`, kept as-is since it has
  its own direct unit test and other callers), couldn't be reused for
  `CompactedItem.replacement_history` once that field's type changed to
  `Vec<ResponseItemEnvelope>`; added an envelope-aware sibling,
  `repair_response_item_envelope_history`, mirroring the same logic.
- New enum variants needed handling: `EventMsg::ThreadQueueChanged` and five new
  `ServerNotification` variants (`ThreadReverted`, `ThreadQueueChanged`, `ProjectChanged`,
  `ThreadProjectUpdated`, `StrictReviewRequired`) joined existing no-op/unhandled buckets.
  `ReviewDecision::ApprovedMcpPolicyAmendment` fails closed (declined/filtered out) in both the
  command-execution and file-change approval paths, mirroring upstream's own
  `CoreReviewDecision -> CommandExecutionApprovalDecision` conversion and its documented
  reasoning: MCP approvals are handled through elicitations, so this decision should never reach
  either approval flow. `CodexErrorInfo::MisalignmentPolicyViolation` maps 1:1 between the
  app-server and core variants of that enum, alongside the existing `CyberPolicy` arm.
  `AgentMessageEvent`/`ThreadItem::AgentMessage` gained an optional `delivery` field
  (`AgentMessageDelivery::Async` marks async-delivered messages); every construction site in
  agenthub sends ordinary synchronous messages, so `delivery: None` throughout.
  `McpServerTransportConfig::StreamableHttp` gained `http_headers_helper: Option<String>` (a
  local-shell-command hook for dynamic headers); `None`, since agenthub doesn't use it.
- `Op` no longer derives `Clone`/`PartialEq` (the new `TurnInput` variant's reply channel isn't
  cloneable or comparable), which broke a test-double's op-recording (`StubCodexThread` in
  `thread.rs` cloned every submitted op for later assertions) and several `assert_eq!` comparisons
  against literal `Op` values. Added `record_op`, a manual field-by-field reconstruction covering
  every variant that flows through the test mock (rebuilding `TurnInput` with a fresh, again
  unused, reply channel); converted whole-value `assert_eq!` comparisons to `matches!`-based slice
  patterns with inner-field equality checks instead, since pattern matching doesn't require
  `Op: PartialEq` the way value comparison does.

# Key Decisions

- Cherry-pick the 2 real patch commits onto fresh upstream rather than rebase the full branch
  history -- the branch's actual unique-to-it commit count (2) diverged sharply from what
  `git rev-list` reported (4067) due to a prior sync artifact; rebasing the reported range would
  have replayed thousands of already-upstream commits and hit spurious conflicts on all of them.
- Push the resync as a new branch (`agenthub-security-deps-2026-08`) instead of force-pushing over
  `agenthub-security-deps` -- avoids rewriting a remote branch's history; Cargo/Bazel pin by exact
  commit SHA, so the branch name is purely a human-readable label with no functional role in the
  pin itself.
- `Op::TurnInput`'s reply channel is constructed and then deliberately dropped unawaited at every
  agenthub call site, rather than threading a `TurnInputSubmission` result back to callers --
  confirmed by checking that `TurnInputMode::StartOrSteer` (unlike `StartIfIdle`) has no documented
  decline path, so dropping the receiver loses no information callers previously had; the
  subsequent event stream remains the sole mechanism for observing what actually happened, exactly
  as before this change.
- `ApprovedMcpPolicyAmendment` fails closed (declined) in agenthub's own approval-decision mapping
  functions, rather than being treated as an implicit accept alongside other amendment-kind
  variants -- deliberately matched to upstream's own conversion and its explicit code comment
  reasoning, rather than picked independently, since upstream had already made and documented this
  exact judgment call for the identical scenario.

# Validation

- `cargo build --workspace --lib --tests` -- clean, only a single expected `#[cfg(test)]`-only
  dead-code warning (a function only exercised by its own dedicated unit test, unrelated to this
  change).
- `cargo test -p agenthub-codex-acp-runtime` -- 133 passed, including every test exercising the
  redesigned submission path (`test_init`, `test_prompt`, `test_review`, `test_undo`,
  `test_custom_review`, `test_branch_review`, `test_commit_review`,
  `prepare_submission_start_leaves_runtime_workspace_roots_unset`, and others).
- `cargo test -p agenthub --lib` -- 767 passed; only the 3 pre-existing, already-documented
  `lance-namespace-impls` panics (`state::tests::initialize_services_*`, an unrelated crate-version
  issue tracked separately) failed.
- `cargo clippy -p agenthub -p agenthub-acp-adapter -p agenthub-codex-acp-runtime
  -p agenthub-object-store --lib --tests` -- clean.
- `cargo fmt -- --check` -- clean.
- `bazel mod deps --lockfile_mode=update` -- resolves cleanly against the new pin (confirms the
  Bazel external-crate-rename patch still applies correctly across the version jump).
- `bazel build` of targets under `agenthub-codex-acp`/`agenthub-acp-adapter` could not be
  validated locally: this codex snapshot's `codex-app-server` pulls in `codex-arg0` ->
  `codex-linux-sandbox` -> a vendored `bwrap` FFI target that's constrained to
  `@@platforms//os:linux`, which fails to build on macOS regardless of this change. CI's Bazel job
  runs on Linux and will perform the real native-build validation this session's tooling couldn't.

# Follow-Ups

- None identified. The fork resync and adaptation are both complete and symmetric with prior
  behavior; no reduced or partially-migrated state was left behind.
