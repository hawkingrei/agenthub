# App Event Configuration

Status: configuration foundation implemented. Signed HTTP ingress, durable event receipts, and
standing event conditions remain part of the active [event intake work](../todo.md#agent-loop-product-transition).

## Problem

A tool binding permits scoped calls but does not authorize an external application to wake members.
Incoming event identity also requires credentials separate from outbound MCP authentication.

## Scope

- Optional, versioned event declarations in the existing App manifest.
- Independent inbound signing-key references and explicit per-member event routes.
- Safe source-attribution types for later durable intake.

## Non-Goals

This foundation does not accept events, advance cursors, schedule activations, or verify signatures
in a production HTTP handler. No raw event payload, remote command, or task assignment is supported.

## Architecture

The App registry owns additive key-version and event-route tables alongside existing immutable
manifests, Team grants, and member bindings. Management uses the existing human capability and
Teamspace role gates. The eventual signed intake consumes these records inside its write transaction.

## Contracts

### Declarations and notifications

`events` is an optional manifest array, defaulting to empty for existing versions. Each declaration
contains a unique bounded `name` and a nonempty `required_scopes` subset of the manifest scopes.
At most 64 classes are declared; names use the same 128-byte identifier grammar as tools. Apps still
require at least one tool. Publishing a new manifest never edits an old version.

The notification type accepts only `schema_version: 1`, `event_id`, positive `cursor`, `team_id`,
`actor_id`, and `event_class`. Event IDs and classes use the bounded App identifier grammar; Team
and actor IDs use the existing loop identifier grammar. Unknown fields are rejected. The optional
source `app_event` retains event ID, class, cursor, and manifest version and requires an enclosing
`app_id`. Existing source records deserialize unchanged.

### Signing keys

| Route | Authority | Behavior |
| --- | --- | --- |
| `GET /api/apps/{app}/event-key` | App owner with runtime inspection | Latest safe key version/state |
| `PUT /api/apps/{app}/event-key` | Instance configuration | Install an environment reference with `expected_version` |
| `POST /api/apps/{app}/event-key/revoke` | App owner with runtime operation | Revoke using `expected_version` |

Installation uses `credential_env`; values are never accepted or returned by these routes.
References use the existing credential-name validator, including reserved environment exclusions.
The first expected version is zero. Every installation or revocation of an active key advances the version and
revokes the previous key. Concurrent changes use optimistic revision fencing. At most 1024 key
installations are retained per App; further installation requires operator maintenance. Revocation
remains available at the limit, and revoking an already revoked current version is idempotent.

Daemon-only lookup returns an active key reference only while the App is active. Verification and
transactional intake must recheck the same key version, preventing rotation races. Management DTOs
expose only App ID, version, revocation timestamp, and creation timestamp. All historical signing
references remain in the credential-isolation set for loop children, including rotated/revoked keys.

### Event routes

| Route | Authority | Behavior |
| --- | --- | --- |
| `GET /api/teams/{team}/members/{actor}/apps/{app}/events` | Team inspection | Safe route configuration |
| `PUT /api/teams/{team}/members/{actor}/apps/{app}/events` | Explicit Team owner | Approve `classes` with `expected_revision` |
| `POST /api/teams/{team}/members/{actor}/apps/{app}/events/revoke` | Explicit Team owner | Revoke using `expected_revision` |

An existing tool binding grants no event classes by default. A route requires current membership,
an active App, active Team/member grants, and a nonempty class subset allowed by both approved scope
sets. It records the selected manifest version and both authorization epochs. Stale configuration
writes fail with a conflict.

Event authority requires every recorded version/epoch to match the current binding. Publishing an
unselected version preserves authority; selecting another binding version requires explicit route
reapproval. Permission changes and revoke/restore cycles cannot reactivate stale routes. Revoking
event routing alone preserves the independent tool configuration.

## Validation Matrix

| Boundary | Focused proof |
| --- | --- |
| Compatibility | Legacy manifests/source JSON and an existing registry migrate without granting event authority |
| Declarations | Duplicate classes, invalid names, undeclared scopes, unknown fields, and oversized class sets fail |
| Keys | Root-only installation, safe owner inspection, rotation races, revocation, reopen, secret isolation |
| Routing | Explicit owner approval, class/scope intersection, foreign targets, and stale version/epoch rejection |
| Separation | Tool-only binding has no event route; configuration creates no triggers |

## Operational Notes

Provision distinct inbound signing-key material in daemon environment storage. These routes
configure references offline; production signature verification and its wire format are not yet
available in this foundation. Existing [tool invocation](app-tool-runtime.md) remains independent.

## Open Risks

The event delivery slice must ship timestamp/signature validation, atomic event ID/cursor/trigger
receipts, replay protection, bounded denial audit, admission budgets, and standing-condition authority
together before advertising event ingress.

## Source Journals

- [App event configuration](../journal/2026-09-18-app-event-configuration.md)
