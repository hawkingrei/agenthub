# Team Prompt And Mailbox Follow-Ups

## Summary

- backfilled blank worker prompts after `GET /api/teams/prompt_defaults` resolves while the
  Team create modal is already open
- kept restored create drafts aligned with API-owned worker prompt defaults without overwriting
  non-empty user edits
- disabled read-only mailbox refresh while any mailbox action is already busy to avoid overlapping
  refresh/accept/send UI state races
- moved the ACP timeout coordination helper import behind `#[cfg(test)]` so workspace clippy stays
  clean

## Scope

- `web/src/pages/team/member_helpers.ts`
- `web/src/pages/team/member_helpers.test.ts`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/pages/team_panels.test.tsx`
- `src/team/permission_review.rs`

## Validation

- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cd web && npm run test -- src/pages/team/member_helpers.test.ts src/pages/team_panels.test.tsx`
- `cd web && npm run build`
