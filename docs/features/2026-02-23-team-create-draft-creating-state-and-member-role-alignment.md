# Team Create Draft `creating` State And Member-Role Alignment

## Background

The Team creation flow should not create a persisted Team definition until the user reaches the final `Create Team` action.
Before that point, in-progress data should be tracked as a draft (`creating`) in browser storage.

The UI wording also needed clarification: in Team context, a "worker" is a role assigned to a forged agent, not a separate runtime entity type.

During wizard usage, users may forge multiple candidate agents (for example, leader via `codex` then corrected to `gemini`).
After `Create Team` succeeds, unselected forged candidates from this creation session should be cleaned up to avoid orphan `team_forge` agents.

## Scope

- Frontend only (`/teams` create flow).
- No DB schema migration and no backend API contract changes.
- Keep existing final-create behavior (`POST /api/teams`) at Launch stage.

## Key Decisions

1. Persist draft in browser storage (localStorage) with explicit `status: "creating"`.
2. Restore draft only when the requested entry mode matches (`wizard` vs `manual_spec`) to avoid cross-mode surprise.
3. Keep draft when modal closes, clear draft only after successful Team creation.
4. Clarify wording so "worker" is treated as a role on forged agents.
5. Any error in create-time dependencies (runtime defaults load, draft parse/save failure) should surface via visible UI error instead of silent fallback.
6. After successful Team creation, clean up unselected forged agents created in the same draft session; do not block Team creation when cleanup fails, but surface explicit cleanup errors.

## Implementation

### New draft storage module

- Added `web/src/pages/team/create_draft_storage.ts`.
- Introduced safe load/persist/clear helpers:
  - `persistTeamCreateDraft`
  - `loadTeamCreateDraft`
  - `clearTeamCreateDraft`
- Persisted payload includes:
  - `schema_version`
  - `status: "creating"`
  - `entry_mode`
  - `updated_at`
  - team-create draft fields (name/description/spec/stage/leader/workers/forge member ids).

### Team page integration

- Updated `web/src/pages/team_page.tsx`:
  - Persist draft while Team create modal is open (except submit-in-progress window).
  - On opening modal, restore same-mode draft if present.
  - If draft payload is corrupted/invalid, reset storage and show explicit error.
  - If draft save fails (for example storage write failure), show explicit error.
  - On successful `Create Team`, clear persisted draft.
  - On successful `Create Team`, identify stale forged agents via:
    - `teamForgeAgentIds` (forge candidates tracked in current draft), minus
    - `created.spec.members[].member_id` (members actually selected into Team).
  - Delete stale forged agents via `DELETE /api/agents/:id`.
  - If cleanup fails for any stale agent, keep Team creation success and surface a visible warning message with failed IDs.
  - Updated create-flow wording and validation messages to emphasize:
    - worker is a role of forged agents;
    - leader/member assignment uniqueness.

### Create modal lifecycle error visibility

- Updated `web/src/pages/team/use_team_create_modal_lifecycle_effects.ts`:
  - Loading Team Forge runtime defaults now surfaces error when create modal is open:
    - `Failed to load Team Forge defaults: ...`
  - This replaces previous silent catch behavior during create flow.

### Unit tests

- Added `web/src/pages/team/create_draft_storage.test.ts`:
  - persist/restore wizard draft;
  - mode mismatch returns null;
  - manual-spec restore;
  - clear removes draft.
- Extended `web/src/pages/team/create_helpers.test.ts`:
  - collect unique member IDs from `spec.members`;
  - resolve stale forge candidates (`teamForgeAgentIds - spec.members[].member_id`).

## Validation

- Intended checks:
  - open Team create modal, fill wizard fields, close modal, reopen same mode, confirm draft restored;
  - open other mode and confirm no cross-mode restore;
  - submit successfully and confirm draft cleared;
  - verify wording in Leader/Worker stage copy reflects "worker as role".
- Chrome DevTools MCP verification:
  - Baseline attempt: blocked by existing locked browser profile (`chrome-profile` already running).
  - Post-change attempt: same blocker; no snapshot captured in this change.

## Risks

- Browser draft is local to one browser profile and device.
- If a stale payload is manually injected into localStorage, parser sanitization falls back to defaults where possible.
