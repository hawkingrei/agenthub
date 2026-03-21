const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
type WorktreeMode = "use_existing" | "create_worktree" | "reuse_worktree";
type NodeWorktreeRootSource = {
  id: string;
  default_worktree_root?: string | null;
};

export function normalizeWorkdirInput(value?: string | null): string {
  const trimmed = (value ?? "").trim();
  if (!trimmed) return "";
  const stripped = trimmed.replace(/[\\/]+$/, "");
  return stripped || trimmed;
}

function normalizePathForCompare(value?: string | null): string {
  return normalizeWorkdirInput(value);
}

export function normalizeRuntimeWorktreeRoot(
  value?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  return normalizeWorkdirInput(value) || fallback;
}

export function shouldApplyDefaultWorkdir(
  currentWorkdir?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): boolean {
  return !normalizePathForCompare(currentWorkdir) ||
    normalizePathForCompare(currentWorkdir) === normalizePathForCompare(fallback);
}

export function resolveWorkdirForRuntimeDefaults(
  currentWorkdir?: string | null,
  runtimeDefaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (!shouldApplyDefaultWorkdir(currentWorkdir, fallback)) {
    return normalizeWorkdirInput(currentWorkdir);
  }
  return normalizeRuntimeWorktreeRoot(runtimeDefaultRoot, fallback);
}

export function resolveWorkdirForCreateModal(
  currentWorkdir?: string | null,
  defaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (!shouldApplyDefaultWorkdir(currentWorkdir, fallback)) {
    return normalizeWorkdirInput(currentWorkdir);
  }
  return normalizeRuntimeWorktreeRoot(defaultRoot, fallback);
}

export function isDefaultWorkdirValue(
  currentWorkdir?: string | null,
  defaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): boolean {
  const current = normalizePathForCompare(currentWorkdir);
  if (!current) return true;
  const root = normalizePathForCompare(defaultRoot || fallback);
  const base = normalizePathForCompare(fallback);
  return current === root || current === base;
}

export function resolveWorkdirForModeChange(
  currentWorkdir: string | null | undefined,
  nextMode: WorktreeMode,
  defaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (nextMode === "create_worktree") {
    return resolveWorkdirForCreateModal(currentWorkdir, defaultRoot, fallback);
  }
  if (nextMode === "use_existing" && isDefaultWorkdirValue(currentWorkdir, defaultRoot, fallback)) {
    return "";
  }
  return normalizeWorkdirInput(currentWorkdir);
}

export function resolveDefaultWorktreeRootForTargetNode(
  targetNodeId: string | null | undefined,
  nodes: NodeWorktreeRootSource[],
  localDefaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  const normalizedLocalRoot = normalizeRuntimeWorktreeRoot(localDefaultRoot, fallback);
  const normalizedTargetNodeId = normalizeWorkdirInput(targetNodeId);
  if (!normalizedTargetNodeId || normalizedTargetNodeId === "main") {
    return normalizedLocalRoot;
  }
  const selectedNode = nodes.find((node) => node.id === normalizedTargetNodeId);
  return normalizeRuntimeWorktreeRoot(
    selectedNode?.default_worktree_root,
    normalizedLocalRoot
  );
}

export function resolveWorkdirForTargetNodeChange(
  currentWorkdir: string | null | undefined,
  nextMode: WorktreeMode,
  previousDefaultRoot?: string | null,
  nextDefaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  const normalizedCurrent = normalizeWorkdirInput(currentWorkdir);
  const normalizedPreviousRoot = normalizeRuntimeWorktreeRoot(previousDefaultRoot, fallback);
  const normalizedNextRoot = normalizeRuntimeWorktreeRoot(nextDefaultRoot, fallback);
  if (nextMode !== "create_worktree") {
    if (!normalizedCurrent || isDefaultWorkdirValue(normalizedCurrent, normalizedPreviousRoot, fallback)) {
      return "";
    }
    return normalizedCurrent;
  }
  if (!normalizedCurrent || isDefaultWorkdirValue(normalizedCurrent, normalizedPreviousRoot, fallback)) {
    return normalizedNextRoot;
  }
  return normalizedCurrent;
}

export function resolveWorkdirForModalOpen(
  currentWorkdir: string | null | undefined,
  mode: WorktreeMode,
  defaultRoot?: string | null,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (mode === "create_worktree") {
    return resolveWorkdirForCreateModal(currentWorkdir, defaultRoot, fallback);
  }
  if (mode === "use_existing" && isDefaultWorkdirValue(currentWorkdir, defaultRoot, fallback)) {
    return "";
  }
  return normalizeWorkdirInput(currentWorkdir);
}
