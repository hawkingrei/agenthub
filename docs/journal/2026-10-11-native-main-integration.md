# Native Main Integration

## Summary

Assemble the native capability stack in a branch based directly on current main. This provides
one integration PR while preserving the smaller implementation PRs and their review history.
Publication of the producer and configured-provider qualification remain rollout gates.

## Background

PR #1201 merged into the branch for #1200 at `d4dc9060beb27ec15e4513af58f10d2f74adc331`.
That merge did not place the native capability stack into main. The remaining dependency PRs
still form a chain from #1189 to #1200.

## Scope

| Source PRs | Integrated capability |
| --- | --- |
| #1189, #1190 | Reopened milestones and session-scoped controlled App/Mem sources |
| #1191 | Canonical semantic outcomes and clarification waits |
| #1193, #1194 | Durable conversation binding, current-owner approval and recovery admission |
| #1195, #1196 | Explicit browser recovery review and committed-opening reconciliation |
| #1197, #1198 | Fenced loop resume and independent standalone continuity |
| #1199 | Retained activation transcript navigation |
| #1200, #1201 | Six configured Team acceptance entrypoints and the assembled producer pin |

The ACP rollout remains the mainline baseline. Remote ownership, non-Linux guardians and the
other separately scoped extensions in TODO are not added by this integration.

## Key Decisions

- Create `codex/native-main-integration` from main `b4bf69f1086b4fc27406706906902a4814b5970c`.
- Merge the complete consumer head normally, preserving source branches and commit history.
- Keep main's Vite 8.3.2 dependency update. Before this documentation checkpoint, the merged tree
  differs from the qualified consumer head only in `web/package.json` and `web/package-lock.json`.
- Keep the integration PR in draft until the producer publication and runtime/provider gates are
  satisfied. Do not infer those gates from a dependency PR's merged state or passing consumer CI.

## Validation

The merge preview and actual merge have no conflicts. The source consumer at `c7fdc83c` passed
all applicable CI before #1201 merged. The prior exact-binary and six-entrypoint local results are
recorded in [candidate qualification](2026-10-07-native-runtime-qualification.md).

With Vite 8.3.2 installed from the retained lockfile, web lint, TypeScript, production build and
22 focused tests across four recovery/history files pass. Five Chromium E2E cases pass: desktop
and narrow recovery review, viewer restrictions, retained loop history and request identity after
reload. Rust formatting, whitespace and 121 local documentation links also pass.

```sh
cd web
npm ci --prefer-offline
npm run lint
npm exec tsc -- --noEmit
npm run build
npm exec vitest -- run src/native_recovery.test.ts src/components/native_recovery_panel.test.tsx src/pages/team/loop_activation_history.test.tsx src/pages/team/loop_activation_transcript.test.tsx
NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost CI=1 PLAYWRIGHT_PORT=4187 npm exec playwright -- test 'native_recovery\.e2e\.ts$' 'team_loop\.e2e\.ts$' --project=chromium --workers=1
```

The first browser attempt timed out because the inherited proxy intercepted the localhost
readiness check. A direct request confirmed Vite was ready; a command-scoped proxy bypass fixed
readiness. The required Playwright Chromium revision was then installed before the final passing
run. No repository configuration or application code changed for either environment prerequisite.
Logs use `/tmp/agenthub-native-main-*.log`. The new integration head still requires its own CI;
source-branch results are not reported as integration-head results.

## Follow-Ups

- Complete applicable CI on the new main-targeted integration head before integration.
- Publish producer commit `df7ed52b684aa175cb1f280ea306abaafedf73e2` and complete upstream CI after
  the existing destination authorization is answered.
- Complete installed-runtime and external configured-provider Team qualification. The existing
  provider destination/payload authorization is still unanswered; local deterministic checks do
  not establish that result.

Contract: [direct runtime integration](../features/rara-direct-integration.md).
Active work: [TODO](../todo.md).
