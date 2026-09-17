# App Management API

## Problem

App connection provisioning can select daemon secrets and destinations. Human management must keep
that authority separate from ordinary manifest publication and member configuration.

## Scope

Authenticated HTTP management of the [registry](app-registry-storage.md): connection provisioning,
owner-scoped inspection/publication/revocation, Team approvals, and explicit member bindings.

## Non-Goals

- Provider invocation, launch resolution, public discovery, or signed event intake.
- Credential values in requests, responses, Cards, or logs. Requests reference daemon environment
  names; only the runtime resolver reads their values.
- App ownership transfer, editing a registered endpoint/authority, or restoring global revocation.

## Architecture

The `/api/apps` router owns registration management. Routes under `/api/teams` apply existing
Teamspace access rules before using the same durable registry. Human capability checks precede
ownership checks; no provider credential can use these endpoints.

## Contracts

| Method and path | Required authority | Result |
| --- | --- | --- |
| `POST /api/apps` | `instance:configure` | Provision connection, owner, and version 1; `201` |
| `GET /api/apps` | `runtime:inspect` | Current user's own registrations only |
| `GET /api/apps/{app_id}` | `runtime:inspect` and App ownership | Safe registration |
| `GET /api/apps/{app_id}/versions/{version}` | Same | Immutable manifest version |
| `POST /api/apps/{app_id}/versions` | `runtime:operate` and App ownership | New immutable version; `201` |
| `POST /api/apps/{app_id}/revoke` | Same | Permanently revoke the App |
| `GET /api/teams/{team_id}/apps/{app_id}` | `runtime:inspect` and Team access | Team grant, including revoked history |
| `PUT /api/teams/{team_id}/apps/{app_id}` | `runtime:operate`, Team ownership, and App ownership | Approve a Team scope subset |
| `POST /api/teams/{team_id}/apps/{app_id}/revoke` | `runtime:operate` and Team ownership | Revoke Team authority |
| `GET /api/teams/{team_id}/members/{actor_id}/apps` | `runtime:inspect`, Team access, current member | Explicit member bindings |
| `PUT /api/teams/{team_id}/members/{actor_id}/apps/{app_id}` | `runtime:operate` and Team ownership | Bind a specific version and scope subset |
| `POST /api/teams/{team_id}/members/{actor_id}/apps/{app_id}/revoke` | Same | Revoke that member's authority |

Registration accepts `name`, optional `owner_user_id` (defaults to the caller), `connection`, and
`manifest`. Connection fields are `endpoint`, optional `credential_env`, `authority`, and `namespace`.
Only instance configuration may submit them. All responses omit connection fields. App ownership
alone does not confer instance configuration, and instance configuration does not bypass ownership
on ordinary management reads or writes.

Publication accepts `expected_revision` and `manifest`. Approval accepts `expected_revision` and
`scopes`; member binding also requires `version`. Revocation accepts only `expected_revision`.
Unknown request fields are rejected. New approval/binding uses revision `0`; all updates and
revocations use the observed positive revision. Version numbers are explicit, never `latest`.

Approving a Team currently requires the same caller to own the App and be a Team owner. This is a
conservative initial contract; cross-owner consent workflows are not implicitly available. Subsequent
member binding or revocation requires Team ownership only. Revocation remains available after global
App revocation. Team ownership includes an active Teamspace `owner` role, but a legacy Team without
an explicit `owner_user_id` cannot grant new App authority through compatibility access.

Lists accept `after` (exclusive registration/App id) and `limit` (`1..100`, default `50`), and return
an ordered JSON array. Use the last returned id as the next cursor. Revoked records remain inspectable.
Approval alone creates no member binding and starts no process.

Requests are bounded to 512 KiB for registry routes and 16 KiB for Team binding routes. Manifest and
scope constraints additionally follow the storage contract. Malformed or oversized JSON yields a
bounded `400` without echoing request fragments. Missing capability yields `401`; a foreign App or
inaccessible Team yields `404`; insufficient Team ownership or excess scope yields `403`. Stale
revisions, revoked authority, and duplicate authority registration yield `409`. Database internals,
private endpoints, and credential references are not exposed in error messages.

## Validation Matrix

| Boundary | Focused check |
| --- | --- |
| Provisioning | Root capability succeeds; admin/operator/viewer/device and anonymous callers fail |
| Ownership | Assigned owner sees safe projections; unrelated users and provisioning root cannot read them |
| Versioning | Publication requires owner and observed revision; old manifests remain unchanged |
| Capability changes | A viewer may inspect its records but cannot publish |
| Team approval | App and Team ownership intersect; legacy unowned Team cannot approve |
| Explicit binding | Approval alone grants nothing; scopes, member identity, and revisions are enforced |
| Revocation | Ordinary Team member cannot revoke; owner can revoke after global App revocation |
| Bounds and privacy | Request/page/revision limits, malformed schemas, and sanitized extractor/database failures |

## Operational Notes

Provisioning does not contact the endpoint or prove it healthy. Manifest publication and binding do
not launch agents. Runtime preflight and call-time revocation enforcement are separate integration
gates in the [App tool contract](app-tool-registration.md).

## Open Risks

[Runtime integration](app-tool-runtime.md) consumes these durable contracts for shared proxy calls,
launch, safe Cards, and pinned history. Events are slice 15. Frontend authoring and cross-owner
approval flows remain separate scope.

## Source Journals

- [App management API checkpoint](../journal/2026-09-18-app-management-api.md)
