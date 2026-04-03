// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AcpPermissionRecord } from "./api";
import { AcpDebug, AcpDebugProps } from "./components/acp_debug";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const permission: AcpPermissionRecord = {
  id: "perm-1",
  agent_id: "agent-a",
  session_id: "session-a",
  tool_call_id: "call-1",
  options: [{ option_id: "allow_once", name: "Allow once", kind: "allow_once" }],
  status: "pending",
  created_at: 1,
  tool_call: {
    title: "Read file",
    raw_input: { path: "README.md" },
  },
};

function buildProps(
  override: Partial<AcpDebugProps> = {}
): AcpDebugProps {
  return {
    terminalOutputs: [],
    ansi: (input) => input,
    terminalRef: React.createRef<HTMLDivElement>(),
    onTerminalScroll: () => {},
    showTerminalJump: false,
    onJumpToTerminalBottom: () => {},
    currentMode: "default",
    rawEvents: [],
    configOptions: [],
    acpPermissionHistory: [permission],
    acpModeId: "",
    acpModelId: "",
    acpConfigId: "",
    acpConfigValue: "",
    onAcpModeIdChange: () => {},
    onAcpModelIdChange: () => {},
    onAcpConfigIdChange: () => {},
    onAcpConfigValueChange: () => {},
    canControlAcp: true,
    onAcpSetMode: () => {},
    onAcpSetModel: () => {},
    onAcpSetConfig: () => {},
    onAcpCancel: () => {},
    onAcpClearSession: () => {},
    onJumpToPermissionHistory: () => {},
    runtimeMetrics: {
      totalConversationItems: 0,
      sourceConversationItems: 0,
      renderedConversationItems: 0,
      pendingConversationItems: 0,
      virtualizedConversation: false,
      stickToBottom: true,
      averageConversationHeight: 48,
      rawEventCount: 0,
      toolCallCount: 0,
      messageCount: 0,
      markdownCacheHits: 0,
      markdownCacheMisses: 0,
      ansiCacheHits: 0,
      ansiCacheMisses: 0,
      payloadParses: 0,
      payloadParseFailures: 0,
    },
    ...override,
  };
}

function clickByText(container: HTMLElement, text: string) {
  const button = Array.from(container.querySelectorAll("button")).find((el) =>
    el.textContent?.includes(text)
  );
  if (!button) throw new Error(`button not found: ${text}`);
  act(() => {
    button.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  });
}

describe("AcpDebug interactions", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("invokes terminal jump callback from terminal tab", () => {
    const onJumpTerminal = vi.fn();
    act(() => {
      root.render(
        <AcpDebug
          {...buildProps({
            initialTab: "terminal",
            showTerminalJump: true,
            onJumpToTerminalBottom: onJumpTerminal,
            terminalOutputs: [
              {
                event_id: 1,
                ts: 1,
                seq: "1",
                stream: "stdout",
                message: "line 1",
                agent_id: "agent-a",
                session_id: "session-a",
              },
            ],
          })}
        />
      );
    });

    clickByText(container, "Jump to latest");
    expect(onJumpTerminal).toHaveBeenCalledTimes(1);
  });

  it("filters terminal output down to stderr lines", () => {
    act(() => {
      root.render(
        <AcpDebug
          {...buildProps({
            initialTab: "terminal",
            terminalOutputs: [
              {
                event_id: 1,
                ts: 1,
                seq: "1",
                stream: "stdout",
                message: "stdout line",
                agent_id: "agent-a",
                session_id: "session-a",
              },
              {
                event_id: 2,
                ts: 2,
                seq: "2",
                stream: "stderr",
                message: "stderr line",
                agent_id: "agent-a",
                session_id: "session-a",
              },
            ],
          })}
        />
      );
    });

    clickByText(container, "Stderr");
    expect(container.textContent).toContain("stderr line");
    expect(container.textContent).not.toContain("stdout line");
  });

  it("invokes jump callback on permission row click", () => {
    const onJump = vi.fn();
    act(() => {
      root.render(<AcpDebug {...buildProps({ onJumpToPermissionHistory: onJump })} />);
    });
    clickByText(container, "Permissions");

    const row = container.querySelector(".acp-permission-toggle");
    act(() => {
      row?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(onJump).toHaveBeenCalledTimes(1);
    expect(onJump.mock.calls[0][0]).toMatchObject({ id: "perm-1" });
  });

  it("submits selected ACP mode and model values from config selectors", () => {
    const onSetMode = vi.fn();
    const onSetModel = vi.fn();
    function ControlledDebug() {
      const [acpModeId, setAcpModeId] = React.useState("");
      const [acpModelId, setAcpModelId] = React.useState("");
      return (
        <AcpDebug
          {...buildProps({
            configOptions: [
              {
                id: "mode",
                label: "Mode",
                currentValueId: "workspace_write",
                selectOptions: [
                  { valueId: "workspace_write", label: "Workspace Write" },
                  { valueId: "danger_full_access", label: "Full Access" },
                ],
              },
              {
                id: "model",
                label: "Model",
                currentValueId: "gpt-5",
                selectOptions: [
                  { valueId: "gpt-5", label: "GPT-5" },
                  { valueId: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
                ],
              },
            ],
            acpModeId,
            acpModelId,
            onAcpModeIdChange: setAcpModeId,
            onAcpModelIdChange: setAcpModelId,
            onAcpSetMode: onSetMode,
            onAcpSetModel: onSetModel,
          })}
        />
      );
    }
    act(() => {
      root.render(<ControlledDebug />);
    });

    const modeSelect = container.querySelector('select[name="acp-mode"]');
    const modelSelect = container.querySelector('select[name="acp-model"]');
    expect(modeSelect).not.toBeNull();
    expect(modelSelect).not.toBeNull();

    act(() => {
      (modeSelect as HTMLSelectElement).value = "danger_full_access";
      modeSelect?.dispatchEvent(new Event("change", { bubbles: true }));
      (modelSelect as HTMLSelectElement).value = "gemini-2.5-pro";
      modelSelect?.dispatchEvent(new Event("change", { bubbles: true }));
    });

    clickByText(container, "Set Mode");
    clickByText(container, "Set Model");

    expect(onSetMode).toHaveBeenCalledWith("danger_full_access");
    expect(onSetModel).toHaveBeenCalledWith("gemini-2.5-pro");
  });

  it("copies permission payload and resets copied state after timeout", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    act(() => {
      root.render(<AcpDebug {...buildProps()} />);
    });
    clickByText(container, "Permissions");

    const copyButton = container.querySelector(".acp-permission-copy");
    await act(async () => {
      copyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(writeText).toHaveBeenCalledTimes(1);
    const payload = JSON.parse(writeText.mock.calls[0][0] as string) as Record<
      string,
      unknown
    >;
    expect(payload.permission_id).toBe("perm-1");
    expect(container.querySelector(".acp-permission-copy")?.textContent).toContain(
      "Copied"
    );

    act(() => {
      vi.advanceTimersByTime(1600);
    });
    expect(container.querySelector(".acp-permission-copy")?.textContent).toContain(
      "Copy"
    );
  });

  it("falls back to document.execCommand when Clipboard API is unavailable", async () => {
    vi.useFakeTimers();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: undefined,
    });
    const execCommand = vi.fn().mockReturnValue(true);
    Object.defineProperty(document, "execCommand", {
      configurable: true,
      value: execCommand,
    });

    act(() => {
      root.render(<AcpDebug {...buildProps()} />);
    });
    clickByText(container, "Permissions");

    const copyButton = container.querySelector(".acp-permission-copy");
    await act(async () => {
      copyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(execCommand).toHaveBeenCalledWith("copy");
    expect(container.querySelector(".acp-permission-copy")?.textContent).toContain(
      "Copied"
    );
  });

  it("keeps copy button idle when clipboard write fails", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("clipboard denied"));
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    act(() => {
      root.render(<AcpDebug {...buildProps()} />);
    });
    clickByText(container, "Permissions");

    const copyButton = container.querySelector(".acp-permission-copy");
    await act(async () => {
      copyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(writeText).toHaveBeenCalledTimes(1);
    expect(container.querySelector(".acp-permission-copy")?.textContent).toContain(
      "Copy"
    );
  });

  it("clears previous copied-state reset timer when copying repeatedly", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const clearSpy = vi.spyOn(window, "clearTimeout");

    act(() => {
      root.render(<AcpDebug {...buildProps()} />);
    });
    clickByText(container, "Permissions");

    const copyButton = container.querySelector(".acp-permission-copy");
    await act(async () => {
      copyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    await act(async () => {
      copyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(clearSpy).toHaveBeenCalled();
    expect(writeText).toHaveBeenCalledTimes(2);
  });
});
