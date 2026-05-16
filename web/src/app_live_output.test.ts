import { describe, expect, it, vi } from "vitest";
import type { AgentRecord } from "./api";
import type { OutputLine } from "./output_cache";
import {
  analyzeLiveOutputBatch,
  collectAcpPermissionSignalAgentIds,
  normalizeSseOutputLines,
  routeLiveOutputBatch,
} from "./app_live_output";

function buildOutputLine(
  overrides: Partial<OutputLine> = {}
): OutputLine {
  return {
    agent_id: "agent-1",
    session_id: "session-1",
    event_id: 1,
    seq: "1",
    ts: 1,
    stream: "stdout",
    message: "hello",
    ...overrides,
  };
}

describe("normalizeSseOutputLines", () => {
  it("accepts a single output event payload", () => {
    expect(
      normalizeSseOutputLines({
        type: "output",
        payload: buildOutputLine(),
      })
    ).toEqual([buildOutputLine()]);
  });

  it("keeps only valid entries from batch payloads", () => {
    expect(
      normalizeSseOutputLines({
        type: "batch",
        payload: [
          buildOutputLine({ event_id: 1, seq: "1" }),
          { event_id: "bad", seq: "2" },
          buildOutputLine({ event_id: 3, seq: "3", stream: "acp" }),
        ],
      })
    ).toEqual([
      buildOutputLine({ event_id: 1, seq: "1" }),
      buildOutputLine({ event_id: 3, seq: "3", stream: "acp" }),
    ]);
  });

  it("rejects unexpected payload shapes", () => {
    expect(normalizeSseOutputLines({ type: "unknown", payload: [] })).toEqual([]);
    expect(normalizeSseOutputLines(null)).toEqual([]);
  });
});

describe("analyzeLiveOutputBatch", () => {
  it("routes active-session lines and derives next agent status from ACP run_status", () => {
    const lines = [
      buildOutputLine({
        event_id: 1,
        seq: "1",
        session_id: "session-1",
        stream: "stdout",
      }),
      buildOutputLine({
        event_id: 2,
        seq: "2",
        session_id: "session-1",
        stream: "acp",
        message: JSON.stringify({ type: "run_status", status: "running" }),
      }),
      buildOutputLine({
        event_id: 3,
        seq: "3",
        agent_id: "agent-2",
        session_id: "session-2",
        stream: "acp",
        message: JSON.stringify({ type: "run_status", status: "completed" }),
      }),
    ];

    const analyzed = analyzeLiveOutputBatch(lines, "agent-1", "session-1");

    expect(analyzed.activeLines.map((line) => line.event_id)).toEqual([1, 2]);
    expect(analyzed.activeAcpLines.map((line) => line.event_id)).toEqual([2]);
    expect(analyzed.nextStatuses).toEqual({
      "agent-1": "running",
      "agent-2": "stopped",
    });
  });

  it("keeps agents active while waiting for permission", () => {
    const analyzed = analyzeLiveOutputBatch(
      [
        buildOutputLine({
          agent_id: "agent-1",
          session_id: "session-1",
          stream: "acp",
          message: JSON.stringify({
            type: "run_status",
            status: "waiting_permission",
          }),
        }),
      ],
      "agent-1",
      "session-1"
    );

    expect(analyzed.nextStatuses).toEqual({ "agent-1": "running" });
  });

  it("keeps agents active while a prompt is marked stale", () => {
    const analyzed = analyzeLiveOutputBatch(
      [
        buildOutputLine({
          agent_id: "agent-1",
          session_id: "session-1",
          stream: "acp",
          message: JSON.stringify({
            type: "run_status",
            status: "stale_prompt",
          }),
        }),
      ],
      "agent-1",
      "session-1"
    );

    expect(analyzed.nextStatuses).toEqual({ "agent-1": "running" });
  });
});

describe("collectAcpPermissionSignalAgentIds", () => {
  it("collects unique agents from ACP permission lifecycle events", () => {
    expect(
      collectAcpPermissionSignalAgentIds([
        buildOutputLine({
          agent_id: "agent-b",
          stream: "acp",
          message: JSON.stringify({ type: "permission_request", permission_id: "p1" }),
        }),
        buildOutputLine({
          agent_id: "agent-a",
          stream: "acp",
          message: JSON.stringify({ type: "permission_response", permission_id: "p2" }),
        }),
        buildOutputLine({
          agent_id: "agent-b",
          stream: "acp",
          message: JSON.stringify({ type: "permission_timeout", permission_id: "p3" }),
        }),
        buildOutputLine({
          agent_id: "agent-c",
          stream: "stdout",
          message: JSON.stringify({ type: "permission_request", permission_id: "p4" }),
        }),
        buildOutputLine({
          agent_id: "agent-d",
          stream: "acp",
          message: JSON.stringify({ type: "run_status", status: "running" }),
        }),
      ])
    ).toEqual(["agent-a", "agent-b"]);
  });

  it("accepts dispatch-error signals and ignores malformed ACP payloads", () => {
    expect(
      collectAcpPermissionSignalAgentIds([
        buildOutputLine({
          agent_id: "agent-a",
          stream: "acp",
          message: JSON.stringify({
            type: "permission_review_dispatch_error",
            permission_id: "p1",
          }),
        }),
        buildOutputLine({
          agent_id: "agent-b",
          stream: "acp",
          message: "{not-json",
        }),
        buildOutputLine({
          agent_id: "agent-c",
          stream: "acp",
          message: JSON.stringify({ permission_id: "p2" }),
        }),
        buildOutputLine({
          agent_id: "agent-d",
          stream: "acp",
          message: "plain text",
        }),
      ])
    ).toEqual(["agent-a"]);
  });
});

describe("routeLiveOutputBatch", () => {
  it("updates agent statuses only when ACP run-status events are present", () => {
    const updateAgents = vi.fn(
      (
        updater: (prev: AgentRecord[]) => AgentRecord[]
      ) =>
        updater([
          {
            id: "agent-1",
            name: "agent-1",
            workdir: "/tmp/agent-1",
            command: "codex",
            args: [],
            target_node_id: null,
            worktree_mode: "use_existing",
            worktree_repo: null,
            worktree_ref: null,
            code_mode: true,
            agent_loop_enabled: undefined,
            agent_loop_idle_seconds: null,
            agent_loop_prompt: null,
            status: "idle",
            created_at: 1,
            updated_at: 1,
          },
        ])
    );

    const active = routeLiveOutputBatch({
      cursorRef: { current: {} },
      lines: [
        buildOutputLine({
          stream: "acp",
          message: JSON.stringify({ type: "run_status", status: "running" }),
        }),
      ],
      activeAgent: "agent-1",
      activeSessionId: "session-1",
      updateAgents,
      onOutputGroup: vi.fn(),
      onAcpGroup: vi.fn(),
    });

    expect(active.activeAcpLines).toHaveLength(1);
    expect(updateAgents).toHaveBeenCalledTimes(1);
  });
});
