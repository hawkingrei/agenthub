---
sidebar_position: 2
---

# Configuration Basics

This page covers the operator-facing runtime options in AgentHub's current
configuration schema. Experimental integration bindings are intentionally
documented with their owning feature contract until they become a supported
user workflow.

## Configuration File

AgentHub reads configuration from `~/.agenthub/config.toml` by default.

If you want a guided first-run setup, run:

```bash
agenthub init
```

`agenthub init` currently covers instance role selection (`main` vs `node`) and
the current `internal_grpc` bootstrap contract. It does not configure
provider-specific API base URLs or API keys yet.

## Minimal Configuration

Start with this baseline for single-node usage:

```toml
[server]
listen = "127.0.0.1:8080"

[worktree]
default_root = "/home/you/.agenthub/worktrees"

[history]
event_retention_days = 5
vacuum_on_cleanup = false
```

## Complete Configuration Reference

### Top-Level Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `web_dir` | string | `web/dist` (dev only) | Path to web assets |
| `log_path` | string | none | Log file path (default: stdout) |

### `[server]` Section

HTTP server configuration.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `listen` | string | `"127.0.0.1:8080"` | Bind address and port |
| `role` | string | `"main"` | Runtime role: `main` or `node` |
| `node_id` | string | `"main"` in `main` mode | Required non-`main` node identity when `role = "node"` |

**Distributed Node Example**:

```toml
[server]
role = "node"
node_id = "gpu-01"
```

When `role = "node"`, AgentHub starts only the node runtime surface. Public
HTTP/UI routes are disabled, and `internal_grpc.enabled` must be `true`.

### `[web]` Section

WebAuthn/Passkey configuration.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `rp_id` | string | `"localhost"` | Relying party ID for WebAuthn |
| `rp_origin` | string | `"http://localhost:8080"` | Origin for WebAuthn |
| `rp_name` | string | `"AgentHub"` | Display name for WebAuthn |
| `passkey_enabled` | boolean | `false` | Initial passkey state; root can change it later in Admin settings |

**Production WebAuthn Setup**:

WebAuthn requires HTTPS and a valid origin:

```toml
[web]
rp_id = "agenthub.example.com"
rp_origin = "https://agenthub.example.com"
rp_name = "AgentHub Production"
```

### `[worktree]` Section

Worktree creation settings.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `default_root` | string | `"~/.agenthub/worktrees"` | Base directory for new worktrees |

### `[codex_acp]` Section

ACP (Agent Control Protocol) provider settings.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `binary` | string | `"agenthub-acp"` | Canonical AgentHub ACP adapter binary name or path |
| `default_mode` | string | `"full-access"` | Default Codex ACP mode (`read-only`, `auto`, or `full-access`; `yolo` normalizes to `full-access`) |
| `multi_agent_enabled` | boolean | `true` | Force Codex ACP `Feature::Collab` on AgentHub-managed sessions |

The canonical Codex command for new agents is `agenthub-acp codex`. The
`codex_acp.binary` setting points to the adapter executable only and should not
include the `codex` subcommand argument. Existing deployments can still set it
to `agenthub-codex-acp` or another compatible Codex ACP executable.

If you point `codex_acp.binary` at a custom binary, verify it against the ACP
protocol surface used by the exact AgentHub release before mixing versions in a
shared deployment.

Other ACP adapters are configured per agent command rather than through this
section. For Claude, prefer the AgentHub-distributed `agenthub-acp claude`
command. Compatibility commands `claude-agent-acp` and `claude-code-acp-rs --acp`
are also recognized. Configure Anthropic credentials through the adapter-supported
environment or Claude settings files.

### `[history]` Section

Event history retention settings.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `event_retention_days` | integer | `5` | Days to keep events (0 = unlimited) |
| `vacuum_on_cleanup` | boolean | `false` | Run VACUUM after cleanup |
| `delete_batch_size` | integer | `10000` | Rows per cleanup batch (100-200000) |

**Retention Example**:

```toml
[history]
# Keep events for 30 days
event_retention_days = 30
# Reclaim disk space after cleanup
vacuum_on_cleanup = true
# Smaller batches for lower I/O impact
delete_batch_size = 5000
```

### `[object_store]` Section

Object storage for uploaded files, generated artifacts, and image-hosting
objects. AgentHub keeps ownership, publish state, size, checksum, MIME type, and
cleanup eligibility in SQLite metadata; the object store only stores bytes.

The default backend is local filesystem storage. Official release binaries
include the OpenDAL S3 backend; default/source builds must enable the root
`object-store-s3` feature explicitly. Runtime storage remains `fs` until
`backend = "s3"` is configured.

Shipping the S3 backend is not a production certification for every
S3-compatible provider. Validate the exact release artifact against the target
service and bucket before rollout.
Actor-scoped uploads are available from the CLI:

```bash
agenthub actor upload --file report.json --scope teams/team-1
agenthub actor upload --file screenshot.png --scope teams/team-1 --image
```

When `backend = "fs"` and `root` is omitted, actor uploads use
`~/.agenthub/objects`.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `backend` | string | `"fs"` | Object backend: `fs` or `s3` |
| `root` | string | none | Filesystem root for `fs`; optional backend root/prefix for S3-compatible services |
| `prefix` | string | none | Deployment or tenant prefix joined ahead of generated object keys |
| `public_base_url` | string | none | Optional CDN/object-gateway base URL for image-hosting delivery |
| `bucket` | string | none | S3-compatible bucket name |
| `endpoint` | string | provider default | S3-compatible endpoint, for example AWS S3, R2, or MinIO |
| `region` | string | provider default | S3-compatible region |
| `access_key_id_env` | string | provider chain | Environment variable name that contains the access key id |
| `secret_access_key_env` | string | provider chain | Environment variable name that contains the secret access key |
| `download_max_bytes` | integer | `536870912` | Maximum server-side download size in bytes |
| `download_max_redirects` | integer | `5` | Maximum redirects for one download |
| `download_timeout_seconds` | integer | `120` | End-to-end download timeout |
| `download_retry_attempts` | integer | `3` | Total attempts for retryable download failures |
| `download_retry_backoff_millis` | integer | `250` | Initial retry backoff |
| `download_max_concurrent_per_host` | integer | `4` | Per-host concurrent download limit |
| `download_allow_private_networks` | boolean | `false` | Permit private, loopback, or link-local destinations |
| `download_allowed_hosts` | array of strings | `[]` | Optional allowlist; non-empty means unmatched hosts are rejected |
| `download_denied_hosts` | array of strings | `[]` | Explicit denylist evaluated before the allowlist |

**Local Object Store Example**:

```toml
[object_store]
backend = "fs"
root = "~/.agenthub/objects"
prefix = "agenthub/local"
```

**S3-Compatible Object Store Example**:

```toml
[object_store]
backend = "s3"
bucket = "agenthub-artifacts"
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
prefix = "agenthub/prod"
public_base_url = "https://img.example.com"
access_key_id_env = "AGENTHUB_OBJECT_STORE_ACCESS_KEY_ID"
secret_access_key_env = "AGENTHUB_OBJECT_STORE_SECRET_ACCESS_KEY"
download_allowed_hosts = ["downloads.example.com", "*.cdn.example.com"]
download_denied_hosts = ["metadata.google.internal", "169.254.169.254"]
```

`access_key_id_env` and `secret_access_key_env` are references to environment
variable names. Do not put secret values directly in `config.toml`.

For image hosting, `public_base_url` is only a delivery URL helper. It does not
grant create, read, replace, or delete permission. AgentHub API handlers must
authorize against the owning metadata row before returning an image URL or
mutating an object. The current image-hosting helper accepts only raster image
MIME types: `image/png`, `image/jpeg`, `image/webp`, and `image/gif`.

Server-side download ingestion permits only `http` and `https`. Private,
loopback, link-local, metadata-service, and rebinding targets remain blocked by
default. Keep that default unless the deployment has a narrowly reviewed need.

### `[message_archive]` Section

Searchable Team and agent message documents use the LanceDB-backed archive.
The archive is a derived search surface, not control-plane authority.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `backend` | string | `"lancedb"` | Archive backend |
| `uri` | string | `"~/.agenthub/message-archive"` | Archive directory |
| `message_table` | string | `"messages"` | LanceDB message table |

If archive initialization or readiness fails, AgentHub logs the error and runs
without that search surface. Include the archive directory in backups when you
want to preserve it, but treat SQLite as the source for control-plane state.

### `[message_body_store]` Section

The optional RocksDB body store keeps compressed message-body copies while
SQLite remains the compatibility source.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable the tiered message-body store |
| `path` | string | `"~/.agenthub/message-bodies"` | RocksDB directory |
| `auto_migrate` | boolean | `true` | Backfill compatible SQLite bodies at startup after the store is enabled |

This option requires a binary built with the root `rocksdb` feature. Official
release artifacts include it; a default source build does not. Keep the main
SQLite data in backups even when this store is enabled.

### `[push]` Section

Web Push notification settings.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `subject` | string | `"mailto:admin@example.com"` | VAPID contact email |
| `keys_path` | string | `"~/.agenthub/vapid.json"` | Path to VAPID keys |

**Push Configuration**:

```toml
[push]
subject = "mailto:ops@company.com"
keys_path = "/etc/agenthub/vapid.json"
```

Push delivery still depends on a browser that supports service workers and the
Push API. Outside `localhost`, use HTTPS.

### `[internal_grpc]` Section

Internal gRPC control plane for remote nodes and actor CLI.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable internal gRPC server |
| `listen` | string | `"127.0.0.1:50051"` | gRPC bind address |

#### `[internal_grpc.security]` Subsection

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mode` | string | `"tls"` | Security mode: `tls`, `mtls`, or `disabled` |
| `cert_dir` | string | `"~/.agenthub/internal-grpc"` | TLS certificate directory |

#### `[internal_grpc.auth]` Subsection

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `shared_secret` | string | none | JWT signing secret |
| `issuer` | string | none | JWT issuer claim |
| `audience` | string | none | JWT audience claim |

#### `[internal_grpc.bootstrap]` Subsection

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `token` | string | none | Bootstrap token for token-based Agent Node join |

This token is the primary Agent Node onboarding path. Operators copy it from the
`Agents` page, configure it on the remote node, and then register the node's
reachable `grpc_target` in the UI. QR onboarding is not used for Agent Nodes.

Bootstrap scope:

- `internal_grpc.bootstrap.token` authenticates the remote-node join/bootstrap
  handshake.
- `internal_grpc.auth.shared_secret`, `issuer`, and `audience` still define the
  internal gRPC JWT contract used after bootstrap and by `agenthub actor ...`.
- The node registry stores routing metadata only; it does not store bootstrap
  tokens, shared secrets, or TLS file paths.
- In `server.role = "node"` mode, `server.node_id` must be explicit and must
  not be `main`.

**Complete Internal gRPC Example**:

```toml
[server]
role = "node"
node_id = "node-east"

[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/internal-grpc"

[internal_grpc.auth]
shared_secret = "your-256-bit-secret-here-minimum-32-chars"
issuer = "agenthub"
audience = "agenthub-internal"

[internal_grpc.bootstrap]
token = "bootstrap-token-for-remote-nodes"
```

### `[proxy]` Section

HTTP proxy configuration.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `http` | string | none | HTTP proxy URL |
| `https` | string | none | HTTPS proxy URL |
| `all` | string | none | Proxy for all protocols |

**Proxy Example**:

```toml
[proxy]
http = "http://proxy.company.com:8080"
https = "http://proxy.company.com:8080"
```

## Environment Boundary

The server does not currently apply environment overrides for normal TOML
settings. Variables such as `AGENTHUB_LISTEN` and `AGENTHUB_INTERNAL_GRPC_*`
are detected only so startup can warn that they were ignored. Put those
values in `~/.agenthub/config.toml` instead.

Environment variables still have explicit uses at other boundaries:

- `access_key_id_env` and `secret_access_key_env` name the variables from which
  the S3 backend reads credentials.
- ACP provider commands inherit the service environment according to the
  process-launch policy.
- `AGENTHUB_SERVER_URL` and `AGENTHUB_TOKEN` are defaults for `agenthub doctor`,
  not server configuration overrides.
- Pyroscope uses the three variables documented below.

Treat an `env overrides ignored` startup warning as a configuration error to
correct, not proof that the requested value was applied.

## Pyroscope Profiling

Continuous profiling is an operational opt-in and currently uses environment variables instead of
`~/.agenthub/config.toml`.

AgentHub starts a process-wide Pyroscope profiler only when all three variables are present with
non-empty values:

- `PYROSCOPE_SERVER_ADDRESS`
- `PYROSCOPE_BASIC_AUTH_USER`
- `PYROSCOPE_BASIC_AUTH_PASSWORD`

If only part of the configuration is present, AgentHub logs a warning and continues without
profiling.

Example:

```bash
export PYROSCOPE_SERVER_ADDRESS="https://pyroscope.example.com"
export PYROSCOPE_BASIC_AUTH_USER="agenthub"
export PYROSCOPE_BASIC_AUTH_PASSWORD="super-secret"
agenthub
```

Main-mode processes use the Pyroscope application name `agenthub.server`.
Node-mode processes use `agenthub.node`.

## Workdir Access

AgentHub does not restrict which directories an agent or Team workdir can
point at; there is no allowlist to configure. Operators choose workdirs
directly, and the agent process runs with whatever filesystem access the
AgentHub service account has. Use OS-level controls — a dedicated service
account, filesystem permissions, containers/VMs — to scope that access. See
[Security and Path Safety](../operations/security-and-path-safety.md).

## Configuration Validation

After updating configuration:

1. **Restart AgentHub**:
   ```bash
   sudo systemctl restart agenthub.service
   ```

   For a foreground process, stop it normally and start `agenthub` again.

2. **Verify Server Startup**:
   ```bash
   curl --fail http://127.0.0.1:8080/health
   ```

3. **Test Configuration**:
   ```bash
   # Create a test agent
   # Test internal gRPC (if enabled)
   agenthub actor inbox --actor-id test --limit 1
   ```

## Production Configuration Example

```toml
# Production AgentHub Configuration
[server]
listen = "127.0.0.1:8080"

[web]
rp_id = "agenthub.company.com"
rp_origin = "https://agenthub.company.com"
rp_name = "Company AgentHub"
passkey_enabled = false

[worktree]
default_root = "/var/agenthub/worktrees"

[history]
event_retention_days = 30
vacuum_on_cleanup = true
delete_batch_size = 5000

[push]
subject = "mailto:ops@company.com"

[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/certs"

[internal_grpc.auth]
shared_secret = "<load-a-long-random-secret-from-your-deployment-secret-store>"
issuer = "agenthub"
audience = "agenthub-internal"

[proxy]
http = "http://proxy.company.com:8080"
https = "http://proxy.company.com:8080"

# Log to file for log rotation
log_path = "/var/log/agenthub/agenthub.log"
```

## Troubleshooting

### Config Changes Not Applied

- Configuration is loaded at startup; restart required
- Check file permissions on `~/.agenthub/config.toml`
- Verify TOML syntax is valid

### Internal gRPC Connection Failures

- Ensure `shared_secret` is explicitly set (not auto-generated)
- Verify firewall allows traffic on gRPC port
- Check TLS certificates exist in `cert_dir`

### Workdir Errors

- Ensure the workdir path exists and is readable by the service account
- Remember that `~` is expanded to `$HOME`
- Verify worktree directories are writable
