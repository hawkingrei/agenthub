# Team Required Role Skills Enforcement

## Background

Team creation previously treated most skills as optional toggles. This allowed
leader/worker role-defining skills to be removed from Team spec, which weakened
orchestration and actor mailbox reliability.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_page.runs.test.ts`
- `src/api/teams.rs`
- `src/api/teams/tests_core.rs`

## Key Decisions

1. Keep role-required skills always present in Team member skill sets:
   - leader: `agenthub-actor-runtime`, `team-leader-orchestrator`
   - worker: `agenthub-actor-runtime`, `team-worker-executor`
2. Enforce at two layers:
   - Frontend Team creation UI: required skill chips are non-removable.
   - Backend Team spec normalization: required role skills are auto-injected even
     when client-provided `spec.members[].skills` omits them.
3. Preserve user custom skills while de-duplicating final skill arrays.

## Validation

Executed:

```bash
cargo fmt
cargo test api::teams::tests:: -- --nocapture
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
```

Observed:

- Team API tests passed, including role-skill enforcement and snapshot coverage.
- Team page helper tests passed.
- Web production build succeeded.
