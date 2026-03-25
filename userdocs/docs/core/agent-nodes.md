---
sidebar_position: 2
---

# Agent Nodes and Remote Execution

AgentHub can bind an agent to either the local `Main Node` or a registered remote Agent Node. This enables distributed execution while keeping a single control plane.

## Why Agent Nodes Matter

Agent Nodes are not just a deployment detail. They let AgentHub keep one control plane while moving execution closer to:

- **Data**: Large datasets that shouldn't move over network
- **Compute**: GPU/TPU resources or specialized hardware
- **Network**: Required network boundaries or VPN segments
- **Ownership**: Machines that should own local worktrees

This is also where the actor p2p model matters: mailbox and control traffic can stay consistent even when the process runs remotely.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Main Control Plane                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Web UI     │  │   API        │  │   Registry   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │         Internal gRPC Control Plane                │    │
│  │   (mTLS/TLS encrypted, authenticated)             │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                            │
            ┌───────────────┼───────────────┐
            │               │               │
            ▼               ▼               ▼
    ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
    │  Agent Node  │ │  Agent Node  │ │  Agent Node  │
    │  (node-gpu)  │ │  (node-east) │ │  (node-west) │
    └──────────────┘ └──────────────┘ └──────────────┘
```

## What Agent Nodes Control

Each registered node stores:

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Stable unique identifier (e.g., `node-gpu-01`) |
| `name` | Yes | Human-readable name |
| `grpc_target` | Yes | gRPC endpoint (e.g., `https://node1.internal:50051`) |
| `tls_server_name` | No | TLS SNI override for certificate validation |
| `default_worktree_root` | No | Default base for `create_worktree` mode |

The node registry is a control-plane view. Runtime state still lives on the selected execution node.

## Deployment Prerequisites

Before registering remote nodes, ensure:

### Main Control Plane

```toml
[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"  # or "mtls" for mutual TLS
cert_dir = "/etc/agenthub/certs"

[internal_grpc.auth]
shared_secret = "your-256-bit-secret-here"
issuer = "agenthub"
audience = "agenthub-internal"
```

### Remote Node

```toml
[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/certs"

[internal_grpc.auth]
shared_secret = "same-secret-as-main"  # Must match!
```

### Network Requirements

- Main control plane can reach remote node on gRPC port
- Firewalls allow bidirectional gRPC traffic
- DNS resolution (or IP addresses) configured

## TLS Configuration

### Mode: `tls` (Server Authentication)

Remote node presents certificate; main control plane verifies.

**Generate certificates**:

```bash
# On main control plane
cd /etc/agenthub/certs

# Generate CA
openssl req -x509 -newkey rsa:4096 -keyout ca-key.pem -out ca-cert.pem \
  -days 365 -nodes -subj "/CN=agenthub-ca"

# Generate server cert for remote node
openssl req -newkey rsa:4096 -keyout node-key.pem -out node-csr.pem \
  -nodes -subj "/CN=node1.internal"

openssl x509 -req -in node-csr.pem -CA ca-cert.pem -CAkey ca-key.pem \
  -out node-cert.pem -days 365 -CAcreateserial

# Copy to remote node
scp ca-cert.pem node-cert.pem node-key.pem node1.internal:/etc/agenthub/certs/
```

**Register node**:

```
ID: node-01
Name: GPU Node 01
gRPC Target: https://node1.internal:50051
TLS Server Name: node1.internal
Default Worktree Root: /data/agenthub/worktrees
```

### Mode: `mtls` (Mutual Authentication)

Both sides present and verify certificates. More secure but complex.

**Benefits**:
- Node cannot be impersonated
- Main control plane identity verified by nodes
- No shared secret needed (certificates only)

**Setup**:

1. Generate client certificates for main control plane
2. Distribute CA to all nodes
3. Configure `mode = "mtls"` on both sides

## Deployment Examples

### Example 1: Single Remote GPU Node

**Scenario**: Machine learning workloads on GPU server.

**Main Control Plane** (`hub.example.com`):
```toml
[server]
listen = "0.0.0.0:8080"

[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/certs"

[internal_grpc.auth]
shared_secret = "CHANGE-ME-256-BIT-SECRET"
```

**GPU Node** (`gpu01.internal`):
```toml
[internal_grpc]
enabled = true
listen = "0.0.0.0:50051"

[internal_grpc.security]
mode = "tls"
cert_dir = "/etc/agenthub/certs"

[internal_grpc.auth]
shared_secret = "CHANGE-ME-256-BIT-SECRET"

[worktree]
default_root = "/data/agenthub/worktrees"
```

**Registration**:
```bash
# Via UI or API
curl -X POST http://hub.example.com:8080/api/admin/agent-nodes \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "gpu-01",
    "name": "GPU Server 01",
    "grpc_target": "https://gpu01.internal:50051",
    "tls_server_name": "gpu01.internal",
    "default_worktree_root": "/data/agenthub/worktrees"
  }'
```

**Test**:
```bash
# Create agent targeting GPU node
# UI: Agents → Create → Execution Node: "GPU Server 01"
```

### Example 2: Multi-Region Deployment

**Scenario**: Teams in US and EU with local execution.

```
┌─────────────────┐         ┌─────────────────┐
│  Main Control   │◄───────►│  Main Control   │
│  (us-central)   │         │  (eu-west)      │
│  (passive)      │   sync  │  (active)       │
└────────┬────────┘         └────────┬────────┘
         │                           │
    ┌────┴────┐                 ┌────┴────┐
    ▼         ▼                 ▼         ▼
┌──────┐  ┌──────┐          ┌──────┐  ┌──────┐
│us-01 │  │us-02 │          │eu-01 │  │eu-02 │
└──────┘  └──────┘          └──────┘  └──────┘
```

### Example 3: Kubernetes Deployment

**Main Control Plane** (Deployment + Service):

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: agenthub-control
spec:
  replicas: 1
  selector:
    matchLabels:
      app: agenthub-control
  template:
    metadata:
      labels:
        app: agenthub-control
    spec:
      containers:
      - name: agenthub
        image: agenthub:latest
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 50051
          name: grpc
        volumeMounts:
        - name: config
          mountPath: /etc/agenthub
        - name: data
          mountPath: /data
      volumes:
      - name: config
        configMap:
          name: agenthub-config
      - name: data
        persistentVolumeClaim:
          claimName: agenthub-data
---
apiVersion: v1
kind: Service
metadata:
  name: agenthub-control
spec:
  selector:
    app: agenthub-control
  ports:
  - port: 8080
    name: http
  - port: 50051
    name: grpc
```

**Agent Node** (DaemonSet for node-local execution):

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: agenthub-node
spec:
  selector:
    matchLabels:
      app: agenthub-node
  template:
    metadata:
      labels:
        app: agenthub-node
    spec:
      hostNetwork: true
      containers:
      - name: agenthub
        image: agenthub:latest
        command: ["agenthub", "--node-mode"]
        ports:
        - containerPort: 50051
        volumeMounts:
        - name: workdir
          mountPath: /workdirs
      volumes:
      - name: workdir
        hostPath:
          path: /var/agenthub/workdirs
```

## Internal gRPC Also Powers `agenthub actor ...`

The same internal gRPC control plane is used by the actor CLI:

```bash
# These commands require internal gRPC:
agenthub actor team-members --actor-id <id>
agenthub actor inbox --actor-id <id> --run-id <run_id>
agenthub actor ack --actor-id <id> --message-id <msg_id>
agenthub actor send --actor-id <id> --to <recipient> --payload '{}'
agenthub actor time-trigger-list --actor-id <id>
agenthub actor permission-review-respond --request-id <id> --decision allow
```

**Important**: `internal_grpc.enabled = true` is required even on single-machine setups for actor CLI to work.

### Local Loopback Configuration

For local actor CLI usage:

```toml
[internal_grpc]
enabled = true
listen = "127.0.0.1:50051"

[internal_grpc.auth]
shared_secret = "local-dev-secret"
```

The CLI reads `shared_secret` from config to mint tokens. Auto-generated secrets (stored in `cert_dir/auth_secret.txt`) are **not** automatically used by CLI.

## Register and Edit Nodes

### Via Web UI

1. Go to `Agents` page
2. Click "Register Node" (root only)
3. Fill in node details
4. Test connection

### Via API

**Register node**:
```bash
curl -X POST http://localhost:8080/api/admin/agent-nodes \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "node-01",
    "name": "Production Node 01",
    "grpc_target": "https://node01.prod.internal:50051",
    "tls_server_name": "node01.prod.internal",
    "default_worktree_root": "/data/agenthub/worktrees"
  }'
```

**List nodes**:
```bash
curl http://localhost:8080/api/admin/agent-nodes \
  -H "Authorization: Bearer $TOKEN"
```

**Update node**:
```bash
curl -X PUT http://localhost:8080/api/admin/agent-nodes/node-01 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Updated Name",
    "default_worktree_root": "/new/path"
  }'
```

**Delete node**:
```bash
curl -X DELETE http://localhost:8080/api/admin/agent-nodes/node-01 \
  -H "Authorization: Bearer $TOKEN"
```

**Note**: Nodes with active agents cannot be deleted.

## Default Worktree Root

`Default worktree root` is optional and applies to remote `create_worktree` agents.

**With default root**:
- Leaving `Workdir` blank in `create_worktree` mode is allowed
- AgentHub derives workdir under node root
- Example: `/data/worktrees/myproject-goal-abc123/`

**Without default root**:
- Must provide explicit `Workdir`
- Full path must exist on remote node
- Must be within node's safe_paths

### Per-Node Worktree Strategy

| Node Type | Recommended Default Root | Use Case |
|-----------|------------------------|----------|
| GPU nodes | `/data/agenthub/worktrees` | ML workloads with large datasets |
| Build nodes | `/var/lib/agenthub/builds` | CI/CD compilation |
| Dev nodes | `/home/agenthub/worktrees` | Development environments |

## Execution Behavior

### Main Node

- Local safe-path and worktree policies apply directly
- No network overhead
- Filesystem access is direct

### Remote Node

- AgentHub proxies lifecycle control over encrypted gRPC
- Execution data stays on remote node
- UI/Control stays on main control plane
- Output streamed via gRPC to main plane, then to UI

### Network Flow

```
User → Main Control Plane → Remote Node
                ↓                ↓
              gRPC call    Process spawned
                ↓                ↓
           Status check   Output captured
                ↓                ↓
           Stream to UI ← Output via gRPC
```

## Actor P2P And Mailbox Delivery

Remote execution preserves the actor model:

- **Actor control**: Relayed over internal gRPC
- **Mailbox delivery**: Targets remote recipients through same path
- **Local state**: Remote nodes keep execution data
- **Central view**: Main node is primary control plane

This ensures remote execution feels like `AgentHub`, not a different product.

## Health Checking

### Manual Health Check

```bash
# Check gRPC endpoint
curl -v telnet://node01.internal:50051

# Check TLS
echo | openssl s_client -connect node01.internal:50051 2>/dev/null | openssl x509 -noout -text

# Check via AgentHub API
curl http://main-hub:8080/api/admin/agent-nodes/node-01/health \
  -H "Authorization: Bearer $TOKEN"
```

### Automated Monitoring

Monitor these metrics:

| Metric | Warning Threshold | Critical Threshold |
|--------|------------------|-------------------|
| gRPC latency | > 100ms | > 500ms |
| Connection failures | > 1% | > 10% |
| Agent start time | > 30s | > 60s |
| Disk usage (node) | > 80% | > 95% |

## Troubleshooting

### "failed to connect to remote node"

**Causes**:
- Network unreachable
- Firewall blocking
- Node not running
- TLS certificate mismatch

**Diagnostic**:
```bash
# From main control plane
grpcurl -insecure node01.internal:50051 list

# Check certificates
echo | openssl s_client -connect node01.internal:50051 -servername node01.internal
```

### "certificate verify failed"

**Solutions**:
- Verify `tls_server_name` matches certificate CN/SAN
- Check CA certificate is in trust store
- Regenerate certificates if expired

### "unauthorized"

**Causes**:
- `shared_secret` mismatch
- Token expired
- Clock skew between nodes

**Solutions**:
- Ensure same `shared_secret` on all nodes
- Check system clocks are synchronized (NTP)
- Restart nodes after secret changes

### Slow Agent Start on Remote Node

**Possible causes**:
- Network latency to node
- Slow worktree creation
- Resource constraints on node

**Solutions**:
- Use `use_existing` mode for faster starts
- Pre-warm worktree directories
- Monitor node resource usage

## Operator Rollout Flow

### New Node Onboarding

1. **Prepare node**:
   ```bash
   # Install AgentHub on remote machine
   # Copy TLS certificates
   # Create config file
   ```

2. **Start node**:
   ```bash
   agenthub
   # Verify gRPC port listening
   ss -tlnp | grep 50051
   ```

3. **Verify connectivity**:
   ```bash
   # From main control plane
   curl http://main-hub:8080/api/admin/agent-nodes \
     -H "Authorization: Bearer $TOKEN"
   ```

4. **Register node** (via UI or API)

5. **Set default worktree root** (optional)

6. **Test**:
   - Create remote-target agent
   - Verify card shows `node:<id>`
   - Start agent and confirm output visible

7. **Production readiness**:
   - Configure monitoring
   - Set up log aggregation
   - Document node capabilities

## Security Best Practices

1. **Use mTLS** for production multi-node deployments
2. **Rotate shared_secret** periodically
3. **Limit safe_paths** on each node to minimum required
4. **Use dedicated service accounts** for AgentHub processes
5. **Enable audit logging** for node registration/changes
6. **Network segmentation**: Place nodes in appropriate network zones

## Operational Tips

- **Stable IDs**: Use environment-oriented IDs like `node-east` or `build-fleet-a`
- **Naming**: Include region/purpose in names: `us-east-gpu-01`
- **Capacity planning**: Monitor node resource usage
- **Gradual rollout**: Validate one node before full deployment
- **Documentation**: Maintain node capability matrix (GPU, memory, etc.)
