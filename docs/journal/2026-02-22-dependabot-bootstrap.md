# Dependabot Bootstrap

## Background

The repository currently has multiple dependency surfaces without an automated update workflow:

- Rust workspace (`Cargo.toml` + `Cargo.lock` at repo root)
- Frontend app dependencies (`web/package.json`)
- User docs dependencies (`userdocs/package.json`)
- GitHub Actions workflow dependencies

To reduce manual dependency drift and keep security/maintenance updates continuous, Dependabot is enabled at the repository level.

## Scope

- Added `.github/dependabot.yml` with weekly update schedules for:
  - `github-actions` at `/`
  - `cargo` at `/`
  - `npm` at `/web`
  - `npm` at `/userdocs`
- Added update grouping (`groups`) per ecosystem to reduce PR noise.
- Reduced duplicated config blocks with YAML anchors (`weekly_schedule`, `commit_message`).
- Added a TODO verification item in `docs/todo.md` to track first post-merge Dependabot evidence.

## Key Decisions

1. Single root cargo updater for the whole workspace.

- The Rust workspace is defined at repository root and uses a single lockfile.
- A single `cargo` update entry at `/` keeps workspace updates consistent.

2. Split npm update entries by directory.

- `web` and `userdocs` are independent npm projects.
- Separate entries avoid cross-project lockfile confusion and keep PR scope clear.

3. Weekly cadence with staggered times.

- Weekly cadence limits PR churn while preserving regular patch/minor intake.
- Staggered windows reduce simultaneous update spikes.

4. Group updates by ecosystem.

- Each configured ecosystem uses `groups` with a catch-all pattern.
- This keeps Dependabot output manageable (fewer, broader update PRs).

5. Use YAML anchors for shared config fields.

- Shared `schedule` and `commit-message` structure is defined once and reused.
- This keeps the config compact and lowers maintenance cost for future tuning.

## Validation

Executed locally:

- Verified `.github/dependabot.yml` exists and targets all active dependency ecosystems used in this repository.
- Verified `docs/todo.md` includes a follow-up verification item linked to this feature note.

Runtime validation after merge:

- Confirm first Dependabot PR generation for each configured ecosystem and record links in verification evidence.
