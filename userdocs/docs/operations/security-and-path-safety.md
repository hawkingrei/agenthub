---
sidebar_position: 2
---

# Security and Path Safety

AgentHub launches powerful subprocesses in user-selected workspaces. Its
authentication and path checks are important control-plane guardrails, but they
are not an operating-system sandbox.

## Security Boundary

- The web/API layer authenticates a user and checks a capability for each
  protected operation.
- AgentHub does not restrict which workdir an agent or Team member can use;
  operators choose workdirs directly, and the agent process runs with the same
  operating-system identity and ambient privileges as the AgentHub service
  unless you add container or host-level isolation.
- Provider credentials and network access available to that service identity
  may also be available to the runtime.

Setting a process working directory does not prevent it from reading other
files that the OS account can access. For stronger isolation, use a dedicated
service account, containers/VMs, filesystem permissions, and network policy.

## Roles and Capabilities

AgentHub uses explicit capabilities rather than one global user check:

| Role | Intended access |
|------|-----------------|
| `root` | All instance, user, auth, runtime, Team, node, diagnostics, and push operations. |
| `admin` | Agent, Team, node, linker, runtime, diagnostics, and push operations; no root instance/user/auth configuration. |
| `operator` | Agent and Team management plus runtime operation/inspection and push subscription. |
| `viewer` | Runtime inspection and push subscription. |
| `device` | Push subscription only. |

Initialize the first root operator from a trusted network. Teamspace invitation
links create operator accounts; they do not create another root.

## Sessions and Passkeys

- Passwords are hashed with Argon2.
- Browser/API bearer sessions expire after 12 hours. Logging out removes the
  browser's token; use expiration and credential rotation as the current
  server-side invalidation boundary.
- Passkeys are disabled by default and require a correct relying-party origin.
- Outside localhost, use HTTPS before enabling WebAuthn or browser push.

Bearer tokens grant the holder the session's capabilities. Never place them in
URLs, committed files, screenshots, support tickets, or durable shell history.
The SSE endpoint is the browser-specific exception that uses a query token; keep
proxy access logs for `/sse/*` appropriately restricted and redacted.

## Network Exposure

The safe default is:

```toml
[server]
listen = "127.0.0.1:8080"
```

For shared access, place AgentHub behind an authenticated HTTPS reverse proxy or
private network. Preserve SSE streaming without proxy buffering. Do not expose a
fresh first-run instance publicly.

Remote Agent Nodes use a separate internal gRPC listener. Keep it on a private
network, use `tls` or `mtls`, protect the bootstrap token and JWT shared secret,
and synchronize node clocks. `security.mode = "disabled"` is for isolated local
testing only.

## Secrets and Stored Data

Protect at least:

- `config.toml` when it contains internal gRPC secrets or environment-variable
  names that reveal credential wiring;
- `vapid.json`, which contains the Web Push private key;
- the main and per-agent SQLite databases;
- optional message-body, message-archive, and object-store data;
- provider configuration and credentials visible to the service account.

Use restrictive directory permissions and encrypted disks or volumes. S3
credentials should be supplied through the configured environment-variable
names, not as literal values in `config.toml`.

## S3 and Download Ingestion

Official release builds include the OpenDAL S3 backend, while source/default
builds keep it feature-gated and runtime storage defaults to `fs`. Including the
backend does not certify every S3-compatible provider.

When server-side URL ingestion is enabled:

- keep private-network downloads disabled unless explicitly required;
- allowlist expected hosts and retain size, redirect, timeout, retry, and
  per-host concurrency limits;
- verify expected size and SHA-256 when the source provides them;
- authorize object access through AgentHub metadata rather than treating a
  public object URL as permission.

## Operational Hardening Checklist

- Run AgentHub as a dedicated non-root OS account.
- Since AgentHub does not restrict workdirs itself, scope filesystem
  permissions for the service account to the workspaces it should touch.
- Keep HTTP and gRPC listeners private; terminate external access with TLS.
- Back up the complete data set and test restore before upgrades.
- Keep the main binary and `agenthub-acp` on the same release.
- Patch the host, browser, ACP providers, and AgentHub dependencies promptly.
- Review authentication audits, node registration, permission decisions, and
  unexpected process/network activity.
- Revoke sessions, rotate exposed secrets, and preserve logs before repairing
  a suspected compromise.

## Reporting a Vulnerability

Use the repository's private GitHub security-reporting flow. Do not include live
tokens, credentials, private prompts, or customer data in the report, and do not
open a public issue for an unpatched vulnerability.
