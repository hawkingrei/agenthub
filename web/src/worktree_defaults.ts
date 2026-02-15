const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
type WorktreeMode = "use_existing" | "create_worktree" | "reuse_worktree";

function normalizePathForCompare(value: string): string {
  return value.trim().replace(/[\\/]+$/, "");
}

export function normalizeRuntimeWorktreeRoot(
  value: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  const trimmed = value.trim();
  return trimmed || fallback;
}

export function shouldApplyDefaultWorkdir(
  currentWorkdir: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): boolean {
  return !normalizePathForCompare(currentWorkdir) ||
    normalizePathForCompare(currentWorkdir) === normalizePathForCompare(fallback);
}

export function resolveWorkdirForRuntimeDefaults(
  currentWorkdir: string,
  runtimeDefaultRoot: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (!shouldApplyDefaultWorkdir(currentWorkdir, fallback)) {
    return currentWorkdir;
  }
  return normalizeRuntimeWorktreeRoot(runtimeDefaultRoot, fallback);
}

export function resolveWorkdirForCreateModal(
  currentWorkdir: string,
  defaultRoot: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (!shouldApplyDefaultWorkdir(currentWorkdir, fallback)) {
    return currentWorkdir;
  }
  return defaultRoot;
}

export function isDefaultWorkdirValue(
  currentWorkdir: string,
  defaultRoot: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): boolean {
  const current = normalizePathForCompare(currentWorkdir);
  if (!current) return true;
  const root = normalizePathForCompare(defaultRoot || fallback);
  const base = normalizePathForCompare(fallback);
  return current === root || current === base;
}

export function resolveWorkdirForModeChange(
  currentWorkdir: string,
  nextMode: WorktreeMode,
  defaultRoot: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (nextMode === "create_worktree") {
    return resolveWorkdirForCreateModal(currentWorkdir, defaultRoot, fallback);
  }
  if (nextMode === "use_existing" && isDefaultWorkdirValue(currentWorkdir, defaultRoot, fallback)) {
    return "";
  }
  return currentWorkdir;
}

export function resolveWorkdirForModalOpen(
  currentWorkdir: string,
  mode: WorktreeMode,
  defaultRoot: string,
  fallback: string = DEFAULT_WORKTREE_ROOT
): string {
  if (mode === "create_worktree") {
    return resolveWorkdirForCreateModal(currentWorkdir, defaultRoot, fallback);
  }
  if (mode === "use_existing" && isDefaultWorkdirValue(currentWorkdir, defaultRoot, fallback)) {
    return "";
  }
  return currentWorkdir;
}
