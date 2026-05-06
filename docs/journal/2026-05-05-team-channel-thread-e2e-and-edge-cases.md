# Team Channel/Thread E2E Coverage And Edge Case Hardening

## Summary

- Added Playwright E2E coverage for Team channel create, switch, delete, and thread reply flows
- Added channel API route mocks to the E2E fixture
- Fixed two edge-case bugs in the channel metadata and sidebar

## Scope

- `web/tests/e2e/team_page_fixture.ts`
- `web/tests/e2e/team_page_channels.e2e.ts` (new)
- `web/src/pages/team/channel_metadata.ts`
- `web/src/pages/team_sidebar.tsx`

## E2E Tests

| Test | Description |
|------|-------------|
| Shows `#all` by default | Verifies sidebar renders the default channel and clicking it keeps the channels lens active |
| Create, switch, delete custom channel | Full CRUD: opens create form, fills channel ID/description, submits, switches to new channel, switches back to `#all`, deletes via hover action |
| Channel URL routing + thread | Direct navigation to `?lens=channels&channel=all&thread=5` |
| Delete confirmation dialog | Dismissing the confirm dialog preserves the channel |

## Bug Fixes

### 1. Duplicate `# all` when API returns `channel_id === "all"`

`buildTeamChannelItems` in `channel_metadata.ts` did not deduplicate against
`DEFAULT_TEAM_CHANNEL_ID`. If the API returned a channel record with
`channel_id === "all"`, the sidebar would show two `# all` entries.

Fixed by filtering out `DEFAULT_TEAM_CHANNEL_ID` from API channels before
prepending the default channel list.

### 2. Channel create form persists across team switches

The create-channel form (`showCreateChannelForm`, `newChannelId`,
`newChannelDescription`) is local state in `TeamSidebar`. When switching between
teams, the form stayed open with stale input from the previous team.

Fixed by adding a `useEffect` that resets the form state when `selectedTeamId`
changes.

## Channel API Fixture Mocks

Added to `team_page_fixture.ts`:
- `GET /api/teams/:id/channels` — returns in-memory channel list
- `POST /api/teams/:id/channels` — creates channel, rejects duplicates (409)
- `DELETE /api/teams/:id/channels/:channelId` — deletes channel
- `POST /api/teams/:id/channels/:channelId/threads/:messageId/replies` — returns synthetic reply

## Validation

- `cd web && pnpm run lint` — clean
- `cd web && pnpm exec tsc --noEmit` — clean
- `cd web && pnpm run build` — 620ms
- `cd web && pnpm exec vitest run src/pages/team/team_thread_pane.test.tsx src/pages/team/page_helpers.test.ts src/pages/team_panels.test.tsx` — 3 files, 149/149 passed
