# Team UI Refinement And Live `web/dist` Refresh

## Summary

- tightened the Team workbench and shared-thread shells so more horizontal and
  vertical space stays available for actual content;
- removed directly dead legacy selectors from `web/src/styles.css` where the
  corresponding class names are no longer emitted anywhere in `web/src`;
- switched the Tailwind entry stylesheet to the idiomatic Tailwind v4 import
  form while keeping the existing migration guardrails;
- hardened web asset cache behavior so `web/dist` rebuilds can be picked up on
  manual refresh without stale HTML being cached as an immutable asset.

## Scope

- `src/app.rs`
- `src/web.rs`
- `web/src/tailwind.css`
- `web/src/styles.css`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_task_panel.tsx`

## Key Decisions

1. Keep the current debug/live local deployment path based on `web/dist`:
   - the running debug server serves `web_dir` directly;
   - a fresh `make build-web` should therefore update the next manual refresh
     without rebuilding the Rust binary.
2. Treat cache correctness as the real deployment blocker:
   - hashed asset requests keep immutable caching;
   - navigation and shell assets stay `no-cache`;
   - missing asset requests must not fall back to `index.html` with immutable
     headers.
3. Prefer Tailwind-first visual tuning over new handcrafted CSS:
   - shell density, borders, gradients, and spacing are adjusted in TSX class
     constants;
   - `styles.css` is reduced only where selectors are confirmed dead.
4. Keep the Tailwind migration stable:
   - use the Tailwind v4 `@import "tailwindcss";` entrypoint;
   - keep the border-color compatibility base rule;
   - keep preflight disabled via the existing migration config until the legacy
     stylesheet footprint is smaller.

## Validation

- Rust:
  - `cargo test build_app_router_serves_file_system_fallback --lib`
  - `cargo test web_cache_control_uses_immutable_for_hashed_asset_requests_only --lib`
  - `cargo test web_paths_to_try_only_adds_shell_fallback_for_navigation_requests --lib`
- Web:
  - `make build-web`
  - `cd web && npm run lint`
  - `cd web && npm run test -- src/pages/team_panels.test.tsx`
  - `cd web && npm run test -- src/pages/team_page.smoke.test.tsx`
- Browser:
  - baseline on `https://agenthub.hawkingrei.com/teams/...`
  - refresh the same page after `make build-web` and confirm the slimmer Team
    shells render from the rebuilt bundle.
