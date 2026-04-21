// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { useTeamCatalogViewModel } from "./use_team_catalog_view_model";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamCatalogViewModel>[0];
type HookSnapshot = ReturnType<typeof useTeamCatalogViewModel>;

function HookHarness({
  params,
  onCapture,
}: {
  params: HookParams;
  onCapture: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamCatalogViewModel(params);
  useEffect(() => {
    onCapture(snapshot);
  }, [onCapture, snapshot]);
  return null;
}

async function mountHook(params: HookParams) {
  let snapshot: HookSnapshot | null = null;
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(
      <HookHarness
        params={params}
        onCapture={(next) => {
          snapshot = next;
        }}
      />
    );
    await Promise.resolve();
  });
  return {
    getSnapshot: () => snapshot,
    cleanup: () => {
      act(() => {
        root.unmount();
      });
      container.remove();
    },
  };
}

describe("useTeamCatalogViewModel", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("builds selector summaries and selected-team runtime state from one catalog pass", async () => {
    const params: HookParams = {
      teams: [
        {
          id: "team-1",
          name: "Alpha Team",
          description: "Mission alpha",
          spec: {
            leader_member_id: "leader-1",
            members: [
              { member_id: "leader-1", role: "leader" },
              { member_id: "worker-1", role: "worker" },
            ],
            steps: [],
          },
          created_at: 1,
          updated_at: 1,
        },
      ] as HookParams["teams"],
      agents: [
        {
          id: "leader-1",
          name: "Leader One",
          workdir: "/repo",
          command: "codex",
          args: [],
          worktree_mode: "use_existing",
          worktree_repo: null,
          worktree_ref: null,
          code_mode: true,
          agent_loop_enabled: false,
          agent_loop_idle_seconds: 0,
          agent_loop_prompt: "",
          status: "running",
          created_at: 1,
          updated_at: 1,
        },
      ] as HookParams["agents"],
      teamMemberAgentsById: {
        "worker-1": {
          id: "worker-1",
          name: "Worker One",
          workdir: "/repo",
          command: "codex",
          args: [],
          worktree_mode: "create_worktree",
          worktree_repo: null,
          worktree_ref: null,
          code_mode: true,
          agent_loop_enabled: false,
          agent_loop_idle_seconds: 0,
          agent_loop_prompt: "",
          status: "stopped",
          created_at: 1,
          updated_at: 1,
        },
      },
      teamRuntimeByTeamId: {
        "team-1": {
          team_id: "team-1",
          team_name: "Alpha Team",
          status: "running",
          members: [],
        },
      },
      selectedTeam: {
        id: "team-1",
        name: "Alpha Team",
        description: "Mission alpha",
        spec: {
          leader_member_id: "leader-1",
          members: [
            { member_id: "leader-1", role: "leader" },
            { member_id: "worker-1", role: "worker" },
          ],
          steps: [],
        },
        created_at: 1,
        updated_at: 1,
      } as HookParams["selectedTeam"],
      snapshot: null,
      teamSelectorFilter: "alpha",
    };

    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.teamSpecMemberIds).toEqual(["leader-1", "worker-1"]);
      expect(snapshot?.selectorTeamItems).toEqual([
        expect.objectContaining({
          id: "team-1",
          name: "Alpha Team",
          summary: "2 members · 1 active · 1 idle",
          runtimeLabel: "team running",
        }),
      ]);
      expect(snapshot?.selectedTeamHasConfiguredMembers).toBe(true);
      expect(snapshot?.selectedTeamHasLeader).toBe(true);
      expect(snapshot?.selectedTeamWorkerCount).toBe(1);
      expect(snapshot?.selectedTeamRuntimeStatus.label).toBe("team running");
      expect(snapshot?.selectedTeamRuntimeControlTone).toEqual({
        statusColor: "teal",
        countColor: "teal",
      });
    } finally {
      mounted.cleanup();
    }
  });

  it("falls back cleanly when there is no matching selected-team snapshot or configured members", async () => {
    const params: HookParams = {
      teams: [
        {
          id: "team-empty",
          name: "Empty Team",
          description: "   ",
          spec: {},
          created_at: 1,
          updated_at: 1,
        },
      ] as HookParams["teams"],
      agents: [],
      teamMemberAgentsById: {},
      teamRuntimeByTeamId: {},
      selectedTeam: {
        id: "team-empty",
        name: "Empty Team",
        description: "   ",
        spec: {},
        created_at: 1,
        updated_at: 1,
      } as HookParams["selectedTeam"],
      snapshot: {
        team: {
          id: "team-other",
          name: "Other Team",
        },
        leader_member_id: "leader-1",
        members: [
          {
            member_id: "leader-1",
            role: "leader",
            model: null,
            prompt: null,
            skills: [],
            pending_inbox_count: 0,
            status: "idle",
            latest_step: null,
            session_status: "inactive",
          },
        ],
        inbox: [],
        steps: [],
        tasks: [],
        generated_at: 1,
      } as HookParams["snapshot"],
      teamSelectorFilter: "team-empty",
    };

    const mounted = await mountHook(params);
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.selectorTeamItems).toEqual([
        expect.objectContaining({
          id: "team-empty",
          description: "No mission summary yet.",
          summary: "0 members · 0 active",
          runtimeLabel: "team stopped",
        }),
      ]);
      expect(snapshot?.selectedTeamSnapshotMembers).toBeUndefined();
      expect(snapshot?.selectedTeamMemberLiveStates).toEqual([]);
      expect(snapshot?.selectedTeamHasConfiguredMembers).toBe(false);
      expect(snapshot?.selectedTeamHasLeader).toBe(false);
      expect(snapshot?.selectedTeamWorkerCount).toBe(0);
      expect(snapshot?.selectedTeamRuntimeStatus.status).toBe("stopped");
      expect(snapshot?.selectedTeamRuntimeControlTone).toEqual({
        statusColor: "gray",
        countColor: "gray",
      });
    } finally {
      mounted.cleanup();
    }
  });
});
