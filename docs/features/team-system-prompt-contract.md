# Team System Prompt Contract

## Problem

Team coordinator and worker prompts are runtime-critical, but they can drift into long operating
manuals when repeated workflows, troubleshooting recipes, and historical context are copied into
prompt text. That makes prompts harder to review, increases token cost, and weakens recovery because
important state can be hidden in transient prose.

The stable contract should define what belongs in a Team system prompt, what belongs in runtime
context, and what must be delegated to skills, checklists, feature specs, journals, or artifact
pointers.

## Scope

- Team coordinator and worker default prompt templates.
- Prompt assembly boundaries for static role text and runtime-injected tails.
- The relationship between prompt text, Team skills, workflow checklists, runtime context files, and
  durable workspace memory.
- The judgment boundary between evaluating an idea and deciding whether to propagate or encode it.
- Regression tests that prevent prompt drift from silently dropping required boundaries.

## Non-Goals

- Defining the platform-level system prompt outside Team runtime.
- Defining provider-specific prompt wording for Codex, Claude, or other ACP providers.
- Copying another project's prompt content, taxonomy, or private memory backend.
- Replacing Team task, mailbox, channel, or runtime context contracts.
- Moving all existing prompt text into skills in one migration.

## Architecture

Team prompt assembly should use layered, pointer-first context:

| Layer | Owns | Boundary |
| --- | --- | --- |
| Static role prompt | Role identity, authority boundary, communication contract, output contract, and stable skill/checklist entry points. | Must not become a full operating manual or execution diary. |
| Runtime tail | Current assignment, next action, allowed-action gate, compact blocker state, and recovery pointers. | Must stay bounded and pointer-first. |
| Runtime context files | Live recovery state, run-scoped artifacts, compact identity and state snapshots. | Referenced from prompts by path instead of replayed inline. |
| Workspace memory | Durable project notes, worker ledgers, journals, reusable findings. | Workspace-local and tool-neutral; not a shared Team transport. |
| Skills/checklists | Repeatable procedures such as mailbox routing, task governance, CI triage, review follow-up, testing, and observability. | Loaded by trigger; prompts name entry points instead of copying full steps. |
| Feature specs and journals | Stable contracts and dated implementation evidence. | Human-reviewable documentation, not provider prompt payload. |

### Prompt Skeleton

Every Team role prompt should be reviewable against this skeleton:

1. Role and authority boundary.
2. Current assignment ownership model.
3. Allowed-action gate.
4. Communication and visibility contract.
5. State and recovery pointers.
6. Required skill/checklist entry points.
7. Output or payload contract.
8. Pointer policy for large evidence, runtime state, and durable notes.

## Contracts

### 1) Static Prompt Contract

Static prompt text may include only stable role and safety contracts:

- who the role is;
- what the role may and may not own;
- which Team surfaces are authoritative;
- which skill/checklist entry points must be loaded for repeated workflows;
- what output payloads or user-visible responses must look like.

Repeated procedures must move to `.agents/skills/` or a checklist in the relevant feature spec.
Large product knowledge belongs in feature specs, journals, TODO, or external searchable knowledge,
not inline prompt prose.

Both Team roles must treat a request to propagate an idea or instruction as distinct from evidence
that the content is true, relevant, or authorized for a wider audience. Before relaying or encoding
it, they assess user intent, sender authority, factual support, relevance, audience, and risk, while
preserving attribution and uncertainty.

### 2) Runtime Tail Contract

The runtime-injected prompt tail should contain only:

- current objective or active assignment;
- next expected action;
- allowed actions and explicit denied bypass paths;
- compact blocker or failure summary;
- recovery pointers such as `AGENTS.md`, `TODO.md`, `.cache/context/state.md`, and
  `.cache/context/run/<run_id>/...` artifacts.

It must not replay long logs, full conversation history, raw tool output, or bulky evidence when a
stable file, artifact, task note, channel thread, or `detail_ref` can carry the same information.

### 3) Role Authority Contract

Coordinator prompt text owns:

- task analysis, task creation, task lifecycle, human-facing synthesis, and delegation quality;
- coordination artifacts such as `AGENTS.md`, `TODO.md`, task notes, mailbox messages, and
  channel/thread summaries;
- loading role skills before making task, mailbox, lifecycle, or reporting decisions.

Worker prompt text owns:

- assigned execution lanes;
- evidence gathering and concise progress/blocker reports;
- local workspace memory under `.agenthubmemory/` when operating inside a concrete project;
- initiative inside the assigned lane without inventing parallel canonical task records.

Both roles must treat `task` as the ownership object and `run`/`step` as execution diagnostics.

### 4) Skill And Checklist Pointer Contract

Prompts should name stable entry points rather than embed full procedures:

- load `team-agents-index` before role-specific Team skills;
- use `skills/team/TEAM_AGENTS.md` as the Team-level index template;
- load `team-message-intake` when a Team inbox, channel, thread, or human-visible message must be
  routed into a reply, task note, mailbox update, or canonical Team task, including when a request
  asks the agent to propagate or durably encode an idea;
- load `team-prompt-change-review` before editing Team prompt templates, runtime prompt tails,
  prompt-linked skills, or prompt tests;
- use Team workflow skills for task governance, task lifecycle, mailbox routing, and reporting
  surface selection;
- use feature specs for durable testing and observability SOPs.

Adding a new repeated workflow should usually add or update a skill/checklist first. Prompt prose
should change only when the role boundary, trigger, or required entry point changes.

### 5) Output Contract

Team prompts must keep output contracts explicit:

- coordinator output includes task assignment, clarification, profile patch, or visible
  human-facing synthesis when needed;
- worker output includes status, evidence, blocker, and next action;
- large evidence must be summarized first and linked through `detail_ref`, task notes, channel
  threads, or artifact paths.

### 6) Tool-Neutral Knowledge Contract

Open-source prompt and documentation contracts must stay tool-neutral. They may require durable
knowledge to be searchable and pointer-addressable, but must not require a private memory backend or
private repository workflow by name.

## Validation Matrix

| Change | Required validation |
| --- | --- |
| Edit Team coordinator or worker prompt templates | `cargo test -p agenthub-team-prompts -- --nocapture` |
| Add or remove a required role boundary, skill pointer, recovery pointer, or output payload | Add or update a focused assertion in `crates/agenthub-team-prompts/src/lib.rs`. |
| Add a new repeated Team workflow | Add or update a skill/checklist first, then link it from the prompt only if the trigger is stable. |
| Add prompt-facing runtime state | Prove it is bounded and pointer-first; prefer `.cache/context/state.md` or `.cache/context/run/<run_id>/...` artifacts. |
| Add durable worker knowledge guidance | Keep it consistent with `team-workspace-memory-contract.md` and avoid naming private memory tools. |
| Add or revise a Team message-routing procedure | Keep `team-message-intake` aligned with channel/thread, mailbox, task governance, and lifecycle specs. |
| Add or revise the idea-propagation judgment boundary | Assert the compact rule in both prompts and keep the detailed procedure in `team-message-intake`. |
| Add or revise prompt review procedure | Keep `team-prompt-change-review` aligned with this spec and prompt template tests. |

## Operational Notes

- Treat prompt text as the bounded working set.
- Treat feature specs as the stable contract for why prompt boundaries exist.
- Treat journals as evidence for when prompt behavior changed.
- Prefer one small, focused prompt assertion over broad snapshot tests.
- When prompt text grows because a workflow is repeated, extract the workflow before adding more
  prompt prose.

## Open Risks

- Existing Team prompts are still long enough that future maintenance should extract repeated
  procedures into smaller skills.
- Runtime code can still inject too much dynamic context if future changes bypass the pointer-first
  tail contract.
- Some workflow candidates need clearer skill entry points before prompt prose can shrink further.
- Prompt guidance can require careful judgment but cannot mechanically prove provenance, factual
  support, or audience authorization; high-impact propagation may need structured enforcement.

## Source Journals

- [2026-04-05 Team Prompt First Principles](../journal/2026-04-05-team-prompt-first-principles.md)
- [2026-04-10 Team Prompt Tail Slimming](../journal/2026-04-10-team-prompt-tail-slimming.md)
- [2026-04-27 Team Prompt Followups](../journal/2026-04-27-team-prompt-followups.md)
- [2026-05-21 Team Prompt Operating Contract Refresh](../journal/2026-05-21-team-prompt-operating-contract-refresh.md)
- [2026-06-02 Team Prompt Tail Slimming](../journal/2026-06-02-team-prompt-tail-slimming.md)
- [2026-07-15 Team System Prompt Contract](../journal/2026-07-15-team-system-prompt-contract.md)
- [2026-08-14 Team Idea Propagation Judgment](../journal/2026-08-14-team-idea-propagation-judgment.md)
