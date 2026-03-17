import { describe, expect, it } from "vitest";
import type { TeamRunEventRecord, TeamRunRecord } from "../../api";
import {
  extractTaskIdFromRunInput,
  resolveActiveRunIdForSelectedTeam,
  resolveRunStatusFilter,
  selectRunsForTask,
  selectTeamPreviewEvents,
} from "./run_helpers";

function buildRun(id: string, teamId: string, createdAt: number): TeamRunRecord {
  return {
    id,
    team_id: teamId,
    context_id: `ctx-${id}`,
    status: "submitted",
    input: {},
    created_at: createdAt,
    started_at: null,
    ended_at: null,
  };
}

function buildEvent(eventId: number): TeamRunEventRecord {
  return {
    event_id: eventId,
    run_id: "run-1",
    step_id: null,
    event_type: "agent_message",
    ts: 1_700_000_000 + eventId,
    payload: { index: eventId },
  };
}

describe("team run helpers", () => {
  it("returns undefined for all status filter", () => {
    expect(resolveRunStatusFilter("all")).toBeUndefined();
    expect(resolveRunStatusFilter("working")).toBe("working");
  });

  it("chooses active run id scoped to selected team", () => {
    const runs = [
      buildRun("run-a", "team-a", 20),
      buildRun("run-b", "team-a", 10),
      buildRun("run-c", "team-b", 30),
    ];

    expect(resolveActiveRunIdForSelectedTeam(runs, null, "run-a")).toBeNull();
    expect(resolveActiveRunIdForSelectedTeam(runs, "team-a", "run-b")).toBe("run-b");
    expect(resolveActiveRunIdForSelectedTeam(runs, "team-a", "run-c")).toBe("run-a");
    expect(resolveActiveRunIdForSelectedTeam(runs, "team-z", "run-a")).toBeNull();
  });

  it("returns event preview only when member is not selected", () => {
    const events = [buildEvent(1), buildEvent(2), buildEvent(3), buildEvent(4)];

    expect(selectTeamPreviewEvents(events, "", 2).map((event) => event.event_id)).toEqual([
      3,
      4,
    ]);
    expect(
      selectTeamPreviewEvents(events, "agent-worker-1", 2).map((event) => event.event_id)
    ).toEqual([1, 2, 3, 4]);
  });

  it("extracts task ids from run input and filters related runs", () => {
    const runs = [
      buildRun("run-a", "team-a", 20),
      { ...buildRun("run-b", "team-a", 30), input: { task_id: "task-1" } },
      { ...buildRun("run-c", "team-a", 40), input: { task_id: "task-2" } },
      { ...buildRun("run-d", "team-a", 50), input: { task_id: "task-1" } },
    ];

    expect(extractTaskIdFromRunInput({ task_id: "task-7" })).toBe("task-7");
    expect(extractTaskIdFromRunInput({})).toBeNull();
    expect(selectRunsForTask(runs, "task-1").map((run) => run.id)).toEqual([
      "run-d",
      "run-b",
    ]);
  });
});
