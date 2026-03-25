---
sidebar_position: 2
---

# Security and Path Safety

This guide covers security best practices for deploying and operating AgentHub safely.

## Security Model Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Security Layers                       │
├─────────────────────────────────────────────────────────┤
│  1. Authentication: Who are you?                        │
│     - Password-based login                              │
│     - WebAuthn/Passkey support                          │
│     - Token-based API access                            │
├─────────────────────────────────────────────────────────┤
│  2. Authorization: What can you do?                     │
│     - Role-based access (root/user)                     │
│     - Path-based restrictions                           │
│     - Operation-level controls                          │
├─────────────────────────────────────────────────────────┤
│  3. Execution Isolation: What can the agent access?     │
│     - Safe path restrictions                            │
│     - Workdir boundaries                                │
│     - Network policies                                  │
├─────────────────────────────────────────────────────────┤
│  4. Data Protection: How is data secured?               │
│     - Encryption at rest                                │
│     - TLS in transit                                    │
│     - Audit logging                                     │
└─────────────────────────────────────────────────────────┘
```

## Path Safety

### Principle: Least Path Access

Only allow paths users actually need.

**Good examples**:
- `/home/you/projects/agenthub` - Specific project directory
- `/home/you/workspace/team-a` - Team workspace
- `/data/sandboxes/experiments` - Isolated sandbox

**Avoid**:
- `/home/you` - Too broad, includes personal files
- `/` - System-wide access
- `/etc`, `/usr` - System directories
- Network mounts with sensitive data

### Path Validation

AgentHub validates paths at multiple stages:

1. **Configuration time**: `safe_paths` loaded from config
2. **Agent creation**: Workdir validated against allowed paths
3. **Runtime**: Agent process restricted to workdir context

### Safe Paths Configuration

```toml
# ~/.agenthub/config.toml
safe_paths = [
  # Development directories
  "/home/dev/projects",
  "/home/dev/experiments",
  
  # Team workspaces
  "/data/team-a/workspace",
  "/data/team-b/sandboxes",
  
  # CI/CD builds
  "/var/ci/builds",
  
  # Production (read-only operations)
  "/opt/services/configs",
]
```

**Important**: `~/.agenthub/worktrees` is automatically included.

### Path Security Checklist

- [ ] Paths are as specific as possible
- [ ] No parent directories of sensitive data
- [ ] No world-writable directories
- [ ] Separate paths for different teams if needed
- [ ] Regular audit of effective paths

## Authentication

### Password-Based Authentication

Default authentication method:

```toml
# Configure password policy (if applicable)
[auth]
min_password_length = 12
require_special_chars = true
```

**Best practices**:
- Use strong, unique passwords
- Enable account lockout after failed attempts
- Rotate passwords regularly

### WebAuthn/Passkey Support

Modern passwordless authentication:

```toml
[web]
rp_id = "agenthub.company.com"
rp_origin = "https://agenthub.company.com"
rp_name = "Company AgentHub"
```

**Setup**:
1. Login with password
2. Go to Settings → Security
3. Register passkey (Touch ID, YubiKey, etc.)
4. Use passkey for future logins

**Benefits**:
- Phishing-resistant
- No passwords to remember
- Hardware-backed security

### API Token Security

```bash
# Get token via login
TOKEN=$(curl -X POST http://localhost:8080/api/login \
  -d '{"username":"user","password":"pass"}' \
  | jq -r '.token')

# Use in API calls
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/agents
```

**Token Security**:
- Store tokens in environment variables or secrets manager
- Never commit tokens to version control
- Rotate tokens periodically
- Revoke tokens on logout/device removal

## Authorization

### Role-Based Access Control

| Role | Permissions |
|------|-------------|
| `root` | Full system access, user management, node registration |
| `user` | Create agents, view own agents, participate in teams |

### Root-Only Operations

Require root privileges:

- Register Agent Nodes
- Rotate VAPID keys
- Manage users
- View all agents (admin mode)
- System configuration changes

### Team-Level Permissions

Teams have their own permission model:

- **Leader**: Planning, task assignment, final approval
- **Worker**: Implementation, status updates
- **Human operators**: Conversation, oversight

## Execution Isolation

### Workdir Boundaries

Agent processes are constrained to their configured workdir:

```
Agent Process
├── Workdir: /home/dev/projects/myapp
│   ├── Can read/write here
│   └── Can execute commands here
├── Cannot access: /home/dev/other-project
├── Cannot access: /etc/passwd
└── Cannot access: /
```

**Implementation**:
- Process working directory set to workdir
- Agent tools validate paths before operations
- Configuration prevents path traversal

### Container Isolation (Advanced)

For stronger isolation, run agents in containers:

```bash
# Wrap agent execution in Docker
docker run \
  --rm \
  -v "${WORKDIR}:${WORKDIR}:rw" \
  -w "${WORKDIR}" \
  --network none \
  --security-opt no-new-privileges \
  agent-runtime:latest \
  "$@"
```

**Configuration**:
```toml
[codex_acp]
binary = "/usr/local/bin/agenthub-docker-wrapper"
```

### Network Policies

Restrict agent network access:

```bash
# iptables example: Allow only specific egress
iptables -A OUTPUT -p tcp --dport 443 -d github.com -j ACCEPT
iptables -A OUTPUT -p tcp --dport 443 -d api.openai.com -j ACCEPT
iptables -A OUTPUT -p tcp -j DROP
```

## Data Protection

### Encryption at Rest

AgentHub stores data in SQLite databases:

| Data | Location | Protection |
|------|----------|------------|
| Main database | `~/.agenthub/agenthub.db` | File permissions |
| Event databases | `~/.agenthub/agent-events/*.db` | File permissions |
| Config | `~/.agenthub/config.toml` | File permissions |
| Secrets | `~/.agenthub/*.json` | File permissions |

**Recommended**:
```bash
# Set restrictive permissions
chmod 700 ~/.agenthub
chmod 600 ~/.agenthub/config.toml
chmod 600 ~/.agenthub/vapid.json
```

**Full Disk Encryption**:
- Enable LUKS on Linux
- Enable FileVault on macOS
- Enable BitLocker on Windows

### Encryption in Transit

**HTTP/HTTPS**:
```toml
[server]
# For production, use reverse proxy (nginx, traefik)
# with TLS termination
listen = "127.0.0.1:8080"
```

**Internal gRPC**:
```toml
[internal_grpc.security]
mode = "tls"  # or "mtls" for mutual TLS
cert_dir = "/etc/agenthub/certs"
```

### Secret Management

**Environment Variables** (for containers):
```bash
# Don't put secrets in config file
export AGENTHUB_INTERNAL_GRPC_AUTH_SHARED_SECRET="$(cat /run/secrets/grpc_secret)"
```

**HashiCorp Vault**:
```python
import hvac

client = hvac.Client(url='https://vault.company.com')
secret = client.secrets.kv.v2.read_secret_version(path='agenthub/grpc')
shared_secret = secret['data']['data']['shared_secret']
```

**Kubernetes Secrets**:
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: agenthub-secrets
type: Opaque
stringData:
  shared_secret: "secret-value"
  vapid_private_key: "key-value"
```

## Audit Logging

### What Gets Logged

| Event | Logged Data | Purpose |
|-------|-------------|---------|
| Login | User ID, IP, success/fail | Security monitoring |
| Agent creation | User, workdir, command | Activity trace |
| Agent start/stop | Agent ID, user, timestamp | Operational audit |
| Node registration | Node ID, user | Infrastructure changes |
| Permission review | Request, reviewer, decision | Compliance |

### Log Format

```json
{
  "timestamp": "2026-03-25T10:30:00Z",
  "level": "INFO",
  "event": "agent_started",
  "agent_id": "agent-123",
  "user_id": "user-456",
  "ip": "10.0.0.1",
  "workdir": "/home/dev/project"
}
```

### Audit Log Configuration

```toml
[logging]
level = "info"
format = "json"  # or "text"
output = "/var/log/agenthub/audit.log"
```

### Log Retention

```bash
# Rotate logs daily
logrotate /etc/logrotate.d/agenthub

# Keep 30 days
timeout 30
```

### Audit Log Monitoring

**SIEM Integration**:
```bash
# Forward to Splunk/ELK
filebeat -c /etc/filebeat/filebeat.yml
```

**Alerting Rules**:
```yaml
# Example: Alert on failed logins
rules:
  - name: Failed Login Spike
    condition: count(event="login_failed") > 10 per 5m
    action: notify_security_team
```

## Security Best Practices

### Deployment Security

1. **Run as non-root**:
   ```bash
   useradd -r -s /bin/false agenthub
   sudo -u agenthub agenthub
   ```

2. **Use dedicated service account** (Kubernetes):
   ```yaml
   serviceAccountName: agenthub
   securityContext:
     runAsNonRoot: true
     runAsUser: 1000
     readOnlyRootFilesystem: true
   ```

3. **Resource limits**:
   ```yaml
   resources:
     limits:
       cpu: "2"
       memory: "4Gi"
     requests:
       cpu: "100m"
       memory: "512Mi"
   ```

### Operational Security

**Daily**:
- Review failed login attempts
- Check for unauthorized path access
- Monitor agent activity

**Weekly**:
- Audit user list
- Review agent nodes
- Check log integrity

**Monthly**:
- Rotate secrets
- Update TLS certificates
- Security patch review

### Incident Response

**Containment**:
1. Stop suspicious agents immediately
2. Revoke compromised tokens
3. Disable affected user accounts

**Investigation**:
1. Review audit logs
2. Check event databases
3. Analyze agent outputs

**Recovery**:
1. Patch vulnerabilities
2. Rotate all secrets
3. Rebuild affected nodes

## Hardening Checklist

### Pre-Deployment

- [ ] Review and minimize `safe_paths`
- [ ] Enable TLS for all communications
- [ ] Configure strong authentication
- [ ] Set up audit logging
- [ ] Plan secret management strategy

### Deployment

- [ ] Run as non-root user
- [ ] Enable firewall rules
- [ ] Configure resource limits
- [ ] Set up log aggregation
- [ ] Enable monitoring/alerting

### Post-Deployment

- [ ] Regular access reviews
- [ ] Secret rotation schedule
- [ ] Backup and recovery tested
- [ ] Incident response plan documented
- [ ] Security training for users

## Common Security Pitfalls

### 1. Overly Permissive safe_paths

```toml
# BAD - Too broad
safe_paths = ["/home"]

# GOOD - Specific directories
safe_paths = [
  "/home/dev/projects",
  "/home/dev/sandboxes"
]
```

### 2. Hardcoded Secrets

```toml
# BAD
[internal_grpc.auth]
shared_secret = "my-secret-123"

# GOOD - Use environment variable
# AGENTHUB_INTERNAL_GRPC_AUTH_SHARED_SECRET loaded at runtime
```

### 3. Missing TLS

```toml
# BAD
[internal_grpc.security]
mode = "disabled"

# GOOD
[internal_grpc.security]
mode = "mtls"
```

### 4. Running as Root

```bash
# BAD
sudo agenthub

# GOOD
sudo -u agenthub agenthub
```

## Compliance

### SOC 2 Considerations

- **CC6.1**: Logical access security (authentication/authorization)
- **CC6.2**: Access removal (user offboarding)
- **CC7.1**: Security monitoring (audit logs)
- **CC7.2**: Incident detection (monitoring/alerting)

### GDPR Considerations

- Audit logs contain user activity
- Event databases store conversation history
- Implement data retention policies
- Provide data export/deletion capabilities

## Security Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CIS Benchmarks](https://www.cisecurity.org/cis-benchmarks)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)

## Reporting Security Issues

If you discover a security vulnerability:

1. Do not open a public issue
2. Email security@yourcompany.com
3. Provide detailed reproduction steps
4. Allow time for patch before disclosure
