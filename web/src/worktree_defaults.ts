const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
type WorktreeMode = "use_existing" | "create_worktree" | "reuse_worktree";

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
  currentWorkdir?: string | null,
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

export function resolveWorkdirForModalOpen(
  currentWorkdir?: string | null,
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
