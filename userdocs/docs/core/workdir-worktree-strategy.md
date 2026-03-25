---
sidebar_position: 2
---

# Workdir and Worktree Strategy

Choosing the right run location is the biggest lever for stable agent behavior. This guide covers workdir modes, worktree management, and production best practices.

## Two Modes

### `use_existing` Mode

Run agent directly in an existing directory.

**Best for**:
- Direct access to long-lived local workspaces
- Incremental work on existing branches
- Scenarios where local uncommitted state should be visible
- Quick experiments in familiar directories

**Trade-offs**:
- Agent sees all local uncommitted changes
- Risk of conflicts with your local work
- Cleanup is manual

### `create_worktree` Mode

Create an isolated git worktree under your configured root.

**Best for**:
- Reproducible isolated runs
- Parallel tasks on the same repository
- Clean cleanup boundaries after completion
- CI-like execution patterns

**Trade-offs**:
- Requires git repository
- Takes time to set up new worktree
- Uses more disk space

## How to Choose

| Scenario | Recommended Mode | Why |
|----------|-----------------|-----|
| Daily development | `use_existing` | Familiar context |
| Feature implementation | `create_worktree` | Isolated branch |
| Code review | `create_worktree` | Clean slate |
| Parallel experiments | `create_worktree` | No conflicts |
| Production fixes | `use_existing` | Quick access |
| Refactoring | `create_worktree` | Easy rollback |

## Git Worktree Deep Dive

### What Are Git Worktrees?

Git worktrees allow multiple working directories attached to the same repository. Each worktree:
- Has its own checkout state
- Shares the same `.git` object database
- Can be on different branches
- Can be deleted independently

### Worktree Structure

```
~/.agenthub/worktrees/
├── myproject-feature-a-abc123/     # Worktree 1
│   ├── .git                        # git file (points to main repo)
│   ├── src/
│   └── ...
├── myproject-feature-b-def456/     # Worktree 2
│   ├── .git
│   └── ...
└── myproject-hotfix-ghi789/        # Worktree 3
    ├── .git
    └── ...
```

### Managing Worktrees

**List all worktrees**:
```bash
cd ~/your-repo
git worktree list
```

**Create worktree manually**:
```bash
git worktree add ~/.agenthub/worktrees/myproject-feature ../path/to/repo
```

**Remove worktree**:
```bash
# Safe removal (keeps branch)
git worktree remove ~/.agenthub/worktrees/myproject-feature

# Force removal if dirty
git worktree remove --force ~/.agenthub/worktrees/myproject-feature
```

**Clean up stale worktrees**:
```bash
# Prune worktrees that no longer exist on disk
git worktree prune
```

## Configuration

### Default Worktree Root

```toml
[worktree]
default_root = "/var/agenthub/worktrees"
```

**Important**: This path is automatically added to `safe_paths`.

### Per-Node Default Root

For remote Agent Nodes, configure a node-specific default:

```toml
# In node registration UI or API
default_worktree_root = "/data/agenthub-worktrees"
```

This allows different filesystem layouts per execution node.

## Naming Convention Best Practices

### Agent Naming

Use predictable, parseable names:

```
<project>-<goal>-<date>
<project>-<ticket-id>-<brief-desc>
<team>-<sprint>-<feature>
```

Examples:
- `agenthub-docs-2026-03-15`
- `myapp-1234-user-auth`
- `platform-s3-refactor`

### Worktree Naming

AgentHub auto-generates worktree directories:

```
<repo-name>-<sanitized-goal>-<short-hash>
```

You can override by specifying explicit `worktree_repo` and `worktree_ref`.

## Safe Path Constraints

All run paths must be under configured `safe_paths`:

```toml
safe_paths = [
  "/home/you/projects",
  "/data/sandboxes",
]
```

**Effective safe paths** automatically include:
- All explicitly configured paths
- `worktree.default_root` (default: `~/.agenthub/worktrees`)

### Path Validation Flow

1. User specifies workdir or leaves blank for worktree
2. AgentHub resolves full path (expanding `~`)
3. Validates path is within allowed roots
4. Rejects if outside safe paths
5. Creates directory if needed

### Troubleshooting Path Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `path outside safe_paths` | Directory not allowed | Add to `safe_paths` or use allowed location |
| `workdir does not exist` | Path not found | Create directory first |
| `permission denied` | Filesystem permissions | Check owner and permissions |
| `symlink loop detected` | Circular symlinks | Use real paths, not symlinks |

## Production Best Practices

### Isolation Strategy

**High-risk operations** → Always `create_worktree`:
- Refactoring
- Dependency updates
- Database migrations
- Configuration changes

**Low-risk operations** → Can use `use_existing`:
- Documentation updates
- Log analysis
- Read-only investigations
- Quick fixes

### Disk Space Management

**Monitor worktree usage**:
```bash
# Check total worktree disk usage
du -sh ~/.agenthub/worktrees/* | sort -hr | head -20

# Find largest worktrees
find ~/.agenthub/worktrees -maxdepth 1 -type d -exec du -sh {} \; | sort -hr
```

**Automatic cleanup**:
- Delete completed agents regularly
- Archive important changes before deletion
- Set up cron job for old worktree pruning:
  ```bash
  # Delete worktrees older than 30 days
  find ~/.agenthub/worktrees -maxdepth 1 -type d -mtime +30 -exec rm -rf {} \;
  ```

### Repository Preparation

For best `create_worktree` performance:

1. **Keep repositories lean**:
   ```bash
   git gc --aggressive
   git remote prune origin
   ```

2. **Use shallow clones for large repos** (optional):
   ```bash
   git clone --depth 100 git@github.com:org/repo.git
   ```

3. **Configure git hooks** (if needed):
   ```bash
   # Hooks run in worktree context
   .git/hooks/post-checkout
   ```

### Branch Strategy

**Recommended workflow**:

1. Create worktree from clean main:
   ```
   Agent: myapp-feature-x
   Worktree mode: create_worktree
   Worktree repo: /home/you/myapp
   Worktree ref: main
   ```

2. Agent creates feature branch automatically

3. Work happens in isolated worktree

4. Changes merged via normal PR process

5. Worktree deleted after merge

## Remote Node Considerations

When using `create_worktree` on remote nodes:

1. **Node must have default root configured** OR you must specify explicit workdir
2. **Repository must be accessible** on the remote node
3. **Git must be installed** on the remote node
4. **Network storage** considerations for shared repositories

### Remote Worktree Example

```toml
# AgentHub config on remote node
[worktree]
default_root = "/data/worktrees"
```

Agent creation:
- Execution node: `node-east`
- Workdir mode: `create_worktree`
- Worktree repo: `/data/repos/myproject` (must exist on node-east)
- Workdir: (blank - uses node default)

Result: Worktree created at `/data/worktrees/myproject-<goal>-<hash>/`

## Migration Between Modes

**Switching from `use_existing` to `create_worktree`**:

1. Complete or stop current agent
2. Create new agent with `create_worktree`
3. Manually copy needed files if necessary
4. Update automation/scripts

**Migrating existing work**:

```bash
# Archive current state
cd /existing/workdir
git stash push -m "pre-migration"
git checkout -b migration-backup

# Create new worktree-based agent
# Copy over relevant files
# Continue work in isolated environment
```

## Cleanup Practice

### Regular Maintenance

**Daily**:
- Delete completed/failed agents you don't need
- Keep agents with useful context for reference

**Weekly**:
- Review disk usage
- Clean up stale worktrees

**Monthly**:
- Archive important worktrees
- Prune old agent event databases

### Safe Cleanup Checklist

Before deleting an agent:

- [ ] Changes committed to git (if applicable)
- [ ] Important output saved/logged
- [ ] Event history no longer needed
- [ ] Worktree can be recreated if needed

### Automated Cleanup Script

```bash
#!/bin/bash
# cleanup-old-agents.sh

# Delete agents older than 30 days (requires API access)
# Adjust retention as needed

RETENTION_DAYS=30
CUTOFF=$(date -d "${RETENTION_DAYS} days ago" +%s)

# Note: Implement using AgentHub API or database queries
# This is a placeholder for your automation

echo "Cleaning up agents older than ${RETENTION_DAYS} days"
```

## Common Pitfalls

### 1. Nested Worktrees

Don't create worktrees inside other worktrees:
```
❌ ~/.agenthub/worktrees/a/worktrees/b
✅ ~/.agenthub/worktrees/a
✅ ~/.agenthub/worktrees/b
```

### 2. Long Paths

Avoid deep directory structures:
```
❌ /very/long/path/to/the/actual/work/directory/that/exceeds/limits
✅ /data/worktrees/project-name
```

### 3. Special Characters

Avoid special characters in agent names:
```
❌ agent:foo/bar
❌ agent;drop table
✅ agent-foo-bar
```

### 4. Repository State

Don't use `create_worktree` with dirty repositories:
```bash
# Check repository state
git status
# Clean or commit changes first
git stash || git commit -am "WIP"
```

## Summary

| Aspect | `use_existing` | `create_worktree` |
|--------|---------------|-------------------|
| Isolation | Low | High |
| Setup speed | Fast | Slower |
| Cleanup | Manual | Automatic |
| Best for | Daily dev | Isolated tasks |
| Disk usage | Shared | Separate |
| Git required | No | Yes |

Choose based on your risk tolerance, cleanup needs, and repository hygiene preferences.
