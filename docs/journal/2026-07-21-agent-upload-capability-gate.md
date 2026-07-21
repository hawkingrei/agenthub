# Agent Upload Capability Gate

## Summary

Agent object and image upload routes now use the `agents:manage` user capability instead of plain
authenticated-user authorization. Viewer sessions are denied before the route can publish
agent-scoped object metadata.

## Background

The access-control rollout is migrating normal operator routes from coarse authentication-only gates
to product capability gates by route cluster. Agent lifecycle and configuration routes already use
`agents:manage`, while agent object/image uploads were left for separate owner-scope
classification because they publish persistent upload metadata under an agent resource.

## Scope

- Converted `POST /{id}/uploads` to `agents:manage`.
- Converted `POST /{id}/images` to `agents:manage`.
- Added router coverage proving a viewer is denied with `agents:manage required`.
- Preserved the existing agent existence check and agent owner-scope object/image metadata contract
  for authorized uploads.

## Key Decisions

- Treat agent uploads as agent management because they create persistent agent-scoped object
  metadata and object keys.
- Keep the resource lookup after the capability check so authorization failure does not reveal
  whether an agent id exists.
- Leave Team and Team-task upload gates under `teams:manage`; this slice only covers agent owner
  scope.

## Validation

```bash
cargo test -p agenthub api::agents::tests::agent_upload_routes_publish_agent_scoped_metadata -- --nocapture
cargo test -p agenthub api::authz::tests::api_code_does_not_bypass_capability_authz_for_human_roles -- --nocapture
cargo fmt -p agenthub -- --check
git diff --check
```

## Follow-Ups

- Continue auditing other API route clusters for authentication-only authorization.
