---
sidebar_position: 5
---

# FAQ

## Why does a task continue after I close the browser?

The backend owns the agent process. The browser replays persisted history and
reconnects to live SSE when it returns.

## Which binaries do I need?

Install both `agenthub` and `agenthubd` from the same release. Debian and npm
platform packages contain both. GitHub publishes separate archives.

## Which platforms have official binaries?

macOS Apple Silicon, Linux x86_64, and Linux ARM64. Windows and macOS Intel are
not current release targets. The minimum Linux glibc baseline is still being
finalized, so validate the exact artifact on the deployment host.

## Is Homebrew the recommended install?

Not for a new complete installation while the tap trails the current release
and adapter naming. Use the verified GitHub archives or Debian package.

## Does AgentHub restrict which workdir I can use?

No. AgentHub does not validate or restrict workdirs; you can point an agent or
Team workdir at any path the service account can access, and the subprocess
inherits that account's full filesystem and network privileges. Use a
dedicated account and host, container, or VM isolation for stronger
boundaries.

## Should I use one agent per repository?

Use one agent per stable intent. Parallel changes are easier to review when
each agent has its own worktree, even if they share a repository.

## Why is the Debug tab missing?

Production defaults hide developer-only output and session metadata. A root
operator can enable developer mode from **Admin** for diagnosis.

## Why does the badge say `SSE idle`?

No running agent currently needs a live stream. This is normal for created,
stopped, exited, or failed agents.

## How do I debug missing live output?

Check the agent state, connection badge, `/health`, browser Network entry for
`/sse/agents`, and server logs. Refreshing is safe and triggers persisted event
replay. Do not delete an event database.

## Does the OpenAPI document cover every HTTP route?

No. It is an incremental public automation contract focused on Team operations
and scoped uploads. Generate clients from the exact deployed
`/api/openapi.json` and do not infer stability for omitted routes.

## Does the official binary support S3?

Official release builds include the OpenDAL S3 backend, but storage defaults to
local `fs`. Source/default builds keep S3 feature-gated. You must configure
`backend = "s3"` and validate the exact release against your provider; inclusion
does not certify every S3-compatible service.

## What must I back up?

Prefer a consistent snapshot of the entire AgentHub data directory while the
service is stopped, plus any externally configured archive, body, object, and
certificate paths. Include per-agent event databases, not only `agenthub.db`.
