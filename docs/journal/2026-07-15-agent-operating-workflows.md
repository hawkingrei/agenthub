# 2026-07-15 Agent Operating Workflows

## Summary

Added a project-owned workflow organization contract for repository prompts, SOPs, skills,
checklists, testing evidence, and observability evidence.

## Background

The repository already has short root agent instructions, Team runtime prompts, feature specs,
journals, TODO, and local skills. The missing contract was how to decide which surface should own a
repeated engineering workflow without copying another project's process taxonomy or expanding every
prompt.

## Scope

- `AGENTS.md`
- `docs/features/agent-operating-workflows.md`
- `docs/features/README.md`
- `docs/architecture-map.md`

## Key Decisions

- Use project-owned workflow types: `SOP`, `skill`, and `checklist`.
- Keep the repository tool-neutral and avoid naming private memory systems as required
  dependencies.
- Treat testing workflow as protected object, invariant, terminal oracle, boundary proof, and
  validation layer.
- Treat observability workflow as acceptance surface, minimal evidence, failure-layer
  classification, narrow diagnostic surface, and proof boundary.
- Promote repeated workflows to skills/checklists only when the steps are stable enough to be useful.

## Validation

```bash
git diff --check
```

## Follow-Ups

- Promote the highest-frequency workflows into project-owned skills or checklists as they next need
  maintenance: PR review follow-up, CI triage, runtime stuck diagnosis, release artifact
  verification, and prompt/skill update review.
