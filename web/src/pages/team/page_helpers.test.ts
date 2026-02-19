import { describe, expect, it } from "vitest";
import type { AgentEvent, AgentRecord, TeamRunEventRecord, TeamRunRecord } from "../../api";
import {
  buildAgentLabel,
  formatTs,
  pickNextWorkerAgentId,
  toPrettyJson,
  upsertAgentEventList,
  upsertEventList,
  upsertRun,
} from "./page_helpers";

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

function buildRunEvent(
  eventId: number,
  payload: unknown = {}
): TeamRunEventRecord {
  return {
    event_id: eventId,
    run_id: "run-1",
    step_id: null,
    event_type: "agent_message",
    ts: 1_700_000_000 + eventId,
    payload,
  };
}

function buildAgentEvent(eventId: number, message: string): AgentEvent {
  return {
    event_id: eventId,
    agent_id: "agent-1",
    session_id: "session-1",
    seq: String(eventId),
    ts: 1_700_000_000 + eventId,
    stream: "stdout",
    message,
  };
}

function buildAgent(overrides: Partial<AgentRecord> = {}): AgentRecord {
  return {
    id: "agent-1",
    name: "Leader Agent",
    workdir: "/tmp",
    command: "agenthub-codex-acp",
    args: [],
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: true,
    status: "running",
    created_at: 1_700_000_000,
    updated_at: 1_700_000_001,
    ...overrides,
  };
}

describe("team page helpers", () => {
  it("upserts run by id and keeps latest-first sort order", () => {
    const list = [buildRun("run-1", 100), buildRun("run-2", 120)];
    const updated = upsertRun(list, buildRun("run-1", 140, "working"));
    expect(updated.map((run) => run.id)).toEqual(["run-1", "run-2"]);
    expect(updated[0]?.status).toBe("working");
  });

  it("upserts team run events with dedupe and monotonic ordering", () => {
    const replace = upsertEventList(
      [buildRunEvent(1), buildRunEvent(2)],
      [buildRunEvent(2, { text: "updated" }), buildRunEvent(3)],
      "replace"
    );
    expect(replace.map((event) => event.event_id)).toEqual([2, 3]);
    expect(replace[0]?.payload).toEqual({ text: "updated" });

    const prepend = upsertEventList(
      [buildRunEvent(3), buildRunEvent(4)],
      [buildRunEvent(2), buildRunEvent(3, { text: "newer" })],
      "prepend"
    );
    expect(prepend.map((event) => event.event_id)).toEqual([2, 3, 4]);
    expect(prepend.find((event) => event.event_id === 3)?.payload).toEqual({});
  });

  it("upserts agent events with dedupe and monotonic ordering", () => {
    const merged = upsertAgentEventList(
      [buildAgentEvent(5, "old-5"), buildAgentEvent(7, "old-7")],
      [buildAgentEvent(6, "new-6"), buildAgentEvent(7, "new-7")],
      "prepend"
    );
    expect(merged.map((event) => event.event_id)).toEqual([5, 6, 7]);
    expect(merged.find((event) => event.event_id === 7)?.message).toBe("old-7");
  });

  it("builds readable agent labels with model metadata", () => {
    const modelFromArgs = buildAgentLabel(
      buildAgent({ args: ["--model", "gpt-5.1"], command: "gemini" })
    );
    expect(modelFromArgs).toContain("gpt-5.1");
    expect(modelFromArgs).toContain("Leader Agent");
    expect(modelFromArgs).toContain("agent-1");

    const fallback = buildAgentLabel(
      buildAgent({ command: "/usr/local/bin/custom-agent", args: [] })
    );
    expect(fallback).toContain("custom-agent");
  });

  it("picks next available worker agent id and handles exhaustion", () => {
    const agents = [buildAgent({ id: "a1" }), buildAgent({ id: "a2" })];
    expect(pickNextWorkerAgentId(agents, new Set(["a1"]))).toBe("a2");
    expect(pickNextWorkerAgentId(agents, new Set(["a1", "a2"]))).toBe("");
  });

  it("formats timestamps and pretty prints JSON safely", () => {
    expect(formatTs(null)).toBe("-");
    expect(formatTs(0)).toBe("-");
    expect(formatTs(1_700_000_000)).toBe(new Date(1_700_000_000 * 1000).toLocaleString());

    expect(toPrettyJson({ a: 1 })).toBe('{\n  "a": 1\n}');
    const circular: { self?: unknown } = {};
    circular.self = circular;
    expect(toPrettyJson(circular)).toBe("[object Object]");
  });
});
