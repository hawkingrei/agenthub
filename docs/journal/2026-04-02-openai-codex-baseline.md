# OpenAI Codex Baseline For AgentHub ACP

## Summary

`agenthub-codex-acp` now treats the official `openai/codex` repository as its upstream dependency baseline and already includes the first live `codex-app-server` bridge slice on top of that baseline.

The adapter is pinned to the commit that backs the latest stable GitHub release that was available when this change was prepared:

- release tag: `rust-v0.118.0`
- pinned commit: `b630ce9a4e754d35a1f33e4366ba638d18626142`
- release name: `0.118.0`
- published at: `2026-03-31T17:02:18Z`

This change is still intentionally narrow in scope: it does not rewrite ACP history replay or frontend rendering around a new native app-server model, but it does move the live execution bridge onto the official app-server thread/turn APIs.

## Why

- The official `openai/codex` repository publishes the same core crates AgentHub depends on today (`codex-core`, `codex-protocol`, `codex-login`, `codex-mcp-server`, and related utilities).
- The official repo also contains the `codex-app-server`, `codex-app-server-client`, and `codex-app-server-protocol` crates that define the richer `thread` and `turn` lifecycle AgentHub wants to adopt next.
- Keeping `agenthub-codex-acp` pinned to a Zed-hosted fork would make the next refactor harder to justify and review because the target runtime contract would still live outside the selected upstream.

## Current Adapter Gap

The live ACP execution path now goes through `app_server_thread.rs`, but the surrounding adapter still retains significant legacy shape:

- ACP history replay and a large portion of the renderer still consume the legacy `EventMsg` stream from `agenthub-codex-acp/src/thread.rs`;
- prompt lifecycle is still tracked locally by `submission_id`, even though live follow-up input is already routed through app-server `turn/start` and `turn/steer`;
- the bridge therefore still has to translate app-server thread/turn state back into the older ACP session model instead of exposing a native app-server session model end to end.

## Follow-up

The next implementation slice should reduce the amount of legacy ACP orchestration that still wraps the live app-server bridge:

1. keep existing ACP session and history plumbing only where replay compatibility still requires it;
2. continue shrinking `submission_id`-driven special cases now that live prompt routing already chooses between `turn/start`, `turn/steer`, and local queueing inside the app-server bridge;
3. tighten notification/request parity until the adapter no longer needs bridge-local fallbacks for resumed turns and approval diffs.

## App-Server Bridge Slice

The next slice is now partially implemented.

- `agenthub-codex-acp` directly depends on `codex-app-server-client`, `codex-app-server-protocol`, and `codex-feedback` from the same `rust-v0.118.0` tag.
- `new_session` and `load_session` now bootstrap live execution through an embedded `InProcessAppServerClient` instead of creating or resuming a `codex-core` thread directly.
- A new `app_server_thread.rs` bridge implements `CodexThreadImpl` and translates:
  - ACP prompt-like operations into `turn/start`, `turn/steer`, `review/start`, `thread/compact/start`, and `thread/rollback`;
  - app-server server requests into existing ACP approval and elicitation events;
  - app-server notifications into the legacy `EventMsg` stream consumed by the ACP renderer.
- Existing ACP history replay and UI plumbing remain in place for now. The refactor is intentionally focused on live turn execution.

## Shared Submission Handling

The ACP adapter still tracks prompt lifecycle by `submission_id`, but app-server follow-up turns can reuse the same active turn via `turn/steer`.

To avoid splitting one live app-server turn into multiple ACP completion channels:

- `PromptState` now supports multiple response waiters for the same `submission_id`;
- when `thread.submit(op)` returns an already-tracked `submission_id`, ACP attaches a new waiter instead of replacing the existing prompt state;
- one `TurnComplete` or `TurnAborted` event now resolves every waiter attached to that submission.

## Resumed Active Turn Recovery

The next gap was `load_session` against a thread that already had an in-progress turn.

Two separate issues showed up there:

- resumed live events could arrive under a submission id ACP had never seen locally yet;
- resumed regular turns were reconstructed as non-steerable, so follow-up ACP prompts would queue locally instead of attempting `turn/steer`.

This slice tightens both sides:

- `thread.rs` now attaches a detached `PromptState` when a resumed live event arrives before ACP has a local submission entry for that turn;
- `app_server_thread.rs` now reconstructs resumed active turns from `thread/resume` metadata and treats them as steerable by default unless the restored turn history clearly shows review mode;
- if a resumed follow-up `turn/steer` fails because local active-turn state is stale, ACP now clears the stale local turn bookkeeping and immediately starts a fresh turn instead of queueing behind a turn that already ended server-side;
- if `turn/steer` fails with `ActiveTurnNotSteerable`, ACP now downgrades the local active turn to non-steerable so repeated follow-up prompts stop retrying the same rejected steer path.

## Review Item Translation

The app-server bridge was still dropping review-mode lifecycle items even though the ACP prompt state already knew how to consume them.

This slice now translates app-server review items back into core events:

- `ThreadItem::EnteredReviewMode` maps to `EventMsg::EnteredReviewMode` with the rendered review text preserved as a user-facing hint;
- `ThreadItem::ExitedReviewMode` maps to `EventMsg::ExitedReviewMode` with the rendered review text carried as the `overall_explanation` field of a synthetic `ReviewOutputEvent`.

That keeps the live ACP review path consistent with the existing prompt-state handling without introducing a second rendering path just for app-server sessions.

## Visible Notification Parity

The bridge was still ignoring several app-server notifications that matter to ACP users even when the underlying prompt state could already surface them.

This slice now narrows that gap:

- `model/rerouted` maps back to `EventMsg::ModelReroute`, and the ACP prompt path now renders a visible status line instead of only logging it;
- `configWarning` maps to a `Warning` event with a formatted message that preserves summary, details, and file location when available;
- `deprecationNotice` maps back to `EventMsg::DeprecationNotice`;
- when these warning-like events arrive without any active local submission state, `ThreadActor` now forwards them directly to the ACP client instead of dropping them as unknown submission traffic;
- resumed detached-submission attachment now also recognizes `ModelReroute`, so a resumed live turn does not lose a reroute notification if it arrives before any other turn-scoped event.

## Request User Input Bridge

The latest upstream app-server can also stop mid-turn and ask the user a structured question through `tool/requestUserInput`.

ACP does not have a native multi-question interaction primitive, so this slice wires the upstream request onto the existing ACP prompt loop instead of inventing a second transport:

- `app_server_thread.rs` now translates `ToolRequestUserInput` into `EventMsg::RequestUserInput` and resolves `Op::UserInputAnswer` back into the app-server request response;
- `thread.rs` now renders the upstream question as a pending ACP tool call and keeps the request attached to the live submission state;
- the next ACP `prompt` is intercepted when a question is pending and is converted into a `RequestUserInputResponse` instead of starting a fresh turn;
- single-question replies accept plain text or a JSON string array; multi-question replies accept either a JSON object keyed by question id or one text block per question in order;
- secret questions avoid echoing the structured answer payload back into ACP tool-call output;
- `codex_agent.rs` now enables `default_mode_request_user_input`, so Default mode can actually exercise the new question path instead of keeping it tool-gated.

## ACP-Native Question Card

The backend bridge was functional, but ACP users still had to answer upstream questions by manually shaping the next prompt.

This slice adds a first native ACP-side interaction layer on top of the existing bridge:

- `web/src/request_user_input.ts` now parses the synthetic `request-user-input:*` tool-call payload, initializes local drafts, and deterministically serializes answers back into the prompt text formats that `thread.rs` already accepts;
- `web/src/components/acp_conversation.tsx` now recognizes `request_user_input` tool calls and renders them as inline ACP cards instead of generic tool-call payload folds;
- the card supports single-question freeform answers, option selection, `None of the above` for `isOther`, and secret-question warnings without adding a second transport path;
- both the main agent ACP panel and the Team member ACP panel now reuse their existing `sendInput` path for question answers, so the new card stays aligned with the current session mismatch recovery and live-turn routing logic;
- completed question exchanges now also render as specialized result cards, including structured answer display for normal questions and private placeholders when secret questions suppress the serialized answer payload.

## Conversation Cutoff Collapse Parity

The ACP UI already defines the conversation cutoff window in `use_acp_conversation.ts` as `50` messages, and older non-message items are supposed to collapse while user/agent messages remain fully visible.

The problem was that this policy did not fully reach the newer live card variants:

- plain live tool calls still defaulted open even after they crossed the cutoff;
- grouped live tool-call cards and explore groups also stayed open because their fold state only followed live status, not the age-based cutoff;
- the new inline `request_user_input` card inherits tool-call rendering, so it would have kept the same inconsistency.

This slice now applies the existing cutoff policy consistently to the remaining non-message card families:

- `ConversationBubble` passes the existing `autoCollapse` decision into tool-call, grouped tool-call, and explore-group bubbles;
- each affected bubble now starts closed when it is already older than the cutoff, and auto-collapses once when it first crosses the cutoff on later renders;
- manual re-open still works after that first forced collapse, so the cutoff remains a timeline-noise guard instead of a persistent lock.

## Bazel `v8` Runfiles Fix

After the Cargo-side migration to `openai/codex rust-v0.118.0`, Bazel CI started failing inside the generated `v8 146.4.0` crate with:

- missing `gen/src_binding_release_x86_64-unknown-linux-gnu.rs`
- `include!(env!("RUSTY_V8_SRC_BINDING_PATH"))` aborting in `src/binding.rs`

The upstream `v8` crate already ships the prebuilt binding files in its published crate archive, so the failure is not a bad release artifact.

The break only appears in Bazel because `rusty_v8`'s `build.rs` emits `RUSTY_V8_SRC_BINDING_PATH` pointing at crate-local files under `gen/`, and the crate-universe generated `cargo_build_script` runfiles did not carry that directory into the sandboxed build.

The final Bazel-side fix follows the same Bazel consumer pattern that upstream `openai/codex` already uses for `rusty_v8`:

- keep Cargo dependencies unchanged;
- add a dedicated Bazel `http_archive` for the published `v8-146.4.0` crate and expose the prebuilt binding file through `third_party/v8/v8_crate.BUILD.bazel`;
- inject a `v8_targets` repo into `crate_universe` and set `build_script_data` / `build_script_env` so the `v8` crate receives explicit Bazel-provided `RUSTY_V8_SRC_BINDING_PATH` and `RUSTY_V8_ARCHIVE` inputs instead of synthesizing crate-local paths itself;
- reuse the upstream `rusty_v8` Bazel consumer patch pattern so custom archives are staged under `OUT_DIR` and the emitted link-search path survives the Rust compile action;
- keep the fix narrow to the current Linux consumer path and preserve the Cargo dependency graph.

## Clippy Follow-up

The first CI pass also surfaced a few mechanical `clippy -D warnings` failures in the ACP bridge layer.

These follow-ups intentionally keep behavior unchanged:

- collapse `override_turn_context` into a single argument struct so the helper stays lint-clean without suppressing `too_many_arguments`;
- switch `build_session_config` from `&PathBuf` to `&Path` and convert back only at ownership boundaries;
- remove a redundant `clone()` on `StopReason`, which already implements `Copy`.

## Test Determinism

Some `thread.rs` tests were still loading Codex through the normal config loader path, which means an invalid local `~/.codex/config.toml` could cause unrelated ACP regression tests to fail.

The test helpers now use Codex's default-config path instead of loading user config files, so the ACP suite no longer depends on workstation-local Codex configuration state.

## Validation Notes

- The release tag was verified from the official GitHub release endpoint for `openai/codex`.
- `cargo update -p arc-swap --precise 1.9.0` was required so the workspace lockfile could resolve the official `rust-v0.118.0` dependency graph.
- `cargo check -p agenthub-codex-acp` passes in the dedicated worktree after updating the lockfile and applying the minimal compatibility fixes for the release-tag API surface.
- The compatibility fixes in this slice are intentionally narrow: feature imports moved to `codex-features`, `ThreadManager` now uses `EnvironmentManager`, removed upstream custom-prompt fetch support is no longer requested from core, and newer protocol fields are mapped or ignored where required for compilation.
- After the bridge landed, `cargo check -p agenthub-codex-acp` still passes with the app-server-backed live session path enabled.
- A focused regression test was added for the shared-`submission_id` waiter behavior in `agenthub-codex-acp/src/thread.rs`.
- `cargo test -p agenthub-codex-acp test_shared_submission_id_completes_all_prompt_waiters -- --nocapture` passes.
- A focused regression test was added for resumed live events attaching a detached submission state in `agenthub-codex-acp/src/thread.rs`.
- `cargo test -p agenthub-codex-acp test_unknown_live_event_attaches_detached_submission -- --nocapture` passes.
- Focused unit tests were added for resumed-turn steering recovery in `agenthub-codex-acp/src/app_server_thread.rs`.
- `cargo test -p agenthub-codex-acp app_server_thread::tests -- --nocapture` passes.
- The app-server thread helper tests now also cover review-mode item translation (`entered_review_mode_translation_preserves_hint` and `exited_review_mode_translation_preserves_rendered_text`).
- The app-server thread helper tests now also cover `ModelRerouted` reason conversion and config-warning message formatting.
- A focused regression test was added for global visible events without a submission (`test_global_visible_events_without_submission_are_forwarded`).
- `cargo test -p agenthub-codex-acp test_global_visible_events_without_submission_are_forwarded -- --nocapture` passes.
- Focused unit and integration tests were added for the request-user-input bridge in both `agenthub-codex-acp/src/app_server_thread.rs` and `agenthub-codex-acp/src/thread.rs`.
- `cargo test -p agenthub-codex-acp request_user_input -- --nocapture` passes.
- After the resumed-turn recovery changes, `cargo check -p agenthub-codex-acp` still passes.
- After enabling Default-mode question support and formatting the updated bridge code, `cargo fmt -p agenthub-codex-acp`, `cargo check -p agenthub-codex-acp`, and `cargo test -p agenthub-codex-acp test_shared_submission_id_completes_all_prompt_waiters -- --nocapture` pass.
- Focused frontend tests were added for `request_user_input` parsing, native ACP rendering, and inline submission wiring.
- `npm run test -- src/request_user_input.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/pages/team_member_acp_panel.test.tsx src/acp_panel.test.tsx` passes.
- `npm run lint` and `npm run build` pass in `web/`.
- Chrome DevTools baseline and regression checks were recorded against the local Vite shell at `http://127.0.0.1:4173/`; the page remained stable before and after the change, and the only visible runtime failure stayed the expected backend-less JSON/API error from loading the standalone frontend shell without the Rust API server.
- Focused ACP conversation tests now also cover older live tool cards collapsing once they move past the cutoff window.
- Focused helper/render tests now also cover completed `request_user_input` result parsing and secret-answer placeholders.
- `src/api/teams/tests_router.rs` now treats `stopped` as a valid runtime status for the router-level contract check after team creation.
- The router contract test still verifies the runtime response shape and member roster, while `src/api/teams/tests_core.rs::teams_api_create_team_auto_starts_member_runtime` remains the dedicated coverage for create-time auto-start semantics.

## Review Follow-up

The latest PR review pass surfaced a few bridge-level gaps that were worth fixing directly instead of hand-waving away as bot noise.

The app-server ACP bridge now keeps enough resumed-turn state to preserve user-visible follow-up events even when the local `active_turn` record is missing:

- server requests that carry a `turn_id` now fall back to that `turn_id` as the detached submission id instead of silently returning `None`;
- `TurnDiffUpdated` is no longer dropped and is cached so patch-approval events can still surface file diffs after resume;
- in-progress `ThreadItem::FileChange` items from `thread/resume` are rehydrated into pending patch-change state so the ACP approval UI can render a structured diff immediately;
- unexpected app-server `InProgress` completion statuses now degrade to failed command/patch terminal states instead of being misreported as success.

The Bazel-side `rusty_v8` archive wiring also now pins explicit `sha256` digests for the four published prebuilt archives used by the `rust-v0.118.0` consumer layout, so the custom archive injection is reproducible in CI instead of depending on unchecked remote downloads.

Additional focused validation was added for the new helper paths:

- `submission_id_for_turn_falls_back_to_turn_id_when_local_state_is_missing`
- `pending_patch_changes_from_turns_recovers_in_progress_patch_items`
- `parse_turn_diff_to_core_changes_splits_multi_file_diff_and_tracks_rename`
- `command_status_in_progress_maps_to_failed`
- `patch_status_in_progress_maps_to_failed`

Validation for this follow-up slice:

- `cargo check -p agenthub-codex-acp` passes after the resumed-turn fallback and diff-caching changes.
- `cargo test -p agenthub-codex-acp app_server_thread::tests -- --nocapture` passes with the new resumed-turn and diff parsing coverage.
- `cargo test -p agenthub-codex-acp test_unknown_live_event_attaches_detached_submission -- --nocapture` still passes after allowing detached `TurnDiff` delivery.

## Final Review Cleanup

The last unresolved review comments in the ACP bridge were mostly about correctness at the adapter boundary rather than missing functionality.

This cleanup tightens those edges without widening the bridge scope:

- Cargo now pins the official `openai/codex` baseline by the release commit `b630ce9a4e754d35a1f33e4366ba638d18626142` instead of by the symbolic `rust-v0.118.0` tag, so dependency provenance stays stable even if the tag is ever moved or reissued;
- `app_server_thread.rs` no longer holds the shared bridge mutex across `turn/steer`, `turn/start`, `review/start`, `thread/compact/start`, or `thread/rollback` RPC awaits; submission startup now stages request parameters under lock and finalizes state only after the request completes;
- app-server `turn/completed` notifications that still report `TurnStatus::InProgress` now terminate the ACP-side submission with an error event instead of leaving it open behind a warning-only path;
- config warning source locations are now translated to the 1-based coordinates expected by ACP and editor UIs;
- the ACP command vector wrapper is now platform-aware (`bash -lc` on Unix-like systems, `powershell.exe -Command` on Windows), but it remains only an ACP compatibility/display representation for shell-script requests, not the source of Codex permission matching;
- `codex_agent.rs` drops the temporary clones that became unused after the earlier session-config refactor;
- the journal summary now matches the current state of the branch: the live path is already on app-server, while replay and some renderer/session plumbing still retain legacy ACP shape.

Validation for this cleanup slice:

- `cargo fmt -p agenthub-codex-acp` passes after the bridge refactor.
- `cargo check -p agenthub-codex-acp` passes after converting the startup/steer helpers to the two-phase lock pattern.
- `cargo test -p agenthub-codex-acp app_server_thread::tests -- --nocapture` passes, including focused coverage for `turn_completed_in_progress_translates_to_error`, config warning formatting, and the platform shell wrapper helper.
- `cargo test -p agenthub-codex-acp test_unknown_live_event_attaches_detached_submission -- --nocapture` still passes.
- `cargo test -p agenthub-codex-acp test_shared_submission_id_completes_all_prompt_waiters -- --nocapture` still passes.
- `cargo test -p agenthub-codex-acp agenthub_codex_acp_ --lib -- --nocapture` passes, confirming the release-commit pin keeps the multi-agent feature toggles behaving as expected.

## Request-User-Input UI Review Cleanup

The final unresolved PR review threads were both in the new ACP `request_user_input` card and were about frontend state stability rather than backend bridge correctness.

This follow-up keeps the UI shape the same, but fixes two real issues:

- `RequestUserInputCard` no longer resets local drafts merely because polling rebuilt `raw_input` into a fresh `questions` array; it now resets only when the `toolCallId` or a stable semantic signature of the question payload changes;
- each inline answer textarea now has an explicit `id`, `name`, and `aria-labelledby` relation tied to the existing visible header and question text, so the card is accessible to screen readers without adding duplicate visible labels.

Validation for this UI cleanup slice:

- `npm run test -- src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx` passes with added coverage for stable draft retention across equivalent rerenders and textarea labeling.
- `npm run lint` passes in `web/`.
- `npm run build` passes in `web/`.
- Chrome DevTools regression check against the local Vite shell at `http://127.0.0.1:4173/` shows the page still loading to the same standalone shell state as before; the visible runtime error remains the expected backend-less `404/JSON` failure, and the existing generic form-field issue still appears even though the new request-user-input textarea now has explicit labeling metadata.
