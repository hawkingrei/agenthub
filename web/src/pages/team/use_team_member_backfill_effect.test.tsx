// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type AgentRecord } from "../../api";
import { useTeamMemberBackfillEffect } from "./use_team_member_backfill_effect";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamMemberBackfillEffect>[0];

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
    agents: [makeAgent("listed-agent")],
    teamSpecMemberIds: ["listed-agent", "missing-a", "missing-b"],
    teamMemberAgentsById: {},
    setTeamMemberAgentsById: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamMemberBackfillEffect(params);
  return null;
}

describe("useTeamMemberBackfillEffect", () => {
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

  it("does nothing when all members are already resolved", async () => {
    const params = createParams({
      teamSpecMemberIds: ["listed-agent"],
      teamMemberAgentsById: { "listed-agent": makeAgent("listed-agent") },
    });
    const getAgentSpy = vi.spyOn(api, "getAgent");

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(getAgentSpy).not.toHaveBeenCalled();
    expect(params.setTeamMemberAgentsById).not.toHaveBeenCalled();
  });

  it("backfills missing members and stores null for fetch failures", async () => {
    const params = createParams();
    vi.spyOn(api, "getAgent").mockImplementation(async (_token, agentId) => {
      if (agentId === "missing-a") {
        return makeAgent("missing-a");
      }
      throw new Error("not-found");
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const setById = params.setTeamMemberAgentsById as ReturnType<typeof vi.fn>;
    expect(setById).toHaveBeenCalledTimes(1);
    const updater = setById.mock.calls[0]?.[0] as (
      prev: Record<string, AgentRecord | null>
    ) => Record<string, AgentRecord | null>;
    const next = updater({ existing: makeAgent("existing") });

    expect(next.existing?.id).toBe("existing");
    expect(next["missing-a"]?.id).toBe("missing-a");
    expect(next["missing-b"]).toBeNull();
  });
});
