// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type AgentRecord } from "../../api";
import { useTeamCreateModalEffects } from "./use_team_create_modal_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamCreateModalEffects>[0];

function makeAgent(id: string): AgentRecord {
  return {
    id,
    name: id,
    workdir: `/tmp/${id}`,
    command: "codex",
    args: [],
    worktree_mode: "use_existing",
    code_mode: true,
    status: "idle",
    created_at: 1,
    updated_at: 1,
  };
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    defaultWorktreeRoot: "~/.agenthub/worktrees",
    showCreateTeamModal: false,
    coordinatorMemberId: "",
    teamForgeAgents: [],
    busy: null,
    setForgeDefaultWorktreeRoot: vi.fn(),
    setCoordinatorMemberId: vi.fn(),
    setShowCreateTeamModal: vi.fn(),
    setCreateTeamStage: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamCreateModalEffects(params);
  return null;
}

describe("useTeamCreateModalEffects", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.restoreAllMocks();
  });

  it("uses fallback root when token is empty", async () => {
    const params = createParams({ token: "" });
    const runtimeSpy = vi.spyOn(api, "getRuntimeDefaults");

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setForgeDefaultWorktreeRoot).toHaveBeenCalledWith(
      "~/.agenthub/worktrees"
    );
    expect(runtimeSpy).not.toHaveBeenCalled();
  });

  it("loads runtime defaults and normalizes worktree root", async () => {
    const params = createParams({ token: "token-2" });
    vi.spyOn(api, "getRuntimeDefaults").mockResolvedValue({
      default_worktree_root: "  ~/custom/root/  ",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setForgeDefaultWorktreeRoot).toHaveBeenCalledWith(
      "~/custom/root"
    );
  });

  it("sets coordinator fallback and handles escape close only when allowed", async () => {
    const params = createParams({
      showCreateTeamModal: true,
      coordinatorMemberId: "missing-coordinator",
      teamForgeAgents: [makeAgent("coordinator-1"), makeAgent("worker-1")],
      busy: null,
    });
    vi.spyOn(api, "getRuntimeDefaults").mockResolvedValue({
      default_worktree_root: "~/.agenthub/worktrees",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setCoordinatorMemberId).toHaveBeenCalledWith("coordinator-1");

    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(params.setShowCreateTeamModal).toHaveBeenCalledWith(false);
    expect(params.setCreateTeamStage).toHaveBeenCalledWith(0);
  });

  it("ignores escape close while create-team is busy", async () => {
    const params = createParams({
      showCreateTeamModal: true,
      coordinatorMemberId: "coordinator-1",
      teamForgeAgents: [makeAgent("coordinator-1")],
      busy: "create-team",
    });
    vi.spyOn(api, "getRuntimeDefaults").mockResolvedValue({
      default_worktree_root: "~/.agenthub/worktrees",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(params.setShowCreateTeamModal).not.toHaveBeenCalledWith(false);
    expect(params.setCreateTeamStage).not.toHaveBeenCalledWith(0);
  });
});
