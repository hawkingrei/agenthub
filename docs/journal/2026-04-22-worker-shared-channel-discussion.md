# Worker Shared-Channel Discussion

## Context

Team runtime guidance still biased workers toward reporting through leader mailbox first, even when
an issue clearly needed shared discussion with multiple teammates. That made important channel
discussion feel leader-gated in practice.

## Decision

- Keep leader as the owner of planning decisions and final integrated human-facing synthesis.
- Allow workers to initiate and participate in shared-channel discussion directly for important
  matters that need team-wide visibility or discussion.
- Require workers to `@member_id` the relevant other agents when the shared-channel discussion
  needs specific reviewers, owners, or dependency peers.
- Keep direct leader mailbox as the default route only when leader is the single next owner.

## Updated Sources

- `docs/features/teams-collaboration-playbook.md`
- `skills/team/AGENTS.md`
- `skills/team/team-worker-executor.SKILL.md`
- `crates/agenthub-managed-skills/src/lib.rs`

## Validation

- `cargo test -p agenthub-managed-skills worker_managed_skills_allow_shared_channel_discussion_for_important_matters -- --nocapture`
