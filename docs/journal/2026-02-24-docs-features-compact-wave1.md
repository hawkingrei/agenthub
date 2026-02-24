# 2026-02-24 Docs Features Compact Wave-1

## Context

`docs/features` accumulated many Team role-skill notes with overlapping scope and timeline-style narration. This increased drift risk and made verification references harder to maintain.

## Goal

- move feature docs toward stable technical specs
- reduce duplicate notes in the Team role-skill area
- keep one canonical reference for active verification items in `docs/todo.md`

## Changes

1. Added feature-doc standard:
   - `docs/features/README.md`
   - defines required sections for active feature specs and compaction policy

2. Added canonical Team role-skill runtime spec:
   - `docs/journal/2026-02-24-team-operating-model-spec.md`
   - consolidates role-skill injection pipeline, single-mode isolation, bootstrap/safe-path policy, cold-start TODO workflow, TODO lifecycle, and AGENTS/SKILL ownership boundary

3. Compacted six overlapping notes into superseded pointers:
   - `docs/journal/2026-02-17-team-skills-bootstrap-script.md`
   - `docs/journal/2026-02-18-team-deliberation-rules-skill.md`
   - `docs/journal/2026-02-19-team-role-skill-acp-auto-injection.md`
   - `docs/journal/2026-02-20-team-single-node-skill-bootstrap.md`
   - `docs/journal/2026-02-22-team-role-skill-single-mode-isolation.md`
   - `docs/journal/2026-02-23-team-cold-start-skill-and-ui-playbook.md`

4. Updated `docs/todo.md` references:
   - switched relevant verification items to the canonical spec
   - added a follow-up compact backlog item for wave-2

## Validation Notes

- Documentation-only change set.
- Performed reference sanity check to ensure Team role-skill TODO entries now point to the canonical spec.

## Follow-up

- Wave-2 compaction target: merge remaining Team UI extraction/reducer phase notes into domain-level canonical feature specs.
