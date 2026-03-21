import { describe, expect, it } from "vitest";
import {
  isDefaultWorkdirValue,
  normalizeRuntimeWorktreeRoot,
  normalizeWorkdirInput,
  resolveDefaultWorktreeRootForTargetNode,
  resolveWorkdirForCreateModal,
  resolveWorkdirForModalOpen,
  resolveWorkdirForModeChange,
  resolveWorkdirForRuntimeDefaults,
  resolveWorkdirForTargetNodeChange,
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
    expect(normalizeWorkdirInput(undefined)).toBe("");
    expect(normalizeWorkdirInput(null)).toBe("");
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
    expect(resolveWorkdirForRuntimeDefaults(undefined, undefined)).toBe(DEFAULT_ROOT);
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
    expect(resolveWorkdirForCreateModal(undefined, undefined, DEFAULT_ROOT)).toBe(DEFAULT_ROOT);
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

  it("resolves selected target node default worktree root", () => {
    const nodes = [
      { id: "node-east", default_worktree_root: "/srv/node-east-worktrees" },
      { id: "node-west", default_worktree_root: null },
    ];
    expect(
      resolveDefaultWorktreeRootForTargetNode("node-east", nodes, "/srv/local-worktrees", DEFAULT_ROOT)
    ).toBe("/srv/node-east-worktrees");
    expect(
      resolveDefaultWorktreeRootForTargetNode("node-west", nodes, "/srv/local-worktrees", DEFAULT_ROOT)
    ).toBe("/srv/local-worktrees");
    expect(
      resolveDefaultWorktreeRootForTargetNode("main", nodes, "/srv/local-worktrees", DEFAULT_ROOT)
    ).toBe("/srv/local-worktrees");
  });

  it("replaces default-like workdir when target node default root changes", () => {
    expect(
      resolveWorkdirForTargetNodeChange(
        "/srv/local-worktrees",
        "create_worktree",
        "/srv/local-worktrees",
        "/srv/node-east-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("/srv/node-east-worktrees");
    expect(
      resolveWorkdirForTargetNodeChange(
        "/tmp/custom",
        "create_worktree",
        "/srv/local-worktrees",
        "/srv/node-east-worktrees",
        DEFAULT_ROOT
      )
    ).toBe("/tmp/custom");
  });

  it("clears default-like workdir when leaving create_worktree target defaults", () => {
    expect(
      resolveWorkdirForTargetNodeChange(
        "/srv/node-east-worktrees",
        "use_existing",
        "/srv/node-east-worktrees",
        "/srv/local-worktrees",
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
    expect(resolveWorkdirForModalOpen(undefined, "use_existing", undefined, DEFAULT_ROOT)).toBe("");
  });
});
