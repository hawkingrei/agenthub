# Team Creation Restricts Member Selection To Team Forge Agents

## Background

Team creation previously allowed selecting leader/worker members from the entire existing `Agents` pool.
This weakened Team/Agents isolation and made Team composition depend on unrelated agent inventory.

## Scope

- `web/src/pages/team_page.tsx`
- `docs/todo.md`

## Key Decisions

1. Team create modal now uses a Team-Forge-scoped candidate pool.
   - Only agents created in the current Team create session are selectable as leader/worker members.
2. Existing agents are no longer eligible for Team member assignment during create flow.
3. Validation is aligned with UI constraints.
   - Leader must belong to Team Forge candidate set when using auto-generated spec mode.
4. Stage hints and CTA disabled states now reflect Team Forge requirements.

## Implementation Notes

- Added `teamForgeAgentIds` state and derived `teamForgeAgents`.
- Leader/worker option lists, auto-fill, and duplicate resolution now use `teamForgeAgents`.
- Team draft reset clears forge candidate state, ensuring each create session starts clean.
- Updated stage copy from "existing agents" to "Team Forge agents".

## Validation

Executed:

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
npm --prefix web run lint
```

Automated assertions added in `web/src/pages/team_page.runs.test.ts`:

1. `filters forge-selectable members to current team forge session ids`
2. `creates initial team draft with empty forge candidate pool`

Manual checks:

1. Open Team create modal and verify leader/worker selectors are empty before Forge creation.
2. Create agent via Team Forge and verify it becomes selectable immediately.
3. Verify agents created outside current Team create session do not appear in leader/worker selectors.
4. Complete Team creation and reopen modal; verify candidate pool resets for a new session.
