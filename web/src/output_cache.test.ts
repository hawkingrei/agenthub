import { describe, expect, it } from "vitest";
import {
  buildAcpCacheSlice,
  buildOutputCacheSlice,
  selectCachedOutputs,
} from "./output_cache";
import { AgentEvent } from "./api";

const makeEvent = (seq: string, stream: AgentEvent["stream"]): AgentEvent => ({
  agent_id: "agent-1",
  session_id: "session-1",
  ts: Number(seq),
  seq,
  stream,
  message: `${stream}-${seq}`,
});

describe("buildAcpCacheSlice", () => {
  it("keeps ACP events when non-ACP output floods arrive", () => {
    const existing = [makeEvent("1", "acp"), makeEvent("2", "acp")];
    const ordered = [
      makeEvent("50", "acp"),
      ...Array.from({ length: 80 }, (_, idx) =>
        makeEvent(String(51 + idx), "stdout")
      ),
    ];
    const next = buildAcpCacheSlice(existing, ordered, 3);
    expect(next.map((evt) => evt.seq)).toEqual(["1", "2", "50"]);
    expect(next.every((evt) => evt.stream === "acp")).toBe(true);
  });
});

describe("buildOutputCacheSlice", () => {
  it("merges and trims to maxCachedEvents", () => {
    const existing = [makeEvent("1", "stdout"), makeEvent("2", "stdout")];
    const ordered = [makeEvent("3", "stdout"), makeEvent("4", "stdout")];
    const next = buildOutputCacheSlice(existing, ordered, 3);
    expect(next.map((evt) => evt.seq)).toEqual(["2", "3", "4"]);
  });

  it("returns full merge when maxCachedEvents is non-positive", () => {
    const existing = [makeEvent("1", "stdout")];
    const ordered = [makeEvent("2", "stdout")];
    const next = buildOutputCacheSlice(existing, ordered, 0);
    expect(next.map((evt) => evt.seq)).toEqual(["1", "2"]);
  });

  it("deduplicates by seq when merging", () => {
    const existing = [makeEvent("1", "stdout"), makeEvent("2", "stdout")];
    const ordered = [makeEvent("2", "stdout"), makeEvent("3", "stdout")];
    const next = buildOutputCacheSlice(existing, ordered, 10);
    expect(next.map((evt) => evt.seq)).toEqual(["1", "2", "3"]);
  });
});

describe("selectCachedOutputs", () => {
  it("prefers session cache when available", () => {
    const outputCache = {
      "agent-1:session-1": [makeEvent("1", "stdout")],
      "agent-1:latest": [makeEvent("2", "stdout")],
    };
    const acpCache = {
      "agent-1:session-1": [makeEvent("1", "acp")],
    };
    const selection = selectCachedOutputs(
      outputCache,
      acpCache,
      "agent-1:session-1",
      "agent-1:latest"
    );
    expect(selection.source).toBe("session");
    expect(selection.outputs?.[0].seq).toBe("1");
    expect(selection.acpOutputs?.[0].seq).toBe("1");
  });

  it("falls back to latest cache when session cache is missing", () => {
    const outputCache = {
      "agent-1:latest": [makeEvent("3", "stdout")],
    };
    const selection = selectCachedOutputs(
      outputCache,
      {},
      "agent-1:session-2",
      "agent-1:latest"
    );
    expect(selection.source).toBe("latest");
    expect(selection.outputs?.[0].seq).toBe("3");
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
