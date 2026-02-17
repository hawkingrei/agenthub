import { describe, expect, it } from "vitest";
import type { TeamRunEventRecord, TeamRunRecord } from "../api";
import {
  mergeRunPages,
  mergeTeamRunList,
  resolveRunStatusFilter,
  selectTeamPreviewEvents,
} from "./team_page";

function buildRun(
  id: string,
  createdAt: number,
  status: TeamRunRecord["status"] = "submitted"
): TeamRunRecord {
  return {
    id,
    team_id: "team-1",
    context_id: `ctx-${id}`,
    status,
    input: {},
    created_at: createdAt,
    started_at: null,
    ended_at: null,
  };
}

function buildRunEvent(eventId: number): TeamRunEventRecord {
  return {
    event_id: eventId,
    run_id: "run-1",
    step_id: null,
    event_type: "agent_message",
    ts: 1_700_000_000 + eventId,
    payload: { event_id: eventId },
  };
}

describe("team run list helpers", () => {
  it("maps run status filter to optional API status", () => {
    expect(resolveRunStatusFilter("all")).toBeUndefined();
    expect(resolveRunStatusFilter("working")).toBe("working");
  });

  it("merges paged runs with dedupe and latest payload preference", () => {
    const existing = [
      buildRun("run-1", 100, "submitted"),
      buildRun("run-2", 120, "working"),
    ];
    const incoming = [
      buildRun("run-2", 120, "completed"),
      buildRun("run-3", 110, "submitted"),
    ];
    const merged = mergeRunPages(existing, incoming);
    expect(merged.map((run) => run.id)).toEqual(["run-2", "run-3", "run-1"]);
    expect(merged.find((run) => run.id === "run-2")?.status).toBe("completed");
  });

  it("keeps active run on replace when it is outside current page window", () => {
    const previous = [
      buildRun("run-active", 50, "working"),
      buildRun("run-9", 90, "submitted"),
    ];
    const incoming = [buildRun("run-10", 110, "submitted"), buildRun("run-11", 105, "working")];
    const merged = mergeTeamRunList(previous, incoming, "replace", "run-active");
    expect(merged.map((run) => run.id)).toEqual(["run-10", "run-11", "run-active"]);
    expect(merged.some((run) => run.id === "run-active")).toBe(true);
  });

  it("shows only latest five run records before selecting a member", () => {
    const events = [1, 2, 3, 4, 5, 6, 7].map(buildRunEvent);
    const preview = selectTeamPreviewEvents(events, "");
    expect(preview.map((event) => event.event_id)).toEqual([3, 4, 5, 6, 7]);
  });

  it("shows full run records after selecting a specific member", () => {
    const events = [1, 2, 3, 4, 5, 6, 7].map(buildRunEvent);
    const fullList = selectTeamPreviewEvents(events, "agent-worker-1");
    expect(fullList.map((event) => event.event_id)).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });
});
