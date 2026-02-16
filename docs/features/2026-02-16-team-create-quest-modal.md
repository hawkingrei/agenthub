# Team Create Quest Modal

## Background

The `/teams` page previously exposed a long always-visible create form in the left sidebar.
While functionally complete, it had two UX problems:

- Team creation fields competed with run-management content in the same view.
- Leader/worker configuration lacked guided progression and felt heavy for first-time setup.

## Scope

- Replace sidebar inline create form with a compact `Team Forge` launch card.
- Add a `Create Team` modal flow with staged progression:
  - `Mission Brief`
  - `Leader Forge`
  - `Recruit Workers`
  - `Launch Team`
- Reuse Agent preset options as selectable model choices for leader/worker (`Codex ACP`, `Gemini CLI`, `Kimi CLI`) instead of free-text model input.
- Use existing Agent records as team members (leader/worker select boxes bind to Agent IDs), and show selected member workdir so work path ownership is explicit.
- Generate default team workflow steps in create flow output:
  - `leader_plan`
  - `worker_*`
  - `leader_synthesize`
- Seed leader/worker default prompts and default skills for mailbox-based actor collaboration.
- Clarify `Create / Load Run` UX copy in Team workbench:
  - `Create Run` = new execution instance for selected team.
  - `Load Run` = open existing run by `run_id`.
- Keep payload contract unchanged (`leader_member_id`, member role/model/prompt/skills/steps).
- Preserve advanced manual spec editing (`Edit spec JSON manually`) in final stage.
- Support leader-only team creation (zero workers).

## Key Decisions

- No API contract changes. Backend now injects default `leader_member_id`, prompts/skills, and workflow steps when missing.
- Stage navigation is explicit and non-blocking for review jumps, but `Next Stage` enforces minimal required inputs (`team name`, `leader member_id`).
- Team member `member_id` in generated spec maps to existing Agent `id`; workdir/worktree still comes from Agent config and is not duplicated in Team spec.
- Team create defaults include collaboration guidance so leader/worker can immediately use actor mailbox primitives without manual prompt bootstrapping.
- Visual direction uses a “quest/forge” metaphor via stage chips and modal framing while staying within existing design system primitives.
- Success path closes modal and resets stage index; existing team draft defaults remain reusable on next open.

## Validation

Suggested checks:

```bash
npm --prefix web run lint
npm --prefix web run build
```

Manual checks:

1. Open `/teams`, click `Create Team`, and verify staged modal progression and stage switching.
2. Create a leader-only team (remove all workers) and verify successful creation.
3. Create a team with multiple workers and verify member selection uses existing agents and displays workdir.
4. Enable `Edit spec JSON manually` in final stage and verify custom JSON create path still works.
5. Verify modal behavior on narrow viewport (stage chips wrap into 2-column layout).
6. Verify generated spec contains default `steps` workflow (`leader_plan` → `worker_*` → `leader_synthesize`).
