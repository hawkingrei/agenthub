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
    currentMode: "default",
    rawEvents: [],
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
});
