---
name: team-deliberation-rules
---

# Team Deliberation Rules

Apply these rules whenever you collaborate in a Team run.

## Shared Workflow Phases

Every discussion and decision should map to one of these phases:

1. Team formation
2. Task analysis
3. Role assignment
4. Communication and collaboration
5. Consensus formation
6. Result integration

Phase rules:
- Always include `current_phase` in high-value internal status updates.
- Do not skip directly from `Task analysis` to `Result integration`; pass through assignment and collaboration.
- Enter `Consensus formation` only when required evidence is present from responsible members.

## Decision Principles

1. State assumptions explicitly before proposing implementation.
2. Prefer bounded, testable steps over broad open-ended changes.
3. When multiple options exist, present 1-3 options with tradeoffs.
4. Default to the safest reversible path when risk is unclear.
5. Treat missing requirements as blockers and ask focused clarification.

## Communication Contract

1. Use deterministic payloads and avoid ambiguous language.
2. Separate facts, inferences, and proposals.
3. Include concrete evidence for claims (file path, test name, command).
4. Report blockers with a specific unblock request and next action.
5. Shared routing and human-facing reply boundaries follow `skills/team/AGENTS.md`.
6. During `Consensus formation`, summarize accepted and rejected options with reasons.
7. During `Result integration`, ensure output is unified and non-contradictory before sending to human actor.
8. Keep `AGENTS.md` as index-level communication; attach deep implementation guidance in relevant skills/docs.
9. Keep `current_phase`, structured evidence fields, and transport metadata inside mailbox/artifact
   updates; shared-channel text should stay concise and human-readable.

## Conflict Resolution

1. If worker outputs conflict, compare evidence first, not confidence.
2. Resolve by priority: correctness > constraints > maintainability > performance.
3. Record final decision and why rejected alternatives were not chosen.

## Output Checklist

- Scope is clear and unchanged unless explicitly re-scoped.
- Acceptance criteria are testable.
- Risks and follow-up items are listed when unresolved.
- Current phase and transition condition are explicit.
- TODO status is consistent with actual execution (`completed` only after acceptance evidence).
