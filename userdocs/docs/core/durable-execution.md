---
sidebar_position: 8
---

# Durable Team Execution

Durable execution activates an offline Team member when eligible work arrives. Each activation
loads current tasks and messages, receives one configured role prompt, uses tools, and records an
outcome before its processes are cleaned up. Closing the browser does not stop this work.

## Supported Boundary

Enable this workflow explicitly for a Team using `execution_mode: loop`. The initial boundary is
local Linux execution with a supported ACP provider and a process guardian. Remote Agent Nodes and
non-Linux hosts retain their existing manual workflows; they do not have equivalent loop recovery.

| Provider path | Runtime profile | Continuity | Qualification |
| --- | --- | --- | --- |
| Codex through `agenthubd acp codex` | ACP model, reasoning effort, and approval mode | Fresh; resume when `loadSession` is advertised and loading succeeds | Official Codex CLI 0.150.1 with the matching adapter; local scripted model and real runtime processes |
| Claude ACP | Spawn-time model and thinking configuration | Fresh; capability-gated resume | Existing adapter path; separate installed-runtime qualification required |
| Gemini and Kimi ACP | Provider defaults; per-agent model/thinking overrides rejected | Fresh; capability-gated resume | Existing adapter paths; separate installed-runtime qualification required |

Use the exact Codex version expected by the installed adapter. A compatible command name alone does
not establish runtime compatibility. The local qualification proves protocol and process behavior;
it does not measure a model's ability to solve an engineering task.

## Configure And Enable

1. Create local agents with explicit workspaces, then add them to a loop Team as coordinator and
   workers. Keep worker workspaces isolated. Configure Agent Cards, role prompts, and tool bindings
   while the members are offline.
2. Open the Team member's execution controls. Review the preflight result before choosing
   **Enable execution**. Resolve workspace, provider, ownership, or tool authorization blockers.
3. Use **Fresh** initially. Set an activation budget and time window, then choose
   **Save execution settings**. The backend enforces actor and Team limits together.
4. Choose **Activate member** for an explicit trigger, or send eligible mailbox/task work. A repeated
   source identity is idempotent; distinct sources can be grouped into one pending activation.
5. Inspect the activation's sources, outcome, session, events, and tool journal. Review canonical task
   notes separately. A worker's result or successful process exit does not accept a task; the
   coordinator reviews the evidence and records acceptance.

For automation, the OpenAPI routes under `/api/teams/{team}/members/{actor}/loop` expose policy,
preflight, activation, history, and metrics. Policy updates require the current `expected_revision`;
reload after a conflict. Use the documented schema rather than copying stale policy revisions.
See [OpenAPI and Automation](../advanced/openapi-and-automation.md).

## Fresh, Resume, And Waiting

Fresh sessions recover current work from durable tasks, messages, source records, and scoped Mem.
The logical member and mailbox partition remain stable across new provider sessions.

**Resume when supported** requests provider continuity. A missing capability, rejected load, or
incompatible launch configuration is an explicit failure; the scheduler does not silently replace
a required resumed session with a new one. Change session policy only after execution ownership has
been released. Current durable work is read again even when provider history is resumed.

An activation may finish with progress, handoff, no actionable work, or a durable wait. A native ACP
permission prompt belongs to a live provider session. Cancellation or session loss invalidates that
callback. A business decision that must survive exit belongs in canonical messages/tasks and a
durable wait, not in an old permission button.

## Scoped Mem

Configure a daemon-side profile and bind it to one Team space. For example:

```toml
[nowledge_mem.profiles.engineering]
endpoint = "https://mem.example.com/mcp"
credential_env = "TEAM_MEM_KEY"
tool_set = "external-agent"

[nowledge_mem.team_bindings."TEAM_ID"]
profile = "engineering"
space_id = "SPACE_ID"
```

Supply `TEAM_MEM_KEY` to the daemon's environment. The key must be narrowed to exactly the bound
space and have that live write target. Invalid configuration, rejected credentials, or a mismatched
binding block launch. A temporary authorization-probe outage leaves Mem unmounted and permits
independent local work. The provider receives a local MCP shim only for a verified binding;
upstream credentials remain in the daemon.

Every fresh or resumed activation requests a current Context Lens before its entry prompt. The
bundle remains attributed data. Once authorization is valid, a Mem transport outage records context
unavailability and permits independent local progress. Scope denial is not treated as an outage.
Tools declaring `space_id` are bound to the selected space; a different explicit scope is rejected.

## Registered Apps And Events

App setup is API-managed. An instance administrator registers a connection and versioned manifest;
the App owner approves Team scopes, and the Team owner binds a member to an explicit version and
scope subset. Registration alone does not authorize execution. Follow the request schemas in
OpenAPI for `/api/apps` and the Team/member App routes.

An activation pins its selected manifest version. Revoking the App, Team grant, or member binding
denies later calls, including calls from an already running session. Upstream credentials and event
signing keys stay in the daemon. A separate event route and active signing key are required for
signed App events to wake a member. Duplicate event identities produce one logical intake record.

Read-only calls and explicitly keyed writes have different retry contracts. If a write may have
reached its upstream but its result was lost, its journal remains unknown. Inspect that upstream
before deciding what new work to request; restarting the daemon does not make replay safe.

## Suspend, Resume, And Recover

**Suspend execution** prevents new admission and retains pending work. It does not automatically
cancel an activation already running. Use the current execution's stop/cancel control when that is
also required. **Resume execution** permits eligible retained work to enter admission again.

Resuming does not reset activation budgets or the consecutive no-progress limit. A completed
activation must reference its new canonical task note through `task_note_id` in the structured
outcome to establish progress. Writing a note without linking it, or reusing old evidence, does not
reset that limit. Inspect admission events when retained work remains pending after resuming.

After daemon restart, expired ownership is reconciled automatically. A reservation that never
authorized a spawn can be released transactionally. A guarded execution requires exclusive access
to its matching durable cleanup evidence, proving that the previous guardian cannot still own
descendants. Recorded outcomes, tasks, messages, and external-effect uncertainty survive recovery.

If the UI reports that previous execution still needs verified cleanup, admission remains blocked.
Missing, corrupt, legacy, or still-locked evidence is insufficient. Preserve the database and event
store, inspect the activation and daemon diagnostics, and establish why the guardian did not finish.
Do not delete reservation rows, remove evidence files, or clear the blocker merely because a PID is
absent. A killed guardian may require operational investigation; there is no unsafe force-release
button.

Keep the event-store directory persistent alongside the control database. It contains private
`.executor-recovery` evidence. Back up or restore the database and evidence together only after
quiescing execution. Copying one side of active ownership is not a supported recovery procedure.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Enable is blocked | Preflight reason, local workspace, Linux guardian support, and tool scope |
| Launch rejects configuration | Exact adapter/runtime version and the selected model's supported reasoning levels |
| Required resume fails | Provider `loadSession` capability, persisted session, and unchanged launch contract |
| Pending work does not start | Suspension, due time, budgets, live ownership, and retained cleanup blocker |
| Mem context is unavailable | Daemon-side endpoint and credential binding; distinguish outage from scope denial |
| App call is denied | Pinned version, current grant/binding epochs, revocation, and required scopes |
| Write result is unknown | Operation journal and upstream effect; do not assume it was never sent |
| Browser reconnects without live output | Open retained activation/session history and check the SSE connection |

See [Connection Status and Recovery](../advanced/connection-status-and-recovery.md) for browser
transport issues and [Troubleshooting](../operations/troubleshooting.md) for general operations.
