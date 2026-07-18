# Object Upload Owner Scopes

## Summary

This checkpoint extends browser/API object uploads beyond the initial Team-owned routes. Task and
agent upload routes now derive their owner scope from authorized resource paths before publishing
object metadata.

## Background

The OpenDAL storage foundation already centralized object publication in `ObjectUploadService` and
limited owner scopes to `teams/<id>`, `tasks/<id>`, and `agents/<id>`. The first browser slice only
exposed Team-scoped JSON/base64 upload routes, leaving task and agent API authorization as the next
small follow-up.

## Scope

- Add shared API upload request decoding and publication helpers.
- Add Team task object/image upload routes under the parent Team task path.
- Add agent object/image upload routes under the agent resource path.
- Keep object bytes and metadata publication inside `ObjectUploadService`.
- Keep browser-direct multipart and presigned upload-token decisions out of this slice.

## Key Decisions

- Task uploads authorize the parent Team first, then verify the task belongs to that Team before
  publishing `tasks/<task_id>` metadata.
- Agent uploads require an authenticated user and an existing agent before publishing
  `agents/<agent_id>` metadata, matching the existing agent API access model.
- API handlers continue to derive owner scope from route resources; browsers still cannot submit a
  raw owner-scope string.
- OpenAPI path assertions cover the new routes so schema fixtures stay aligned with the route
  surface.

## Validation

```bash
cargo test upload
cargo test openapi_json_contains_team_runs_list_path
```

## Follow-Ups

- Decide whether multipart or presigned upload tokens are the canonical large-object browser path.
- Add an S3-compatible integration fixture before enabling the S3 feature in release builds.
