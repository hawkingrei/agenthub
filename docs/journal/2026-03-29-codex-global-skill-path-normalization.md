# Summary

Harden Codex skill-path handling so AgentHub-managed global skills continue to
resolve from the user home skill root, while repo-local `.agents/skills`
remains untouched.

# Why

We observed sessions attempting to open managed global skills via a current-workdir
relative path such as `.agents/skills/...`, which is only valid for repo-local
skills. AgentHub-managed global skills are materialized under the user home root
(`~/.agents/skills/agenthub-runtime/...`) and should not depend on the current
repository path.

# What Changed

- Taught `agenthub-codex-acp` to normalize two compatibility spellings for
  AgentHub-managed global skill wrappers before trust validation:
  - `~/...`
  - `.agents/skills/agenthub-runtime/...`
- Kept trust enforcement narrow: only paths that still resolve under the
  managed global root are converted into native Codex `UserInput::Skill` items.
- Left repo-local `.agents/skills/**/SKILL.md` untouched so workspace-owned
  skills continue to follow local discovery rules.
- Added focused tests covering:
  - `~/...` managed skill wrappers
  - legacy relative managed-global wrappers
  - repo-local relative skill paths staying as plain text

# Validation

Suggested validation:

- `cargo test -p agenthub-codex-acp build_prompt_items`

# Follow-up

- Continue treating the canonical outbound form for managed global skills as an
  absolute home-rooted path even though the Codex bridge now accepts the
  compatibility spellings above.
