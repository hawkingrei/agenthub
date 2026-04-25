// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type TeamRunRecord } from "../../api";
import { useTeamRunLifecycleActions } from "./use_team_run_lifecycle_actions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamRunLifecycleActions>[0];
type HookSnapshot = ReturnType<typeof useTeamRunLifecycleActions>;

function makeRun(
  id: string,
  teamId: string,
  status: TeamRunRecord["status"] = "working",
  createdAt = 1
): TeamRunRecord {
  return {
    id,
    team_id: teamId,
    context_id: `ctx-${id}`,
    status,
    input: {},
    created_at: createdAt,
    started_at: null,
    ended_at: null,
  };
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    activeRunId: "run-1",
    runLookupId: "run-1",
    setBusy: vi.fn(),
    setError: vi.fn(),
    parseErrorMessage: vi.fn(() => "parsed-error"),
    refreshRun: vi.fn().mockResolvedValue(makeRun("run-1", "team-1")),
    refreshEvents: vi.fn().mockResolvedValue(undefined),
    refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    setRuns: vi.fn(),
    setSelectedTeamId: vi.fn(),
    setActiveRunId: vi.fn(),
    setRunLookupId: vi.fn(),
    ...overrides,
  };
}

function HookHarness({
  params,
  onSnapshot,
}: {
  params: HookParams;
  onSnapshot: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamRunLifecycleActions(params);
  onSnapshot(snapshot);
  return null;
}

describe("useTeamRunLifecycleActions", () => {
  let container: HTMLDivElement;
  let root: Root;
  let snapshot: HookSnapshot | null = null;

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
    snapshot = null;
    vi.restoreAllMocks();
  });

  it("validates run id before loading by id", async () => {
    const params = createParams({ runLookupId: "   " });
    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onLoadRunById();
    });

    expect(params.setError).toHaveBeenCalledWith("Run ID is required");
    expect(params.refreshRun).not.toHaveBeenCalled();
  });

  it("loads run by id and syncs selected team and active run", async () => {
    const params = createParams({
      runLookupId: " run-2 ",
      refreshRun: vi.fn().mockResolvedValue(makeRun("run-2", "team-9")),
    });
    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onLoadRunById();
    });

    expect(params.setBusy).toHaveBeenCalledWith("load-run");
    expect(params.setBusy).toHaveBeenCalledWith(null);
    expect(params.refreshRun).toHaveBeenCalledWith("run-2");
    expect(params.setSelectedTeamId).toHaveBeenCalledWith("team-9");
    expect(params.setActiveRunId).toHaveBeenCalledWith("run-2");
  });

  it("cancels active run and refreshes dependent views", async () => {
    const params = createParams({
      activeRunId: "run-1",
      refreshEvents: vi.fn().mockResolvedValue(undefined),
      refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    });
    vi.spyOn(api, "cancelTeamRun").mockResolvedValue(
      makeRun("run-1", "team-1", "canceled", 3)
    );

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onCancelRun();
    });

    expect(api.cancelTeamRun).toHaveBeenCalledWith("token-1", "run-1");
    expect(params.refreshEvents).toHaveBeenCalledWith("run-1");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");

    const setRuns = params.setRuns as ReturnType<typeof vi.fn>;
    const updater = setRuns.mock.calls[0]?.[0] as (
      prev: TeamRunRecord[]
    ) => TeamRunRecord[];
    const next = updater([
      makeRun("run-2", "team-1", "completed", 2),
      makeRun("run-1", "team-1", "working", 1),
    ]);
    const updated = next.find((run) => run.id === "run-1");
    expect(updated?.status).toBe("canceled");
  });

  it("resumes active run and updates active/lookup ids", async () => {
    const params = createParams({ activeRunId: "run-1" });
    vi.spyOn(api, "resumeTeamRun").mockResolvedValue(makeRun("run-2", "team-1"));

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onResumeRun();
    });

    expect(api.resumeTeamRun).toHaveBeenCalledWith("token-1", "run-1");
    expect(params.setActiveRunId).toHaveBeenCalledWith("run-2");
    expect(params.setRunLookupId).toHaveBeenCalledWith("run-2");
  });

  it("restarts active run and updates active/lookup ids", async () => {
    const params = createParams({ activeRunId: "run-1" });
    vi.spyOn(api, "restartTeamRun").mockResolvedValue(makeRun("run-3", "team-1"));

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onRestartRun();
    });

    expect(api.restartTeamRun).toHaveBeenCalledWith("token-1", "run-1");
    expect(params.setActiveRunId).toHaveBeenCalledWith("run-3");
    expect(params.setRunLookupId).toHaveBeenCalledWith("run-3");
  });
});
