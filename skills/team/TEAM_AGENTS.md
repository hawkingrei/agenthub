# Team AGENTS Index Template

Use this template as the team-level runtime index in leader workspace `AGENTS.md`.
Keep it concise and route detailed procedures to skill files.

## Run Objective

- primary objective:
- success criteria:

## Current Phase

- current phase: `team formation|task analysis|role assignment|communication and collaboration|consensus formation|result integration`
- transition condition:

## Human Input

- goals and constraints from human actor:
- priority and acceptance preference:

Note:
- humans provide goals/constraints via conversation;
- internal tasks are created by leader planning, not directly by human.
- routing contract: `@member_id` means directed recipients; no `@` means broadcast.
- delivery contract: conversation is persisted once, then forwarded through actor mailbox transport.

## Team Formation

- member roster:
- role coverage:
- capability gaps:

## Task Analysis

- in-scope:
- out-of-scope:
- risks:
- assumptions:

## Role Assignment

- leader responsibilities:
- worker assignments:
- acceptance criteria per assignment:
- deadlines:

## Skill Routing

- leader orchestration: `skills/team/team-leader-orchestrator.SKILL.md`
- worker execution: `skills/team/team-worker-executor.SKILL.md`
- deliberation rules: `skills/team/team-deliberation-rules.SKILL.md`
- actor mailbox protocol: `skills/team/team-actor-mailbox.SKILL.md`

## Identity Cards

- member identity source: `spec.members[].description`
- runtime check endpoint: `/api/agents/:id/.well-known/agent-card`
- owner of identity updates:

## Communication Log

- checkpoint notes:
- blocking questions:
- escalation events:

## Consensus Decisions

- accepted options:
- rejected options and reasons:

## Result Integration

- merged outcomes:
- evidence pointers:
- human-facing summary draft:

## Open Risks

- unresolved risks:
- mitigation or fallback:

## Next Checkpoint

- checkpoint time:
- expected deliverables:
