import { describe, expect, it } from "vitest";
import {
  normalizeRuntimeWorktreeRoot,
  resolveWorkdirForCreateModal,
  resolveWorkdirForRuntimeDefaults,
  shouldApplyDefaultWorkdir,
} from "./worktree_defaults";

const DEFAULT_ROOT = "~/.agenthub/worktrees";

describe("worktree defaults helpers", () => {
  it("normalizes runtime root and falls back when blank", () => {
    expect(normalizeRuntimeWorktreeRoot(" /custom/root ")).toBe("/custom/root");
    expect(normalizeRuntimeWorktreeRoot("   ")).toBe(DEFAULT_ROOT);
  });

  it("detects when workdir should be replaced by default", () => {
    expect(shouldApplyDefaultWorkdir("")).toBe(true);
    expect(shouldApplyDefaultWorkdir("   ")).toBe(true);
    expect(shouldApplyDefaultWorkdir(DEFAULT_ROOT)).toBe(true);
    expect(shouldApplyDefaultWorkdir("/tmp/custom")).toBe(false);
  });

  it("keeps user-edited workdir during runtime default hydration", () => {
    expect(resolveWorkdirForRuntimeDefaults("/tmp/custom", "/srv/worktrees")).toBe(
      "/tmp/custom"
    );
  });

  it("hydrates runtime default root when workdir is empty", () => {
    expect(resolveWorkdirForRuntimeDefaults("", "/srv/worktrees")).toBe(
      "/srv/worktrees"
    );
  });

  it("replaces fallback placeholder with runtime default root", () => {
    expect(
      resolveWorkdirForRuntimeDefaults(DEFAULT_ROOT, "/srv/runtime-worktrees")
    ).toBe("/srv/runtime-worktrees");
  });

  it("prefills create modal only when workdir is not customized", () => {
    expect(
      resolveWorkdirForCreateModal("", "/srv/default-worktrees", DEFAULT_ROOT)
    ).toBe("/srv/default-worktrees");
    expect(
      resolveWorkdirForCreateModal(DEFAULT_ROOT, "/srv/default-worktrees", DEFAULT_ROOT)
    ).toBe("/srv/default-worktrees");
    expect(
      resolveWorkdirForCreateModal("/tmp/custom", "/srv/default-worktrees", DEFAULT_ROOT)
    ).toBe("/tmp/custom");
  });
});

