# Loop Workspace UI

## Problem

A stopped process must not hide member configuration, task progress, or the reasons work will run
again. Task acceptance, execution policy, activation outcome, and process state have different
owners and lifetimes.

## Scope

The member overview and channel member profile expose offline configuration, launch preflight,
explicit execution enablement/suspension, manual activation, assigned tasks, and durable history.
They use existing workspace routes and the authorized loop APIs.

## Non-Goals

- Changing backend admission, authorization, canonical task ownership, or provider protocols.
- Inferring task completion from process exit or an activation's successful cleanup.
- Automatically opting a Team or member into execution.
- Providing a replacement for ACP process diagnostics or the debug-only doctor.

## Architecture

`/workspace/teams/:team_id/members/:member_id` is the member configuration/history entry point.
Loop member selection defaults to this overview; the existing ACP route remains available through
Process diagnostics. Channel member profiles expose the same panel without selecting a manual run.
Assigned task links open the canonical task route, while ordinary channel and task views retain
their existing durable APIs.

The configuration controller scopes reads and mutations to user, Team, member, and authentication
lifetime. History uses abortable, bounded keyset pages. The server remains the authority for access,
policy revisions, preflight, and admission.

## Contracts

- Process status, execution policy, activation state/outcome, and task status are displayed
  separately. An exited process can have enabled execution and an unfinished assigned task.
- Manual Teams require explicit selection of durable execution. Each member then requires explicit
  enablement after preflight succeeds. Suspension preserves queued work and permits a current
  activation to finish; resume does not imply task acceptance.
- Profile editing works offline. In loop mode, an empty prompt preserves the built-in role prompt.
  The profile reads member identity, role, model, and description from the current Team specification
  even when there is no execution record; closing it retains the existing channel navigation contract.
  Explicit overrides can be saved or cleared. The UI does not inject legacy role defaults or edit
  the legacy process idle watchdog through this flow. Legacy Teams retain their existing defaults.
- Configuration writes include the observed revision and preserve unrelated limits. Conflicts or
  uncertain responses cause a read of current server state, never an automatic write replay.
- An uncertain manual activation retains its request identity across page closure, scoped by user,
  Team, and member. Retry reuses that identity until a receipt arrives. Browser storage contains
  only the retry identity; authorization remains server-side. If storage is unavailable, retention
  lasts only for the mounted controller.
- Assigned tasks use the loaded workspace task page and their recorded status. `completion_proposed` is displayed as a proposal whose
  task review remains separate.
- Activation details show redacted source summaries, ordered lifecycle events, and durable tool
  observations. The UI never needs private launch credentials or raw tool arguments/results.
- Wake information is derived from the loaded pending activations and active schedule records.
  Further pages are explicit; a partial page is not presented as the globally earliest wake.
  Due times remain subject to execution policy and limits. Read failures do not imply no work.
- Polling pauses while hidden and while inspecting details or older history. Explicit refresh starts
  a new contiguous page chain. Zero-valued event/tool cursors are valid; stale responses from other
  members or authentication lifetimes cannot replace current data.

## Validation Matrix

| Boundary | Evidence |
| --- | --- |
| Offline routes and independent states | Workbench and member panel tests; actual browser lifecycle |
| Role default selection | Draft/profile helper tests, legacy/loop modal and management tests, browser profile save |
| Conflict and uncertain write handling | Configuration controller tests with lost responses and changed revisions |
| Activation retry identity | Controller unmount/remount, user/Team/member isolation, and duplicate receipt tests |
| Bounded retained history | Cursor-zero, pagination, abort, late-response, error, and detail interaction tests |
| Page independence | Fake ACP provider held while the application page closes, then finishes before page reopen |
| Web integration | Vitest, TypeScript, ESLint, build, and PR CI |

## Operational Notes

Access hints hide writable controls from observers; backend capabilities and Team ownership remain
mandatory for each mutation. Refresh after access or preflight changes. Existing ACP diagnostics
are the entry point for process-specific investigation.

The opt-in ignored `loop_browser_fixture` test serves a production-schema isolated database and
real API/SSE routes with a fake provider. It requires absolute `LOOP_UI_FIXTURE_DIR` and
`LOOP_UI_WEB_DIR` paths. `ready.json` contains synthetic test authentication and must stay local.
Create `browser-hold` in its provider directory to hold the next prompt, remove it to release, and
create `stop` in the fixture directory to terminate the server. No real user configuration or HOME
replacement is needed.

## Open Risks

- Browser evidence uses a deterministic local provider, not a live model or remote executor.
- Browser storage restrictions prevent retry identity retention across a full reload.
- Membership hints can lag changes; backend rejection remains authoritative.
- Wake pages are bounded observations, not a scheduling guarantee.

## Source Journals

- [Loop workspace UI](../journal/2026-09-17-loop-workspace-ui.md)
- [Offline configuration](../journal/2026-09-15-agent-loop-offline-configuration.md)
- [Durable history and diagnostics](../journal/2026-09-17-loop-history-storage.md)
