# Web Event Pull Batch Limit

## Scope

Reduce UI-facing event fetch batch sizes so routine page loads and incremental history pulls do not request hundreds of event rows at once.

## Changes

- reduced the Agents workbench event pull batch in `web/src/use_app_output_cache.ts` from `80` to `20`;
- reduced Team run event and Team member event page pull limits in `web/src/pages/team/state.ts` from `100` / `300` to `20`;
- reduced Team run snapshot `event_limit` / `message_limit` requests in `web/src/pages/team/use_team_actions.ts` from `200` to `20`;
- clamped the public HTTP event endpoints and Team snapshot event/message query limits to `20` so oversized page-originated requests are bounded server-side as well;
- synchronized related frontend defaults and shared constants so mailbox pagination and agent event fetches do not keep assuming `80` / `100` after the server clamp;
- aligned the Team OpenAPI snapshot/event parameter maxima to `20` so generated docs match the runtime contract;
- updated focused Team action tests to assert the new member event pull batch.

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team/use_team_actions.test.tsx`
- `cd web && npm run test -- vite.config.test.ts src/app.route_shell.test.tsx src/app.runtime_effects.test.tsx src/pages/team/use_team_actions.test.tsx`
- `cargo test list_events_route_clamps_limit_to_twenty -- --nocapture`
- `cargo test teams_router_http_contract -- --nocapture`
- `git diff --check`
