# Push Subscribe Capability Gate

## Summary

The `/api/push/subscribe` route now uses the `push:subscribe` capability instead of
plain authenticated-user authorization.

## Background

Push subscription is intentionally available to every known v1 user role, including
`viewer` and `device`, but it should still deny unknown or malformed roles. The route
previously accepted any valid session before saving the subscription, which bypassed the
capability matrix for this device-facing surface.

## Scope

- Switched subscription authorization to `require_capability(..., PushSubscribe)`.
- Kept `/api/push/vapid_info` and `/api/push/vapid_rotate` root-only because they expose
  or mutate instance VAPID key configuration.
- Added route coverage for missing auth, unknown-role denial, and allowed viewer/device
  subscriptions.

## Key Decisions

- Treat push subscription as the first device-facing user capability route migration.
- Use the canonical user capability helper rather than adding a push-specific role check.
- Keep the test fixture local to the route test while matching the push service table schema.

## Validation

```bash
cargo fmt -p agenthub -- --check
cargo test -p agenthub api::push::tests::subscribe_requires_push_subscribe_capability -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
```

## Follow-Ups

- Continue migrating normal operator routes by route cluster.
- Keep root-only VAPID key inspection and rotation unless the access-control contract is
  explicitly expanded for push administration.
