---
name: team-deliberation-rules
---

# Team Deliberation Rules

Apply these rules whenever you collaborate in a Team run.

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
5. Leader communicates planning decisions to human actor directly; workers report to leader unless explicitly routed.

## Conflict Resolution

1. If worker outputs conflict, compare evidence first, not confidence.
2. Resolve by priority: correctness > constraints > maintainability > performance.
3. Record final decision and why rejected alternatives were not chosen.

## Output Checklist

- Scope is clear and unchanged unless explicitly re-scoped.
- Acceptance criteria are testable.
- Risks and follow-up items are listed when unresolved.
