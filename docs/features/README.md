# Feature Docs Standard

This directory stores feature-oriented technical documents, not per-PR journals.

## Goal

Keep one durable technical document per feature area as the source of truth for:
- problem statement and scope
- architecture and runtime contracts
- configuration and operational constraints
- validation matrix and open risks

## Required Structure

Each active feature document should contain:
- `Problem`
- `Scope`
- `Non-Goals`
- `Architecture`
- `Contracts` (API/schema/runtime behavior)
- `Validation Matrix`
- `Operational Notes` (rollout, compatibility, failure handling)
- `Open Risks`
- `Superseded Notes` (optional, when merged from older notes)

## Writing Rules

- Prefer stable technical language over timeline/journal narration.
- Avoid embedding long command transcripts unless required for reproducibility.
- Keep implementation details tied to interfaces and invariants, not commit history.
- Use explicit file references for critical contracts.

## Compaction Policy

When 3+ notes describe the same feature area:
1. Merge them into one canonical spec document.
2. Replace old notes with short `Superseded by ...` pointers.
3. Update `docs/todo.md` references to the canonical spec document.
4. Keep only one active verification source per feature area.

## Naming

- Keep the existing `YYYY-MM-DD-topic.md` naming.
- Use topic names that describe feature domains (for example, `team-role-skill-runtime-spec`) rather than PR mechanics.
