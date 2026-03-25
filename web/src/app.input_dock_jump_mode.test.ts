import { describe, expect, it, vi } from "vitest";
import { resolveInputDockJumpMode } from "./components/acp_panel_helpers";

describe("resolveInputDockJumpMode", () => {
  it("routes ACP jump behavior through the dock when ACP view is active", () => {
    const onConversationJump = vi.fn();
    const onTerminalJump = vi.fn();
    const resolved = resolveInputDockJumpMode({
      hasAcp: true,
      showConversationJump: true,
      jumpToConversationBottom: onConversationJump,
      showTerminalJump: false,
      jumpToTerminalBottom: onTerminalJump,
    });
    expect(resolved.showConversationJump).toBe(true);
    resolved.onJumpToBottom();
    expect(onConversationJump).toHaveBeenCalledTimes(1);
    expect(onTerminalJump).not.toHaveBeenCalled();
  });

  it("uses terminal jump behavior when ACP view is inactive", () => {
    const onConversationJump = vi.fn();
    const onTerminalJump = vi.fn();
    const resolved = resolveInputDockJumpMode({
      hasAcp: false,
      showConversationJump: true,
      jumpToConversationBottom: onConversationJump,
      showTerminalJump: true,
      jumpToTerminalBottom: onTerminalJump,
    });
    expect(resolved.showConversationJump).toBe(true);
    resolved.onJumpToBottom();
    expect(onTerminalJump).toHaveBeenCalledTimes(1);
    expect(onConversationJump).not.toHaveBeenCalled();
  });
});
