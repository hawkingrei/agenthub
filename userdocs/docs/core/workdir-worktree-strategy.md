---
sidebar_position: 2
---

# Workdir and Worktree Strategy

The workspace mode decides which checkout an agent can change. Choose it before
starting the runtime; changing strategy normally means creating a new agent.

## Workspace Modes

| Mode | Behavior | Use when |
|------|----------|----------|
| `use_existing` | Runs directly in the selected directory. | The current checkout and its uncommitted state are intentionally part of the task. |
| `create_worktree` | Runs `git worktree add` from a repository/ref into an isolated workdir. | You want parallel, reviewable work without sharing checkout state. |
| `reuse_worktree` | Uses an existing Git worktree path without creating it. | A worktree was prepared outside AgentHub and should remain the execution boundary. |

Prefer `create_worktree` for implementation, dependency updates, and parallel
agents. Prefer `use_existing` for read-only diagnosis or work that explicitly
depends on the current checkout. `reuse_worktree` is useful when another
workflow owns branch/worktree creation.

## `create_worktree` Inputs

- **Repository path** must identify a Git repository visible on the selected
  execution node.
- **Repository ref** defaults to `HEAD` and must be valid for `git worktree add`.
- **Workdir** may be generated below the default worktree root or explicitly
  set to any path reachable by the execution node's service account.

If the target directory is a registered worktree for the same agent, AgentHub
can reuse it. It rejects a non-empty ordinary directory and a worktree already
bound to another agent. A configured ref mismatch is logged when an existing
worktree is reused; AgentHub does not silently rewrite that checkout.

## Workdir Location

AgentHub does not restrict which directory a workdir can point at; operators
and users choose the path directly, and `create_worktree` falls back to the
local default root `~/.agenthub/worktrees` (or a node's registered default)
only when no explicit workdir is given:

```toml
[worktree]
default_root = "/srv/agenthub/worktrees"
```

Because there is no path allowlist, the agent subprocess can read anything
else the service account can read. Scope the OS-level account, filesystem
permissions, and container/VM boundaries deliberately instead of relying on
workdir choice for isolation — see
[Security and Path Safety](../operations/security-and-path-safety.md).

## Remote Nodes

Repository and workdir paths are resolved on the selected node, not the main
control plane. A remote node can define **Default worktree root** in its
registry record. Without that default, provide an explicit workdir for
`create_worktree`.

Before starting a remote worktree agent, confirm:

- Git is installed on the node.
- The repository/ref exists on that node.
- The service account can create the workdir.

## Repository State

Before assigning an existing checkout, inspect it:

```bash
git -C /path/to/repo status --short
git -C /path/to/repo worktree list
```

Do not automatically stash, commit, reset, or force-remove user changes. Decide
who owns existing modifications, then choose a separate worktree when ownership
is unclear.

`create_worktree` shares the repository object database but has its own index
and working tree. It does not by itself create, push, merge, or delete a branch;
those actions remain part of the agent/user Git workflow.

## Cleanup

Deleting an AgentHub agent is not a substitute for reviewing its Git state.
Before removing a worktree:

1. Stop the agent.
2. Inspect `git status --short` in the worktree.
3. Preserve required changes in a commit, patch, or other reviewed artifact.
4. Confirm no other agent references the path.
5. Remove it with Git from the owning repository:

   ```bash
   git -C /path/to/repo worktree remove /path/to/worktree
   ```

6. Run `git -C /path/to/repo worktree prune` only after verifying stale entries.

Avoid scheduled `rm -rf` cleanup. Git worktrees can contain uncommitted user
work, and filesystem age does not prove that a workspace is safe to delete.

## Failure Guide

| Error | Check |
|-------|-------|
| `worktree_repo required` | Repository path is set for `create_worktree`. |
| Worktree does not exist | `reuse_worktree` points to a real checkout. |
| Workdir is not empty | Use another path or deliberately select the registered worktree. |
| Worktree belongs to another agent | Keep separate workdirs or stop and reassign ownership explicitly. |
| `git worktree add failed` | Repository/ref validity, existing branch checkout, permissions, and full Git stderr. |

See [Security and Path Safety](../operations/security-and-path-safety.md) for
the process-isolation boundary.
