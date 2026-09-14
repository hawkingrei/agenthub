# Agent Loop Product Definition

## Summary

Redefine AgentHub as an agent loop toolchain: one role prompt per activation, shared leader/worker
execution, Agent Cards as startup inputs, temporary processes, and Nowledge Mem integration.
This checkpoint changes product/design documentation, not runtime behavior.

## Background

The current idle watchdog requires an existing ACP handle. Reminders explicitly do not start stopped
agents. Team docs also retained process-bound identity, manual-restart assumptions, and a fixed
six-phase workflow. Mem has scope/error/journal policy helpers but no complete proxy integration.

## Scope

- Add canonical [product](../features/agent-loop-product-model.md) and
  [runtime](../features/agent-loop-runtime.md) designs.
- Align Team, prompt, memory, startup, profile, ACP, and workspace contracts with explicit migration
  boundaries. Preserve current wire identifiers and behavior as compatibility where necessary.
- Specify activation-centric observability and a durable per-activation trace, extending the
  existing runtime-diagnostics foundations, and align the direct Rara integration with the loop
  execution boundary.
- Add the [app tool registration](../features/app-tool-registration.md) target design: external
  apps declare tools through versioned manifests and deliver signed events, reaching agents only
  through operator bindings and the shared enforcement proxy.
- Update charter, navigation, README, and user-facing overview pages without advertising target
  behavior as delivered.
- Track staged implementation in [TODO](../todo.md#agent-loop-product-transition).

## Key Decisions

- Keep `coordinator` as the compatible identifier for the leader role.
- Persist identity, tasks, IM, outcomes, and continuation independently of process lifetime.
- Use one shared execution mechanism; role prompts guide planning and execution through tools.
- Separate loop outcome, task acceptance, message consumption, and verified process cleanup.
- Keep task/transport state authoritative locally and knowledge scoped in Mem.
- Do not silently reinterpret idle-watchdog settings as permission to start offline agents.
- Session reuse default and trigger cadence remain implementation choices; fresh-session recovery
  must be possible. No user preference for those defaults was assumed.
- The activation id is the correlation spine for metrics, spans, and `agenthub doctor agent-trace`;
  traces are durable, redacted, and reconstructable without the exited process.
- Rara participates as one provider adapter behind the same scheduler: activation identity stays
  separate from Rara thread continuity, handshake capabilities gate durable-wait claims, and
  semantic guard results map to loop outcomes rather than failures.
- Agents are trigger sources, not process schedulers: a leader delegates by creating durable
  triggers — assignment, mentions, due-time/dependency wakes, bounded standing triggers — under
  the same admission, suspension, and budget rules as user triggers, with trace attribution.
  Roster changes stay within the existing adoption flows and operator policy.
- Tool surfaces are extensible by registration, not by runtime changes: manifest-declared app
  tools and signed app events pass one shared enforcement proxy with call-time scope rejection,
  fail-closed unavailability, and no task/IM/scheduling authority. MCP is the canonical
  invocation protocol; app events are notifications that enter normal trigger intake.

## Validation

Classification: documentation-only. Runtime prompt templates, injected tails, role skill/plugin
entry points, schemas, and executable code are unchanged.

Checks supporting this change:

- `git diff --check`: no whitespace errors.
- Read-only Markdown check: 30 changed documents, 253 relative file links, and 10 heading links;
  all targets resolve. New product/runtime specs and revised Mem/playbook specs contain all required
  sections.
- `npm --prefix userdocs run build`: static production output generated successfully. The local
  update-notifier configuration warning did not fail the build.
- Review target/current boundaries against `spawn_agent_loop_controller`, reminder delivery,
  process exit recording, and `agenthub-acp-core/src/nowledge_mem.rs`.

These checks cannot establish lifecycle recovery, provider execution, or working Mem transport.
Their acceptance matrices belong to the subsequent implementation slices.

## Follow-Ups

Implement the staged lifecycle, tool/Mem integration, and UI work in the canonical TODO. Concrete
activation persistence, run/attempt mapping, credential delivery, and provider permission recovery
need explicit implementation designs and runtime evidence.
