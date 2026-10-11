import { afterEach, describe, expect, it, vi } from "vitest";
import { EMPTY_ACP_VIEW } from "./acp";
import { api } from "./api";
import { canReviewNativeRecovery, hasNativeRuntime, validateRecoveryView, validRecoveryNote, type NativeRecoveryView } from "./native_recovery";

const view: NativeRecoveryView = {
  local_session_id: "local", runtime_id: "runtime", session_id: "native",
  recovery: { waiting_turn_id: null, blocked: { recovery_id: "token", turn_id: "old-turn", reason: "process_lost" }, decisions: [] },
};

afterEach(() => vi.unstubAllGlobals());

describe("native recovery boundaries", () => {
  it("offers operation controls only to the matching runtime roles", () => {
    for (const role of ["root", "admin", "operator"]) expect(canReviewNativeRecovery(role)).toBe(true);
    for (const role of [undefined, "", "viewer", "device", "Root", "unknown"]) expect(canReviewNativeRecovery(role)).toBe(false);
  });
  it("requires current query ownership and rejects ambiguous recovery data", () => {
    expect(() => validateRecoveryView(view, "local")).not.toThrow();
    for (const invalid of [
      null,
      { ...view, local_session_id: "previous" },
      { ...view, runtime_id: "" },
      { ...view, session_id: "foreign session" },
      { ...view, recovery: { ...view.recovery, waiting_turn_id: "waiting" } },
      { ...view, recovery: { ...view.recovery, blocked: { ...view.recovery.blocked, recovery_id: "token/path" } } },
      { ...view, recovery: { ...view.recovery, decisions: [{ state: "approved" }] } },
      { ...view, recovery: { ...view.recovery, decisions: Array.from({ length: 257 }, () => ({ state: "uncertain" })) } },
    ]) expect(() => validateRecoveryView(invalid, "local")).toThrow();
  });

  it("bounds notes by UTF-8 bytes and rejects empty or control-bearing content", () => {
    for (const note of ["", " ", "review\nnext", "review\u0085next", "x".repeat(4097), "€".repeat(1366)]) {
      expect(validRecoveryNote(note)).toBe(false);
    }
    expect(validRecoveryNote("Effects inspected")).toBe(true);
    expect(validRecoveryNote("€".repeat(1365))).toBe(true);
  });

  it("only exposes recovery controls for native runtime history", () => {
    expect(hasNativeRuntime(EMPTY_ACP_VIEW)).toBe(false);
    expect(hasNativeRuntime({ ...EMPTY_ACP_VIEW, rawEvents: [{ ts: 1, type: "agent_message", payload: {
      meta: { provider_runtime: { provider: "rara" } },
    } }] })).toBe(true);
  });

  it("encodes the current local session and exact reconciliation target without sending input", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify(view), { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    await api.getNativeRecovery("token", "actor/one", "local /2");
    expect(fetch.mock.calls[0][0]).toBe("/api/agents/actor%2Fone/runtime/recovery?local_session_id=local+%2F2");
    fetch.mockResolvedValue(new Response(JSON.stringify({ status: "ok" }), { status: 200 }));
    const target = { runtime_id: "runtime", session_id: "native", recovery_id: "token" };
    await api.reconcileNativeRecovery("token", "actor/one", "local /2", target, "Effects inspected");
    expect(fetch.mock.calls[1][0]).toBe("/api/agents/actor%2Fone/runtime/recovery");
    expect(JSON.parse(fetch.mock.calls[1][1].body)).toEqual({ local_session_id: "local /2", target, note: "Effects inspected" });
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
