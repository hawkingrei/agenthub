# Summary

`safe_paths` -- the admin-configured workdir allowlist added in
[2026-02-15-safe-path-default-worktrees.md](2026-02-15-safe-path-default-worktrees.md) and later wired
into actual enforcement in
[2026-08-16-safe-paths-workdir-enforcement.md](2026-08-16-safe-paths-workdir-enforcement.md) -- has been
removed from the codebase entirely, by explicit request: the design direction going forward does not
constrain agent/Team workdirs to an allowlist at all.

The immediate trigger was a startup-seeding bug: `seed_safe_paths` unconditionally inserted the default
`~/.agenthub/worktrees` entry into the `safe_paths` DB table on every server boot, which meant the
"empty allowlist = permissive" design intent from the original feature was never actually reachable once
a server had started at least once -- any workdir outside the default worktree root was rejected. Rather
than patch the seeding bug or widen the default, the decision was to drop the feature.

# Scope

`safe_paths` had three independent consumers, found by systematic code search before removal started (to
avoid an incomplete removal that silently half-breaks one of them):

1. **Agent-creation / Team-adoption workdir enforcement** -- `ensure_workdir_within_safe_paths` in
   `src/api/agents.rs`, called from `src/api/agents.rs` and `src/api/teams.rs`. Removed entirely; workdir
   is now accepted unchecked, as it always was for every consumer except this one.
2. **ACP skill-file (`SKILL.md`) loading allowlist** -- `is_skill_path_allowed`, `load_safe_paths` in
   `crates/agenthub-acp/src/lib.rs`, threaded through `SpawnAcpSessionRequest.safe_paths`. Removed; skill
   loading is now fully unrestricted (no path gating).
3. **Team-runtime auto-repair heuristic** -- `collect_repo_candidates` / `infer_repo_from_member_text` in
   `src/team/runtime/repair.rs`, which scanned `safe_paths` roots for candidate git repos to infer a
   missing `worktree_repo` for legacy/misconfigured workers. Deleted entirely, not repointed at another
   root -- `resolve_worker_runtime_repair` now only tries the member's runtime hint and the agent's own
   config; there is no fallback repo-inference step.

Removed across the stack:

- `crates/agenthub-config`: `AppConfig.safe_paths`, `DEFAULT_SAFE_PATH`, `safe_paths()`,
  `AGENTHUB_SAFE_PATHS` env override, and `path_utils::is_path_allowed` (the allowlist-membership check).
  `expand_tilde`/`normalize_path` are general-purpose and were kept.
- `crates/agenthub-db`: the `safe_paths` table DDL, replaced with a `DROP TABLE IF EXISTS safe_paths` so
  any install upgrading from before this change gets it cleaned up; the `migrate_safe_paths_to_absolute`
  migration function.
- `src/state/database.rs`: `seed_safe_paths` (the buggy re-seeding step) and its call site.
- `src/api/admin.rs` + `src/api/authz.rs`: the `/admin/safe_paths` GET/POST/DELETE routes, their three
  handlers, and their `REVIEWED_ROOT_ONLY_CALLS` entries.
- `src/team/runtime/spec.rs`: `TeamRuntimeMemberSpec.description`/`.prompt`, which had no other consumer
  once `infer_repo_from_member_text` was deleted.
- `web/src/`: the admin "Safe Paths" tab and all its wiring (`api.ts` methods, `use_app_admin.ts` state,
  `admin_page.tsx`/`admin_page_sections.tsx` UI, and every test file referencing any of it). The admin
  page's default tab moved from `"safe"` to `"devices"`.
- `userdocs/docs/operations/security-and-path-safety.md`: the `safe_paths`-specific sections and bullets
  (the page also covers roles, sessions, network exposure, and secrets, which are unrelated and were
  kept); `docs/features/access-control-and-roles.md`'s "root-owned safe paths" mention.
- `build/deb/DEBIAN/postinst`: the `safe_paths = ["/var/lib/agenthub/workspaces"]` block that Debian
  packaging hand-wrote into the generated `config.toml` (a gap in the original design -- the code-level
  default never covered this OS-packaging convention, which is now moot).
- `scripts/setup_team_skills.sh` and the remaining `userdocs/` pages that referenced `safe_paths` in
  passing (installation, configuration, troubleshooting, FAQ, and operational-checklist guidance).

Historical dated journal entries about `safe_paths`
([2026-02-15](2026-02-15-safe-path-default-worktrees.md), [2026-03-11 tilde
normalization](2026-03-11-safe-path-tilde-normalization.md), [2026-03-11 team-start
repair](2026-03-11-team-start-safe-path-repair.md),
[2026-08-16](2026-08-16-safe-paths-workdir-enforcement.md)) were left untouched as historical record, not
rewritten to reflect the removal.

# Key Decisions

- Full removal, not just disabling the enforcement check -- confirmed explicitly: config field, DB
  table/migration, admin API, admin UI, all three consumers, and all related tests/docs come out together,
  so there's no half-migrated state where some code still references a table or type that no longer means
  anything.
- ACP skill loading becomes fully unrestricted rather than gated by some other allowlist -- confirmed
  explicitly, since there is no longer a `safe_paths` concept to gate it with.
- The team-runtime repair heuristic is deleted, not repointed at a different root (e.g. `worktree.
  default_root`) -- confirmed explicitly as the recommended option, since a repo-inference fallback that
  scans a filesystem root for candidate git repos is itself a heuristic of debatable value independent of
  `safe_paths`, and keeping it alive under a different root would be scope creep beyond "remove
  `safe_paths`."

# Validation

- `cargo build -p agenthub -p agenthub-config -p agenthub-db -p agenthub-acp --lib --tests` -- clean, zero
  warnings.
- `cargo test -p agenthub --lib` -- full suite passing (the only failures are the pre-existing, unrelated
  `state::tests::initialize_services_*` flakes caused by a `lance-namespace-impls` crate panic, not
  touched by this change); `cargo test -p agenthub-db --lib` -- 50 passed; `cargo test -p agenthub-acp
  --lib` -- 46 passed.
- `cargo clippy -p agenthub -p agenthub-config -p agenthub-db -p agenthub-acp --lib --tests` -- clean.
- `cargo fmt -p agenthub -p agenthub-config -p agenthub-db -p agenthub-acp -- --check` -- clean.
- `web/`: `npx vitest run` -- 162 test files, 1522 tests passing; `npm run lint` -- clean; `npx tsc
  --noEmit` -- clean; `npm run build` -- succeeds.

# Follow-Ups

- None. This is a full, symmetric removal -- there is no reduced or gated version of `safe_paths` left
  behind to revisit later.
