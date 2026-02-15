const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";

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
  const trimmed = currentWorkdir.trim();
  return !trimmed || trimmed === fallback;
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

