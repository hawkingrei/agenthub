import { describe, expect, it } from "vitest";
import type { TeamRunRecord } from "../api";
import { mergeRunPages, mergeTeamRunList, resolveRunStatusFilter } from "./team_page";

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
});
