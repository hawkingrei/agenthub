// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act, type ComponentProps } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type AgentEvent } from "../../api";
import { AcpConversation, type AcpConversationProps } from "../../components/acp_conversation";
import { LoopActivationTranscript } from "./loop_activation_transcript";

vi.mock("../../components/acp_conversation", () => ({
  AcpConversation: vi.fn((props: AcpConversationProps) => (
    <div>{props.items.map(item => "text" in item ? item.text : "").join("")}</div>
  )),
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const scope = { token: "token", userId: "owner", teamId: "team", actorId: "worker" };
function event(id: number, text: string, payload: Record<string, unknown> = {}): AgentEvent {
  return {
    event_id: id, agent_id: "worker", session_id: "retired-session", seq: String(id), ts: id,
    stream: "acp",
    message: JSON.stringify({ type: "agent_message", text, message_id: `message-${id}`, ...payload }),
  };
}
function chunks(start: number, count: number): AgentEvent[] {
  return Array.from({ length: count }, (_, offset) => {
    const id = start + offset;
    return event(id, id === 0 ? "complete beginning " : id === 299 ? " complete end" : "x", {
      message_id: "chunked-response", chunk: true, chunk_index: id,
    });
  });
}

describe("retained activation transcript", () => {
  let root: Root;
  let container: HTMLDivElement;
  async function render(props: Partial<ComponentProps<typeof LoopActivationTranscript>> = {}) {
    await act(async () => root.render(
      <MantineProvider env="test">
        <LoopActivationTranscript scope={scope} activationId="retired-activation" sessionId="retired-session" {...props} />
      </MantineProvider>,
    ));
  }
  async function click(label: string) {
    const button = [...container.querySelectorAll("button")].find(item => item.textContent === label);
    expect(button).toBeDefined();
    await act(async () => button!.click());
  }
  beforeEach(() => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  it("reads the retained session without granting live conversation controls", async () => {
    const read = vi.spyOn(api, "listAgentEvents").mockResolvedValue([event(1, "Retained assistant answer")]);
    await render();
    expect(read).toHaveBeenCalledExactlyOnceWith("token", "worker", 100, "retired-session", null, expect.any(AbortSignal));
    expect(container.textContent).toContain("Retained assistant answer");
    const calls = vi.mocked(AcpConversation).mock.calls;
    const props = calls[calls.length - 1][0];
    expect(props.runStatus).toBe("exited");
    expect(props.isFrozenView).toBe(true);
    expect(props.onSubmitRequestUserInput).toBeUndefined();
    expect(container.querySelector("input,textarea")).toBeNull();
  });

  it("keeps a zero older cursor and refreshes from the newest page", async () => {
    const read = vi.spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce(Array.from({ length: 100 }, (_, id) => event(id, `Turn ${id}`)))
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([event(101, "Refreshed answer")]);
    await render();
    expect(read).toHaveBeenCalledTimes(1);
    await click("Load older transcript");
    expect(read).toHaveBeenLastCalledWith("token", "worker", 100, "retired-session", 0, expect.any(AbortSignal));
    expect(container.textContent).not.toContain("Load older transcript");
    await click("Refresh transcript");
    expect(read).toHaveBeenLastCalledWith("token", "worker", 100, "retired-session", null, expect.any(AbortSignal));
    expect(container.textContent).toContain("Refreshed answer");
    expect(container.textContent).not.toContain("Turn 99");
  });

  it("bounds partial-message recovery to one page and requires explicit further paging", async () => {
    const read = vi.spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce(chunks(200, 100))
      .mockResolvedValueOnce(chunks(20, 180))
      .mockResolvedValueOnce(chunks(0, 20));
    await render();
    expect(read).toHaveBeenCalledTimes(2);
    expect(read).toHaveBeenLastCalledWith("token", "worker", 180, "retired-session", 200, expect.any(AbortSignal));
    expect(container.textContent).toContain("An earlier response is incomplete");
    expect(container.textContent).not.toContain("complete end");
    await click("Load older transcript");
    expect(read).toHaveBeenCalledTimes(3);
    expect(container.textContent).toContain("complete beginning");
    expect(container.textContent).toContain("complete end");
    expect(container.textContent).not.toContain("An earlier response is incomplete");
  });

  it("renders a complete tail without treating its partial prefix as usable content", async () => {
    const read = vi.spyOn(api, "listAgentEvents").mockResolvedValue([
      ...chunks(1, 99), event(100, "Complete later answer"),
    ]);
    await render();
    expect(read).toHaveBeenCalledTimes(1);
    expect(container.textContent).toContain("Complete later answer");
    expect(container.textContent).toContain("An earlier response is incomplete");
  });

  it("aborts prior scope reads and drops late results after member or authentication changes", async () => {
    let resolveOld!: (events: AgentEvent[]) => void;
    const read = vi.spyOn(api, "listAgentEvents")
      .mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }))
      .mockResolvedValueOnce([{ ...event(2, "Current member answer"), agent_id: "other", session_id: "other-session" }]);
    await render();
    const oldSignal = read.mock.calls[0][5]!;
    await render({ scope: { ...scope, token: "new-token", userId: "other-user", teamId: "other-team", actorId: "other" }, sessionId: "other-session" });
    expect(oldSignal.aborted).toBe(true);
    await act(async () => resolveOld([event(1, "Stale private answer")]));
    expect(container.textContent).toContain("Current member answer");
    expect(container.textContent).not.toContain("Stale private answer");
  });

  it("reports denied or incorrectly scoped reads without claiming an empty conversation", async () => {
    const read = vi.spyOn(api, "listAgentEvents").mockRejectedValueOnce(new Error("Forbidden"));
    await render();
    expect(container.textContent).toContain("Forbidden");
    expect(container.textContent).not.toContain("No conversation recorded");
    read.mockResolvedValueOnce([{ ...event(1, "Other session content"), session_id: "wrong-session" }]);
    await click("Refresh transcript");
    expect(container.textContent).toContain("does not match the selected session");
    expect(container.textContent).not.toContain("Other session content");
  });
});
