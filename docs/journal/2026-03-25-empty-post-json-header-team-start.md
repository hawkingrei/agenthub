# Summary

Stop sending `Content-Type: application/json` on empty POST requests from the
web client, and tolerate empty JSON-labelled start requests on the backend.

# Why

The shared `apiFetch` helper always attached a JSON content-type header, even
for POST requests that intentionally had no request body. Endpoints such as
team start, agent start, stop, cancel, and force-new-session therefore arrived
at the backend as "JSON requests with an empty body", which caused extractor
paths that accept optional JSON payloads to fail with:

`Failed to parse the request body as JSON: EOF while parsing a value at line 1 column 0`

# What Changed

- Added header construction logic that only injects `Content-Type:
  application/json` when a request body is actually present.
- Kept authorization header injection unchanged.
- Preserved explicit caller-provided content-type headers.
- Hardened `/api/agents/:id/start` so an empty body with a JSON content-type is
  treated the same as no payload instead of failing JSON extraction.
- Added focused frontend tests covering:
  - empty POST requests omit JSON content-type
  - JSON body requests still send JSON content-type
  - explicit caller headers are preserved
- Added backend tests covering:
  - empty request bodies parse as no start payload
  - `/api/agents/:id/start` accepts an empty JSON-labelled body

# Validation

- `npm test -- src/api.test.ts`
- `cargo test start_route_accepts_empty_body_with_json_content_type -- --nocapture`
- `git diff --check`
