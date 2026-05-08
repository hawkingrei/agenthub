# Team Route Container Cleanup

## Summary

The Team route now has a dedicated `TeamRouteContainer` under `web/src/routes/`.
`web/src/app.tsx` keeps the top-level route-kind switch, while Team-specific route parsing,
lazy loading, and page prop forwarding live outside the root app shell.

## Background

The P0 frontend cleanup backlog calls out continued reduction of `web/src/app.tsx` and
Team workspace composition state. The previous route-container work moved Admin and Agents
surfaces behind route containers, but the Team branch still assembled its lazy page and route
props inline inside `App`.

## Scope

- Extract Team route composition into `web/src/routes/team_route_container.tsx`.
- Preserve the existing lazy `TeamPage` boundary and `RouteFallback` loading text.
- Keep `App` responsible for selecting `routeKind`, not for deriving Team page props.
- Add focused coverage for route parsing and prop forwarding.

## Key Decisions

- The new container accepts primitive route state (`routePathname`, `routeSearch`) instead of
  depending on global `location`, keeping it deterministic and testable.
- `auth.token` is derived inside the container because `TeamPage` still accepts both `auth` and
  `token`; this keeps the compatibility boundary local until `TeamPage` props are simplified.
- This checkpoint does not close the broader P0 item. Further work is still needed to extract
  the next stable Team view-model boundary and continue shrinking route-level state.

## Validation

```bash
cd web && npm exec vitest -- run src/routes/team_route_container.test.tsx
cd web && npm exec tsc -- --noEmit
```

## Follow-Ups

- Continue extracting stable Team view-model or route prop boundaries from `TeamPage`.
- Run the standard web lint/build gates before the PR is opened.
