// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ToolCallBubble } from "./acp_tool_call_bubble";
import { AcpConversationBubble } from "./acp_conversation_bubble";
import { api } from "../api";
import { installReactDomTestGlobals, renderWithMantine } from "../test_utils/react_test_helpers";

installReactDomTestGlobals();

describe("native input surfaces", () => {
  let container: HTMLDivElement;
  let root: Root;
  const target = { runtime_id: "runtime", session_id: "native", turn_id: "waiting" };
  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  async function question(meta?: unknown, submit = vi.fn().mockResolvedValue(undefined)) {
    await act(async () => renderWithMantine(root, <ToolCallBubble msg={{
      kind: "tool_call", id: "request-user-input:question", title: "Question", status: "pending",
      raw_input: [{ id: "choice", header: "Path", question: "Choose a path", options: [{ label: "Staging", description: "Use staging" }] }],
      meta,
    }} ansi={text => text} onSubmitRequestUserInput={submit} />));
    await act(async () => container.querySelector<HTMLInputElement>('input[type="radio"]')!.click());
    const button = [...container.querySelectorAll("button")].find(button => button.textContent?.includes("Submit Answer"))!;
    return { button, submit };
  }

  it("carries the question fence through the card and API payload", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({ status: "ok" }), { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    const submit = vi.fn(async (text, inputTarget) => {
      await api.sendInput("token", "agent", text, "message", "local", [], inputTarget);
    });
    const { button } = await question({ native_input: target }, submit);
    await act(async () => button.click());
    expect(submit).toHaveBeenCalledWith("Staging", target);
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toMatchObject({
      input: "Staging", message_id: "message", session_id: "local", native_input: target,
    });
  });

  it("keeps malformed native cards disabled", async () => {
    const { button, submit } = await question({ native_input: { ...target, turn_id: "" } });
    expect(button.disabled).toBe(true);
    await act(async () => button.click());
    expect(submit).not.toHaveBeenCalled();
  });

  it("preserves the legacy ACP callback and exposes failed native answers", async () => {
    const { button, submit } = await question();
    await act(async () => button.click());
    expect(submit).toHaveBeenCalledWith("Staging");
    const rejected = await question({ native_input: target }, vi.fn().mockRejectedValue(new Error("This question is no longer pending.")));
    await act(async () => rejected.button.click());
    expect(container.textContent).toContain("This question is no longer pending.");
  });

  it("updates the same live receipt status without duplicating the user message", async () => {
    let status: Element | null = null;
    for (const [delivery, label] of [["pending", "Sending"], ["accepted", "Accepted"], ["rejected", "Rejected"], ["outcome_unknown", "Delivery unknown — check the output before retrying"]]) {
      await act(async () => root.render(<AcpConversationBubble msg={{ kind: "user_message", text: "Check release", delivery }} globalIndex={0} latestVisibleGlobalIndex={0} shouldAutoCollapse={false} collapseCutoff={0} isFrozenView={false} ansi={text => text} markdownRenderVersion={0} />));
      const statuses = container.querySelectorAll('[role="status"]');
      expect(statuses).toHaveLength(1);
      expect(statuses[0].textContent).toBe(label);
      if (status) expect(statuses[0]).toBe(status);
      status = statuses[0];
      expect(container.querySelectorAll('[data-acp-message-bubble="user"]')).toHaveLength(1);
    }
  });

  it("does not add a delivery status to messages without a receipt", async () => {
    await act(async () => root.render(<AcpConversationBubble msg={{ kind: "user_message", text: "Check release" }} globalIndex={0} latestVisibleGlobalIndex={0} shouldAutoCollapse={false} collapseCutoff={0} isFrozenView={false} ansi={text => text} markdownRenderVersion={0} />));
    expect(container.querySelector('[role="status"]')).toBeNull();
    expect(container.textContent?.trim()).toBe("Check release");
  });
});
