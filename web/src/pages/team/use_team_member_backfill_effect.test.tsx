// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type AgentRecord } from "../../api";
import {
  resetTeamMemberBackfillCachesForTest,
  useTeamMemberBackfillEffect,
} from "./use_team_member_backfill_effect";

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

function makeApiError(status: number, message: string): Error & { status: number } {
  return Object.assign(new Error(message), { status });
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamMemberBackfillEffect(params);
  return null;
}

function DualHookHarness({ left, right }: { left: HookParams; right: HookParams }) {
  useTeamMemberBackfillEffect(left);
  useTeamMemberBackfillEffect(right);
  return null;
}

describe("useTeamMemberBackfillEffect", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    resetTeamMemberBackfillCachesForTest();
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

  it("backfills missing members and stores null for confirmed not found failures", async () => {
    const params = createParams();
    vi.spyOn(api, "getAgent").mockImplementation(async (_token, agentId) => {
      if (agentId === "missing-a") {
        return makeAgent("missing-a");
      }
      throw makeApiError(404, "not-found");
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

  it("revalidates cached hidden members and clears stale entries after deletion", async () => {
    const params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      teamMemberAgentsById: { "missing-a": makeAgent("missing-a") },
    });
    vi.spyOn(api, "getAgent").mockRejectedValue(makeApiError(404, "not-found"));

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
    const next = updater({ "missing-a": makeAgent("missing-a") });

    expect(next["missing-a"]).toBeNull();
  });

  it("preserves cached hidden members on transient revalidation failures", async () => {
    const params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      teamMemberAgentsById: { "missing-a": makeAgent("missing-a") },
    });
    vi.spyOn(api, "getAgent").mockRejectedValue(makeApiError(503, "unavailable"));

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(params.setTeamMemberAgentsById).not.toHaveBeenCalled();
  });

  it("treats newly added agent fields as cache differences during revalidation", async () => {
    const cachedAgent = {
      ...makeAgent("missing-a"),
      extra_marker: "old",
    } as AgentRecord;
    const refreshedAgent = {
      ...makeAgent("missing-a"),
      extra_marker: "new",
    } as AgentRecord;
    const params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      teamMemberAgentsById: { "missing-a": cachedAgent },
    });
    vi.spyOn(api, "getAgent").mockResolvedValue(refreshedAgent);

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
    const prev = { "missing-a": cachedAgent };
    const next = updater(prev);

    expect(next).not.toBe(prev);
    expect(next["missing-a"]).toEqual(refreshedAgent);
  });

  it("does not immediately revalidate cached hidden members on rerender", async () => {
    vi.useFakeTimers();
    const cachedAgent = makeAgent("missing-a");
    const getAgentSpy = vi.spyOn(api, "getAgent").mockResolvedValue(cachedAgent);

    let params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      teamMemberAgentsById: { "missing-a": cachedAgent },
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);

    params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      teamMemberAgentsById: { "missing-a": cachedAgent },
      setTeamMemberAgentsById: params.setTeamMemberAgentsById,
    });
    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);
  });

  it("coalesces in-flight backfill requests across rerenders before state catches up", async () => {
    let resolveMissingA: ((agent: AgentRecord) => void) | null = null;
    const getAgentSpy = vi.spyOn(api, "getAgent").mockImplementation(
      (_token, agentId) =>
        new Promise((resolve, reject) => {
          if (agentId === "missing-a") {
            resolveMissingA = resolve;
            return;
          }
          reject(makeApiError(404, "not-found"));
        })
    );

    const setById = vi.fn();
    let params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      setTeamMemberAgentsById: setById,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      setTeamMemberAgentsById: setById,
    });
    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveMissingA?.(makeAgent("missing-a"));
      await Promise.resolve();
      await Promise.resolve();
    });
  });

  it("does not immediately refetch after a shared backfill resolves before cache props catch up", async () => {
    const getAgentSpy = vi.spyOn(api, "getAgent").mockImplementation(async (_token, agentId) => {
      if (agentId === "missing-a") {
        return makeAgent("missing-a");
      }
      throw makeApiError(404, "not-found");
    });

    const setById = vi.fn();
    let params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      setTeamMemberAgentsById: setById,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);

    params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-a"],
      setTeamMemberAgentsById: setById,
    });
    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);
  });

  it("coalesces shared backfill requests across multiple hook instances", async () => {
    const getAgentSpy = vi.spyOn(api, "getAgent").mockImplementation(async (_token, agentId) => {
      if (agentId === "missing-a") {
        return makeAgent("missing-a");
      }
      throw makeApiError(404, "not-found");
    });

    const left = createParams({ teamSpecMemberIds: ["listed-agent", "missing-a"] });
    const right = createParams({ teamSpecMemberIds: ["listed-agent", "missing-a"] });

    act(() => {
      root.render(<DualHookHarness left={left} right={right} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);
  });

  it("retains shared cooldown entries when members temporarily leave and rejoin the team spec", async () => {
    const getAgentSpy = vi.spyOn(api, "getAgent").mockRejectedValue(
      makeApiError(404, "not-found")
    );

    let params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-b"],
      teamMemberAgentsById: { "missing-b": null },
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);

    params = createParams({
      teamSpecMemberIds: ["listed-agent"],
      teamMemberAgentsById: { "missing-b": null },
      setTeamMemberAgentsById: params.setTeamMemberAgentsById,
    });
    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);

    params = createParams({
      teamSpecMemberIds: ["listed-agent", "missing-b"],
      teamMemberAgentsById: { "missing-b": null },
      setTeamMemberAgentsById: params.setTeamMemberAgentsById,
    });
    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentSpy).toHaveBeenCalledTimes(1);
  });
});
