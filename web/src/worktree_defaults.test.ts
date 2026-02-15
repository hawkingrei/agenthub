import { describe, expect, it } from "vitest";
import {
  isDefaultWorkdirValue,
  normalizeRuntimeWorktreeRoot,
  normalizeWorkdirInput,
  resolveWorkdirForCreateModal,
  resolveWorkdirForModalOpen,
  resolveWorkdirForModeChange,
  resolveWorkdirForRuntimeDefaults,
  shouldApplyDefaultWorkdir,
} from "./worktree_defaults";

const DEFAULT_ROOT = "~/.agenthub/worktrees";

describe("worktree defaults helpers", () => {
  it("normalizes runtime root and falls back when blank", () => {
    expect(normalizeRuntimeWorktreeRoot(" /custom/root ")).toBe("/custom/root");
    expect(normalizeRuntimeWorktreeRoot("   ")).toBe(DEFAULT_ROOT);
  });

  it("normalizes workdir input and keeps filesystem roots", () => {
    expect(normalizeWorkdirInput(" /tmp/work/ ")).toBe("/tmp/work");
    expect(normalizeWorkdirInput("/")).toBe("/");
    expect(normalizeWorkdirInput("\\")).toBe("\\");
    expect(normalizeWorkdirInput("   ")).toBe("");
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

  it("treats both fallback and runtime root as default values", () => {
    expect(
      isDefaultWorkdirValue("~/.agenthub/worktrees/", "/srv/default-worktrees", DEFAULT_ROOT)
    ).toBe(true);
    expect(isDefaultWorkdirValue("/srv/default-worktrees", "/srv/default-worktrees", DEFAULT_ROOT)).toBe(
      true
    );
    expect(isDefaultWorkdirValue("/tmp/custom", "/srv/default-worktrees", DEFAULT_ROOT)).toBe(
      false
    );
  });

  it("clears default workdir when switching to use_existing mode", () => {
    expect(
      resolveWorkdirForModeChange(
        "/srv/default-worktrees",
        "use_existing",
        "/srv/default-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("");
    expect(
      resolveWorkdirForModeChange(
        "/tmp/custom",
        "use_existing",
        "/srv/default-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("/tmp/custom");
  });

  it("fills runtime root only in create_worktree mode changes", () => {
    expect(
      resolveWorkdirForModeChange(
        "",
        "create_worktree",
        "/srv/default-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("/srv/default-worktrees");
    expect(
      resolveWorkdirForModeChange(
        "",
        "reuse_worktree",
        "/srv/default-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("");
  });

  it("clears default-like values on modal open for use_existing mode", () => {
    expect(
      resolveWorkdirForModalOpen(
        "/srv/default-worktrees",
        "use_existing",
        "/srv/default-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("");
    expect(
      resolveWorkdirForModalOpen(
        "/tmp/custom",
        "use_existing",
        "/srv/default-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("/tmp/custom");
  });
});
