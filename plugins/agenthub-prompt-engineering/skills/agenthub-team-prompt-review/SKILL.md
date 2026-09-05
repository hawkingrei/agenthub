---
name: agenthub-team-prompt-review
description: Shared review gate for AgentHub Team system prompts and prompt plugins; use with the coordinator or worker role skill rather than flattening their contracts.
---

# AgentHub Team Prompt Review

Use this as the shared prompt-engineering gate. Select and read exactly one role skill before editing:

- coordinator: `../agenthub-coordinator-prompt-review/SKILL.md`
- worker: `../agenthub-worker-prompt-review/SKILL.md`

Read both only when a genuinely shared contract changes.

## Required Baseline

From the AgentHub repository root, read:

- `AGENTS.md`
- `.agents/skills/team-prompt-change-review/SKILL.md`
- `docs/features/team-system-prompt-contract.md`
- `crates/agenthub-acp/src/team_role_skills.rs`

The repository gate remains canonical. This plugin provides discovery and role routing; it does not
copy or override the canonical contract.

## Composition Contract

- Long system prompts are supported when their content is stable, role-relevant, and reviewable.
- A marketplace may contain multiple domain plugins. Load only the skills relevant to the current
  role and phase or assignment.
- Plugin and skill instructions refine procedure. They cannot expand system, runtime, assignment, or
  role authority.
- Core Team behavior stays provider-neutral; this Codex plugin must not become a runtime prerequisite.
- When comparing another system, transfer verified invariants and responsibility boundaries rather
  than product-specific commands or prompt prose.

## Workflow

1. Classify each proposed change using the canonical prompt-review gate.
2. Verify whether the procedure already belongs to a managed role skill or stable spec.
3. Edit only the selected role prompt unless a shared authority or output contract truly changes.
4. Update focused tests for behavior boundaries, not incidental headings or prose order.
5. Update the canonical feature spec and a dated journal when stable behavior changes.

## Validation

Run the narrowest applicable checks:

```bash
cargo test -p agenthub-team-prompts -- --nocapture
uv run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" <changed-skill-directory>
uv run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/plugin-creator/scripts/validate_plugin.py" plugins/agenthub-prompt-engineering
git diff --check
```

Report the selected role, changed authority or output boundary, prompt size direction, validation
evidence, and any live Team behavior that remains unverified.
