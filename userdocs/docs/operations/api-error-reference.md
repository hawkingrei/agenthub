---
sidebar_position: 5
---

# API Error Reference

This page documents common API error responses from AgentHub and how to resolve them.

## Error Response Format

All API errors follow this JSON structure:

```json
{
  "error": "Human-readable error message"
}
```

HTTP status codes indicate the error category:

| Status Code | Meaning |
|-------------|---------|
| `400` | Bad Request - Invalid input parameters |
| `401` | Unauthorized - Authentication required or failed |
| `403` | Forbidden - Insufficient permissions |
| `404` | Not Found - Resource doesn't exist |
| `409` | Conflict - Resource state conflict |
| `500` | Internal Server Error - Server-side problem |

## Authentication Errors (401)

### `missing authorization token`

**Cause**: Request missing `Authorization` header.

**Solution**: Include the header:
```bash
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/...
```

### `invalid token`

**Cause**: Token is malformed, expired, or revoked.

**Solution**: 
- Re-authenticate via `/api/login` or `/api/join`
- Check token hasn't expired
- Verify token hasn't been revoked

### `root required`

**Cause**: Operation requires root/admin privileges.

**Solution**: 
- Log in as root user
- For automation, use root credentials
- Some operations (like node registration) require root

### `root already initialized`

**Cause**: Attempting to create root user when one already exists.

**Solution**: Root user already configured; use `/api/login` instead of `/api/join`.

### `invalid join token` / `join token expired` / `invalid pin`

**Cause**: Join/bootstrap flow validation failed.

**Solution**:
- Generate a new join challenge via admin API
- Complete join within token expiration time
- Verify PIN is correct

## Validation Errors (400)

### Common Field Validation

| Error | Cause | Solution |
|-------|-------|----------|
| `team name is required` | Missing `name` field | Provide non-empty team name |
| `title is required` | Missing task title | Include task title in request |
| `member_id is required` | Missing member reference | Specify which team member |
| `step_key is required` | Missing step identifier | Provide unique step key |
| `error_text is required` | Missing error details | Include error description |
| `spec must be an object` | Invalid spec format | Send spec as JSON object |

### Team Specification Errors

#### `spec.members must be an array`

Team definition missing members array.

```json
{
  "spec": {
    "members": [
      {"member_id": "coordinator", "description": "...", "skills": []}
    ],
    "entrypoint": {...}
  }
}
```

#### `spec.members must not be empty`

Team requires at least one member.

#### `spec.members[].member_id is required`

Each member needs a unique identifier.

#### `spec.coordinator_member_id must reference a defined member`

Coordinator ID must match one of the defined `member_id` values.

#### `spec.steps[].depends_on entries must be non-empty strings`

Dependency references must be valid step keys.

### Agent Creation Errors

#### `workdir is required`

**Cause**: Creating agent without specifying working directory.

**Solution**: Provide `workdir` field, or use `create_worktree` mode with `worktree_repo`.

#### `workdir path must be within allowed safe paths`

**Cause**: Specified path outside configured `safe_paths`.

**Solution**:
- Add path to `safe_paths` in config
- Choose a directory within existing safe paths
- Use `create_worktree` mode for automatic path derivation

### Agent Node Errors

#### `agent node id or name already exists`

**Cause**: Attempting to register node with duplicate ID or name.

**Solution**: Use unique identifier and name for each node.

## Not Found Errors (404)

### `agent not found`

**Cause**: Agent ID doesn't exist or you don't have access.

**Solution**:
- Verify agent ID is correct
- Check agent hasn't been deleted
- Ensure you're authenticated as the owner

### `team not found`

**Cause**: Team ID doesn't exist.

**Solution**: Use list endpoint (`GET /api/teams`) to verify team exists.

### `run not found`

**Cause**: Team run ID doesn't exist for this team.

### `task not found`

**Cause**: Task ID doesn't exist in this team.

### `conversation not found`

**Cause**: Conversation not found for task.

### `step not found`

**Cause**: Step key doesn't exist in run.

### `message not found`

**Cause**: Message ID not found in conversation.

### `agent node not found`

**Cause**: Specified node ID not registered.

## Conflict Errors (409)

### `team name already exists`

**Cause**: Team with this name already exists.

**Solution**: Choose a different name or update existing team.

### `team definition changed concurrently; reload and retry`

**Cause**: Optimistic locking failure - team was modified by another request.

**Solution**: Re-fetch team definition and retry operation.

### `step already exists for run`

**Cause**: Duplicate step key in run.

**Solution**: Use unique step keys within a run.

### `completed run cannot be resumed; use restart`

**Cause**: Attempting to resume a run that already completed.

**Solution**: Start a new run or use restart endpoint.

### `agent is already running`

**Cause**: Attempting to start an already running agent.

**Solution**: Wait for completion or stop current run first.

## Team Runtime Errors

### Permission Review Errors

#### `worker-originated permission review cannot route to worker`

**Cause**: Invalid routing - workers can't review their own requests.

**Solution**: Route to coordinator or another member.

#### `route=to_coordinator requires a coordinator member in spec.members`

**Cause**: Team spec missing coordinator definition.

**Solution**: Define a coordinator member and set `coordinator_member_id`.

### Message Routing Errors

#### `to_actor_id is required when route=to_member`

**Cause**: Direct member routing requires target member ID.

#### `route must be one of: broadcast, to_coordinator, to_member`

**Cause**: Invalid route type specified.

## Internal gRPC Errors

### `internal grpc client not available`

**Cause**: AgentHub not configured with `internal_grpc.enabled = true`.

**Solution**: Add to config:
```toml
[internal_grpc]
enabled = true
listen = "127.0.0.1:50051"
```

### `failed to connect to remote node`

**Cause**: Cannot reach registered Agent Node.

**Solution**:
- Verify node is running
- Check network connectivity
- Verify gRPC TLS certificates

## Troubleshooting API Errors

### Enable Debug Logging

Add to config for detailed error information:

```toml
[logging]
level = "debug"
```

### Common Diagnostic Steps

1. **Check Authentication**:
   ```bash
   curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/me
   ```

2. **Validate Request Body**:
   - Ensure JSON is well-formed
   - Check required fields are present
   - Verify data types match schema

3. **Check Server Logs**:
   ```bash
   # If running directly
   agenthub 2>&1 | grep -i error
   
   # If using systemd
   journalctl -u agenthub -f
   ```

4. **Verify Configuration**:
   - Check `~/.agenthub/config.toml` syntax
   - Ensure required sections are present
   - Validate paths exist and are accessible

### Getting Help

When reporting API errors, include:

1. HTTP status code
2. Full error message
3. Request URL and method
4. Request body (sanitized)
5. Server logs at debug level
6. AgentHub version (`agenthub --version`)
