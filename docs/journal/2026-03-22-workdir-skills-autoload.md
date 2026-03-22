---
title: Workdir Skills Autoload for ACP Sessions
date: 2026-03-22
status: implemented
---

## Summary

ACP sessions now auto-discover project-local skills from
`<workdir>/.agents/skills/**/SKILL.md` in addition to the existing builtin
skills and user-level `~/.agenthub/skills.json` config.

## Decision

- Keep builtin runtime/team skills unchanged.
- Discover repo-local skills under the current ACP workdir only.
- Continue enforcing `safe_paths` on every discovered `SKILL.md`.
- Load repo-local skills before `~/.agenthub/skills.json` so project-local
  definitions win when names collide.
- Keep discovery narrow: only files named `SKILL.md` are loaded.
- Skip symlinked entries during recursive discovery so `.agents/skills` cannot
  recurse through symlink loops.

## Validation

- `cargo test -p agenthub-acp load_workdir_skills_discovers_nested_skill_markdown_files -- --nocapture`
- `cargo test -p agenthub-acp load_workdir_skills_uses_parent_directory_name_as_fallback -- --nocapture`
- `cargo test -p agenthub-acp repo_local_skills_take_precedence_over_global_config_skills -- --nocapture`
