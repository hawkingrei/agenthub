# Summary

A code-only backend correctness review found that `safe_paths` -- the admin-configured allowlist meant
to restrict where agent processes can run -- was never actually checked against an agent's `workdir`.
`is_path_allowed` (the allowlist-membership check) was only ever called for ACP skill-file loading; the
`workdir` supplied when creating an agent, or when adopting an existing agent into a Team with a
workspace-content copy destination, was accepted unchecked and used directly as the spawned process's
`cwd`. Any account holding `AgentsManage` (an operator-level, non-root capability) could set `workdir` to
an arbitrary filesystem path and fully bypass the allowlist an operator configured via `/admin/safe_paths`.

# Background

`crates/agenthub-config/src/path_utils::is_path_allowed` already existed with correct `..`/`.` lexical
normalization, tested and used by `crates/agenthub-acp`'s skill-loading path
(`is_skill_path_allowed`). It was never wired into the agent-creation code paths that actually decide
where a process runs. `is_skill_path_allowed` fails closed when `safe_paths` is empty (nothing is
allowed); applying that same default to `workdir` validation would mean every install that has never
configured `safe_paths` -- which today is effectively every existing deployment, since this check never
enforced anything for `workdir` before -- would immediately be unable to create any agent, until an
operator added at least one allowlist entry. That's a hard breaking change for a security fix that closes
a gap nobody has been relying on being closed. Confirmed with the person requesting this fix: an empty
`safe_paths` list should continue to mean "no restriction configured yet," matching every existing test
and deployment; the allowlist becomes enforced only once at least one entry exists.

# Scope

- `src/api/agents.rs`: new `pub(super) async fn ensure_workdir_within_safe_paths`, called in
  `create_agent` right after `resolve_create_agent_workdir` resolves the final workdir (whether explicit
  or auto-generated under `default_worktree_root`). No-op when `safe_paths` is empty; otherwise rejects
  with `403 Forbidden` if the resolved workdir isn't inside any configured entry.
- `src/api/teams.rs`: the same check, called in `adopt_existing_agent_to_team` -- but only when the
  caller supplies an explicit `workspace_copy_destination`. When no destination is given, `workdir` falls
  back to the source agent's already-existing workdir, which is not re-validated: retroactively enforcing
  the allowlist on agents that already existed before this fix would be a separate, larger migration
  decision, not part of closing this specific bypass. The check runs before `copy_adoption_workspace`
  actually copies files, so a rejected request never touches the filesystem.

# Key Decisions

- Empty `safe_paths` = no restriction (not fail-closed), by explicit confirmation, to avoid a breaking
  change for every deployment that has never configured the allowlist. This intentionally diverges from
  `is_skill_path_allowed`'s fail-closed default for the same underlying check function -- the two call
  sites have different blast radii (skill loading is best-effort and narrow; agent creation is core
  product functionality) and different pre-existing behavior to stay compatible with.
- Only the *newly supplied, untrusted* input is validated. `adopt_existing_agent_to_team`'s fallback to
  `source.workdir` when no destination is given is deliberately left unchecked, so adopting an
  already-existing agent (created before this fix, potentially outside any configured safe_paths) doesn't
  suddenly start failing.
- Did not address the secondary, lower-severity finding from the same review: `is_path_allowed` resolves
  `..`/`.` lexically, not via filesystem canonicalization, so a symlink planted inside an allowed
  directory that points outside it could still defeat the check. Canonicalization requires the path to
  exist on disk, which doesn't hold for workdirs about to be created (`worktree_mode: create_worktree`),
  so this needs its own design pass, not a drop-in change alongside this fix.

# Validation

- `cargo test --lib api::agents` -- 59 passed (2 new: rejects a workdir outside a configured allowlist
  with `403` and the exact error message; allows any workdir when no `safe_paths` are configured at all).
- `cargo test --lib api::teams` -- 103 passed (1 new: `adopt_existing_agent_to_team` rejects a
  `workspace_copy_destination` outside a configured allowlist, and confirms the source agent is left
  unowned by any team -- the rejected adoption did not partially apply).
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- Symlink/canonicalization gap in `is_path_allowed` (see Key Decisions) -- needs a design decision on how
  to canonicalize a path that may not exist yet.
- The other three findings from the same review round (unbounded gRPC client cache, remote-relay
  panic-poisoning trap, panic landmine on non-object `context_json`) are tracked separately and not
  addressed in this change.
