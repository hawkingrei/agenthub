# Team Forge Role Tag Without Manual Bind

## Background

Team create wizard previously used a manual `bind_target` selector in a global Agent Forge
entry. This made Mission stage UX confusing because users could see/create agents before
entering Leader/Worker stages, and role assignment depended on a separate bind switch.

## Scope

- Remove manual `bind_target` selection from Team create wizard.
- Derive forge role automatically from current wizard stage:
  - `Leader Forge` -> `leader`
  - `Recruit Workers` -> `worker`
- Block Agent Forge action in non-member stages (`Mission Brief`, `Launch Team`).
- Keep the existing create modal and auto-assignment behavior for leader/worker drafts.

## Key Decisions

1. Keep one forge modal implementation, but remove independent bind state.
2. Introduce stage-derived `role_tag` in UI instead of user-editable bind selector.
3. Make role assignment deterministic at create time:
   - leader tag writes directly to `leaderMemberId`;
   - worker tag fills next available worker slot (or appends one).
4. Disable forge entry outside stage 1/2 to align with wizard progression.

## Validation

- `npm --prefix web run build`
- Manual check:
  - Open Team create modal at `Mission Brief`: `New Agent` disabled and stage hint shown.
  - Move to `Leader Forge`: `role_tag` displays `leader`; created agent auto-selected as leader.
  - Move to `Recruit Workers`: `role_tag` displays `worker`; created agent auto-filled to worker list.
