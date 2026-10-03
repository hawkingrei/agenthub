// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installReactDomTestGlobals } from "../../test_utils/react_test_helpers";
import { useTeamMemberAcpInput } from "./use_team_member_acp_input";

installReactDomTestGlobals();

type InputArgs = Parameters<typeof useTeamMemberAcpInput>[0];
type InputControls = ReturnType<typeof useTeamMemberAcpInput>;

describe("useTeamMemberAcpInput question submission", () => {
  let container: HTMLDivElement;
  let root: Root;
  const nativeTarget = { runtime_id: "runtime", session_id: "native", turn_id: "waiting" };

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  function renderInput(overrides: Partial<InputArgs>) {
    let controls: InputControls | undefined;
    function Harness() {
      controls = useTeamMemberAcpInput({
        selectedMemberId: "worker",
        selectedSessionId: "local",
        ...overrides,
      });
      return null;
    }
    act(() => root.render(<Harness />));
    return () => {
      if (!controls) throw new Error("Input controls were not rendered");
      return controls;
    };
  }

  it.each([
    { name: "native", target: nativeTarget },
    { name: "legacy", target: undefined },
  ])("rejects a dropped $name answer and permits an explicit retry", async ({ target }) => {
    let release!: () => void;
    const pending = new Promise<void>(resolve => { release = resolve; });
    const send = vi.fn<NonNullable<InputArgs["onSendInput"]>>()
      .mockResolvedValue(undefined)
      .mockImplementationOnce(() => pending);
    const controls = renderInput({ onSendInput: send });
    act(() => controls().handleInputChange("First input"));
    let first!: Promise<void>;
    act(() => { first = controls().handleSendInput(); });
    expect(controls().sendingInput).toBe(true);

    await expect(controls().handleSubmitRequestUserInput("Staging", target)).rejects.toThrow("not sent");
    await controls().handleSendInput();
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith("First input", "local");

    await act(async () => { release(); await first; });
    expect(controls().sendingInput).toBe(false);
    expect(send).toHaveBeenCalledTimes(1);
    await act(async () => controls().handleSubmitRequestUserInput("Staging", target));
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1]).toEqual(target ? ["Staging", "local", target] : ["Staging", "local"]);
  });

  it.each(["session", "callback", "text"])("rejects an answer when %s is unavailable", async missing => {
    const send = vi.fn().mockResolvedValue(undefined);
    const controls = renderInput({
      selectedSessionId: missing === "session" ? null : "local",
      onSendInput: missing === "callback" ? undefined : send,
    });
    await expect(controls().handleSubmitRequestUserInput(missing === "text" ? " " : "Staging", nativeTarget))
      .rejects.toThrow("not sent");
    expect(send).not.toHaveBeenCalled();
    expect(controls().sendingInput).toBe(false);
  });
});
