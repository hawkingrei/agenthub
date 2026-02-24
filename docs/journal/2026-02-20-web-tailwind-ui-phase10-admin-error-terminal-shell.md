# Web Tailwind UI Phase-10: Admin, Error, and Terminal Shell

## Background

After ACP and Team shell migration, admin and shared runtime surfaces still used
legacy class-only styling. This phase migrates those shells to Tailwind utility
layers while preserving behavior and existing semantic classes.

## Scope

- `web/src/pages/admin_page.tsx`
- `web/src/error_banner.tsx`
- `web/src/components/terminal_output.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep behavior and markup semantics stable:
   - admin tab switching and actions unchanged
   - error banner close button contract unchanged (`error-close`)
   - terminal stream rendering still uses `ansi()` and `dangerouslySetInnerHTML`
2. Preserve legacy semantic classes used by existing selectors/tests:
   - admin: `app`, `session`, `admin`, `toolbar`, `tab`, `active`, `card`,
     `form-row`, `checkbox`, `join-card`, `kv-*`, `mono`
   - error: `error`, `error-text`, `error-close`
   - terminal: `terminal`, `line`, stream classes (`stdout` / `stderr` / `system`)
3. Add Tailwind-only visual layering:
   - admin shell cards, tab bar/button states, form controls, list surfaces
   - error banner alert tone and dismiss affordance
   - terminal dark shell and stream-specific text tones

## Validation Evidence (local)

- Targeted regression tests:
  - `npm --prefix web run test -- src/error_banner.test.tsx src/output_body.test.tsx src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_conversation_render.test.tsx src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks:
  - admin tab visual parity (`Safe Paths`, `Devices`, `Login Audits`, `Join Device`, `VAPID Keys`)
  - safe-path bulk select + delete affordance readability
  - terminal long-output readability and scroll comfort in runtime sessions
  - error banner visibility/close behavior under network and API error states

## Verification Result

- Local automated checks passed (targeted tests + lint + build).
- Manual visual verification for this phase was marked completed in review
  confirmation (`"2 completed"`).
