import { describe, expect, it } from "vitest";
import {
  appendOutputLine,
  buildAcpCacheSlice,
  buildOutputCacheSlice,
  mergeOutputsPreserveHistory,
  replaceAcpCacheSlice,
  selectCachedOutputs,
} from "./output_cache";
import { AgentEvent } from "./api";

const makeEvent = (event_id: number, stream: AgentEvent["stream"]): AgentEvent => ({
  agent_id: "agent-1",
  session_id: "session-1",
  ts: event_id,
  event_id,
  seq: String(event_id),
  stream,
  message: `${stream}-${event_id}`,
});

describe("buildAcpCacheSlice", () => {
  it("keeps ACP events when non-ACP output floods arrive", () => {
    const existing = [makeEvent(1, "acp"), makeEvent(2, "acp")];
    const ordered = [
      makeEvent(50, "acp"),
      ...Array.from({ length: 80 }, (_, idx) =>
        makeEvent(51 + idx, "stdout")
      ),
    ];
    const next = buildAcpCacheSlice(existing, ordered, 3);
    expect(next.map((evt) => evt.event_id)).toEqual([1, 2, 50]);
    expect(next.every((evt) => evt.stream === "acp")).toBe(true);
  });
});

describe("replaceAcpCacheSlice", () => {
  it("returns only ACP events and drops non-ACP events", () => {
    const ordered = [
      makeEvent(1, "stdout"),
      makeEvent(2, "acp"),
      makeEvent(3, "stderr"),
      makeEvent(4, "acp"),
    ];
    const next = replaceAcpCacheSlice(ordered, 10);
    expect(next.map((evt) => evt.event_id)).toEqual([2, 4]);
    expect(next.every((evt) => evt.stream === "acp")).toBe(true);
  });

  it("returns empty list when there are no ACP events", () => {
    const ordered = [makeEvent(1, "stdout"), makeEvent(2, "stderr")];
    const next = replaceAcpCacheSlice(ordered, 10);
    expect(next).toEqual([]);
  });

  it("trims ACP list to maxCachedEvents", () => {
    const ordered = [
      makeEvent(1, "acp"),
      makeEvent(2, "acp"),
      makeEvent(3, "acp"),
    ];
    const next = replaceAcpCacheSlice(ordered, 2);
    expect(next.map((evt) => evt.event_id)).toEqual([2, 3]);
  });
});

describe("buildOutputCacheSlice", () => {
  it("merges and trims to maxCachedEvents", () => {
    const existing = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const ordered = [makeEvent(3, "stdout"), makeEvent(4, "stdout")];
    const next = buildOutputCacheSlice(existing, ordered, 3);
    expect(next.map((evt) => evt.event_id)).toEqual([2, 3, 4]);
  });

  it("returns full merge when maxCachedEvents is non-positive", () => {
    const existing = [makeEvent(1, "stdout")];
    const ordered = [makeEvent(2, "stdout")];
    const next = buildOutputCacheSlice(existing, ordered, 0);
    expect(next.map((evt) => evt.event_id)).toEqual([1, 2]);
  });

  it("deduplicates by event_id when merging", () => {
    const existing = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const ordered = [makeEvent(2, "stdout"), makeEvent(3, "stdout")];
    const next = buildOutputCacheSlice(existing, ordered, 10);
    expect(next.map((evt) => evt.event_id)).toEqual([1, 2, 3]);
  });

  it("keeps latest payload when event_id already exists", () => {
    const existing = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const ordered = [{ ...makeEvent(2, "stdout"), message: "stdout-2-updated" }];
    const next = buildOutputCacheSlice(existing, ordered, 10);
    expect(next).not.toBe(existing);
    const updated = next.find((evt) => evt.event_id === 2);
    expect(updated?.message).toBe("stdout-2-updated");
  });
});

describe("appendOutputLine", () => {
  it("inserts by event_id ordering even if ts regresses", () => {
    const existing = [
      { ...makeEvent(2, "stdout"), ts: 200 },
    ];
    const incoming = { ...makeEvent(1, "stdout"), ts: 300 };
    const next = appendOutputLine(existing, incoming);
    expect(next.map((evt) => evt.event_id)).toEqual([1, 2]);
  });

  it("replaces existing line when event_id matches", () => {
    const existing = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const incoming = { ...makeEvent(2, "stdout"), message: "stdout-2-new" };
    const next = appendOutputLine(existing, incoming);
    expect(next).not.toBe(existing);
    expect(next.map((evt) => evt.event_id)).toEqual([1, 2]);
    expect(next.find((evt) => evt.event_id === 2)?.message).toBe("stdout-2-new");
  });

  it("returns the same array when incoming line is unchanged", () => {
    const existing = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const incoming = { ...makeEvent(2, "stdout") };
    const next = appendOutputLine(existing, incoming);
    expect(next).toBe(existing);
  });
});

describe("selectCachedOutputs", () => {
  it("prefers session cache when available", () => {
    const outputCache = {
      "agent-1:session-1": [makeEvent(1, "stdout")],
      "agent-1:latest": [makeEvent(2, "stdout")],
    };
    const acpCache = {
      "agent-1:session-1": [makeEvent(1, "acp")],
    };
    const selection = selectCachedOutputs(
      outputCache,
      acpCache,
      "agent-1:session-1",
      "agent-1:latest"
    );
    expect(selection.source).toBe("session");
    expect(selection.outputs?.[0].event_id).toBe(1);
    expect(selection.acpOutputs?.[0].event_id).toBe(1);
  });

  it("falls back to latest cache when session cache is missing", () => {
    const outputCache = {
      "agent-1:latest": [makeEvent(3, "stdout")],
    };
    const selection = selectCachedOutputs(
      outputCache,
      {},
      "agent-1:session-2",
      "agent-1:latest"
    );
    expect(selection.source).toBe("latest");
    expect(selection.outputs?.[0].event_id).toBe(3);
  });

  it("returns none when no cache exists", () => {
    const selection = selectCachedOutputs(
      {},
      {},
      "agent-1:session-3",
      "agent-1:latest"
    );
    expect(selection.source).toBe("none");
    expect(selection.outputs).toBeNull();
    expect(selection.acpOutputs).toBeNull();
  });
});

describe("mergeOutputsPreserveHistory", () => {
  it("keeps older outputs when cache is truncated for the same key", () => {
    const previous = [
      makeEvent(1, "stdout"),
      makeEvent(2, "stdout"),
      makeEvent(3, "stdout"),
      makeEvent(4, "stdout"),
    ];
    const cached = [makeEvent(3, "stdout"), makeEvent(4, "stdout")];
    const merged = mergeOutputsPreserveHistory(previous, cached, true);
    expect(merged.map((evt) => evt.event_id)).toEqual([1, 2, 3, 4]);
  });

  it("replaces outputs when the key changes", () => {
    const previous = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const cached = [makeEvent(3, "stdout")];
    const merged = mergeOutputsPreserveHistory(previous, cached, false);
    expect(merged.map((evt) => evt.event_id)).toEqual([3]);
  });

  it("keeps existing outputs when cache is empty for the same key", () => {
    const previous = [makeEvent(1, "stdout"), makeEvent(2, "stdout")];
    const merged = mergeOutputsPreserveHistory(previous, [], true);
    expect(merged.map((evt) => evt.event_id)).toEqual([1, 2]);
  });
});
