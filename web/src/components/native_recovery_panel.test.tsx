// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../api";
import type { NativeRecoveryView } from "../native_recovery";
import { installReactDomTestGlobals, renderWithMantine } from "../test_utils/react_test_helpers";
import { NativeRecoveryPanel } from "./native_recovery_panel";

installReactDomTestGlobals();

const view: NativeRecoveryView = {
  local_session_id: "local", runtime_id: "runtime", session_id: "native",
  recovery: { waiting_turn_id: null, blocked: { recovery_id: "token", turn_id: "old", reason: "process_lost" }, decisions: [{ state: "uncertain" }] },
};

describe("native recovery panel", () => {
  let container: HTMLDivElement;
  let root: Root;
  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(api, "getNativeRecovery").mockResolvedValue(view);
    vi.spyOn(api, "reconcileNativeRecovery").mockResolvedValue({ status: "ok" });
    vi.spyOn(api, "sendInput").mockResolvedValue({ status: "ok" });
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });
  const render = async (localSessionId = "local", canOperate = true) => {
    await act(async () => renderWithMantine(root, <NativeRecoveryPanel token="token" agentId="actor"
      localSessionId={localSessionId} canOperate={canOperate} nativeRuntime />));
  };
  const button = (label: string) => [...container.querySelectorAll("button")].find(button => button.textContent === label)!;
  const click = async (label: string) => { await act(async () => button(label).click()); };
  const fill = async (note: string) => {
    await act(async () => {
      const input = container.querySelector("input")!;
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, note);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
  };

  it("requires an explicit note, preserves exact identity and never submits a prompt", async () => {
    await render();
    expect(api.getNativeRecovery).not.toHaveBeenCalled();
    await click("Review recovery");
    expect(button("Confirm recovery review").disabled).toBe(true);
    expect(container.textContent).toContain("1 earlier approval has an unconfirmed outcome");
    await fill("Old executor stopped; effects inspected");
    await act(async () => { button("Confirm recovery review").click(); button("Confirm recovery review").click(); });
    expect(api.reconcileNativeRecovery).toHaveBeenCalledExactlyOnceWith("token", "actor", "local", {
      runtime_id: "runtime", session_id: "native", recovery_id: "token",
    }, "Old executor stopped; effects inspected");
    expect(container.textContent).toContain("Recovery review saved. Send a new instruction when ready.");
    expect(api.sendInput).not.toHaveBeenCalled();
    expect(api.getNativeRecovery).toHaveBeenCalledTimes(1);
  });

  it("ignores a late query after switching local session", async () => {
    let complete!: (value: NativeRecoveryView) => void;
    vi.mocked(api.getNativeRecovery).mockReturnValueOnce(new Promise(resolve => { complete = resolve; }));
    await render();
    await click("Review recovery");
    await render("replacement");
    await act(async () => complete(view));
    expect(button("Review recovery")).toBeDefined();
    expect(container.querySelector("input")).toBeNull();
    expect(api.reconcileNativeRecovery).not.toHaveBeenCalled();
  });

  it("requires a fresh query after an uncertain resolution response", async () => {
    vi.mocked(api.reconcileNativeRecovery).mockRejectedValueOnce(new Error("connection closed"));
    await render();
    await click("Review recovery");
    await fill("Effects inspected");
    await click("Confirm recovery review");
    expect(container.textContent).toContain("Refresh status before trying again");
    expect(container.querySelector("input")).toBeNull();
    expect(api.reconcileNativeRecovery).toHaveBeenCalledTimes(1);
    await click("Refresh recovery status");
    expect(container.querySelector("input")!.value).toBe("");
    expect(button("Confirm recovery review").disabled).toBe(true);
    expect(api.reconcileNativeRecovery).toHaveBeenCalledTimes(1);
  });

  it("does not carry a saved result into a replacement session", async () => {
    let complete!: (value: { status: string }) => void;
    vi.mocked(api.reconcileNativeRecovery).mockReturnValueOnce(new Promise(resolve => { complete = resolve; }));
    await render();
    await click("Review recovery");
    await fill("Effects inspected");
    await click("Confirm recovery review");
    await render("replacement");
    await act(async () => complete({ status: "ok" }));
    expect(container.textContent).not.toContain("Recovery review saved");
    expect(button("Review recovery")).toBeDefined();
    expect(api.sendInput).not.toHaveBeenCalled();
  });

  it("hides controls without operation authority and rejects foreign query results", async () => {
    await render("local", false);
    expect(container.querySelector("button")).toBeNull();
    expect(api.getNativeRecovery).not.toHaveBeenCalled();
    await render("replacement");
    await click("Review recovery");
    expect(container.querySelector("[role=alert]")).not.toBeNull();
    expect(container.querySelector("input")).toBeNull();
    expect(api.reconcileNativeRecovery).not.toHaveBeenCalled();
  });

  it("keeps pending questions and clean sessions out of reconciliation", async () => {
    vi.mocked(api.getNativeRecovery)
      .mockResolvedValueOnce({ ...view, recovery: { ...view.recovery, blocked: null, waiting_turn_id: "question" } })
      .mockResolvedValueOnce({ ...view, recovery: { ...view.recovery, blocked: null } });
    await render();
    await click("Review recovery");
    expect(container.textContent).toContain("Answer the current question or approval in the conversation");
    expect(container.querySelector("input")).toBeNull();
    await click("Refresh recovery status");
    expect(container.textContent).toContain("No interrupted work needs reconciliation");
    expect(container.querySelector("input")).toBeNull();
    expect(api.reconcileNativeRecovery).not.toHaveBeenCalled();
    expect(api.sendInput).not.toHaveBeenCalled();
  });
});
