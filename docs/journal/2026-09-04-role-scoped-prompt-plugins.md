# Role-Scoped Prompt Plugins

## Summary

Added the first repository-owned Codex plugin marketplace entry for AgentHub prompt engineering and
made plugin composition an explicit, role-scoped Team prompt contract.

## Background

A source review of Raft/Slock prompt construction showed useful invariants: runtime identity should be
authoritative, instructions should distinguish authority from procedure, and reusable operating
knowledge should have a canonical source instead of drifting across prompt copies. AgentHub already
materializes provider-neutral managed skills, while its coordinator and worker prompts remain
independently editable role contracts.

The comparison used the local `origin/staging` snapshot at
`81cb2ada771b2cd86d5eac325790039adb357fed`; the relevant prompt files were unchanged from the older
checked-out branch. Remote access was unavailable, so this is source-snapshot evidence rather than a
claim about the current remote head.

## Scope

- Added a repository marketplace that can list multiple Codex plugins.
- Added `agenthub-prompt-engineering` with a shared review gate and separate coordinator and worker
  role skills, avoiding duplicated procedures without flattening role contracts.
- Added a compact extension-authority rule to both default Team prompts.
- Replaced the old line-count assertion with a generous byte ceiling so long stable prompts remain
  supported while accidental unbounded growth still fails review.
- Updated the prompt and workflow specs with the multi-plugin and provider-neutrality boundaries.

## Key Decisions

- Long system prompts are supported; prompt length is a review signal, not an optimization target.
- Coordinator and worker prompts stay separately editable.
- A plugin may provide role-specific skills, and the marketplace may contain several plugins.
- Plugins refine procedures but never expand system, runtime, assignment, or role authority.
- The Codex plugin is an optional maintenance/discovery surface. Core Team behavior continues to rely
  on provider-neutral prompts and managed skills.
- External prompt systems are reference evidence only; AgentHub adopts responsibility boundaries and
  invariants rather than copying product-specific prose.

## Validation

```bash
cargo test -p agenthub-team-prompts -- --nocapture
uv --cache-dir /private/tmp/agenthub-prompt-uv-cache run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" .agents/skills/team-prompt-change-review
uv --cache-dir /private/tmp/agenthub-prompt-uv-cache run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" plugins/agenthub-prompt-engineering/skills/agenthub-team-prompt-review
uv --cache-dir /private/tmp/agenthub-prompt-uv-cache run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" plugins/agenthub-prompt-engineering/skills/agenthub-coordinator-prompt-review
uv --cache-dir /private/tmp/agenthub-prompt-uv-cache run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" plugins/agenthub-prompt-engineering/skills/agenthub-worker-prompt-review
uv --cache-dir /private/tmp/agenthub-prompt-uv-cache run --with pyyaml python "${CODEX_HOME:-$HOME/.codex}/skills/.system/plugin-creator/scripts/validate_plugin.py" plugins/agenthub-prompt-engineering
git diff --check
```

## Follow-Ups

- Add another domain plugin only when it has a concrete, non-overlapping trigger and maintained
  content; do not create placeholder catalog entries.
- Validate the role-specific plugin flow in a fresh Codex thread after installing the repository
  marketplace.
