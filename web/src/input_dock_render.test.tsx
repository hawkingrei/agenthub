import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { InputDock } from "./components/input_dock";

const baseProps: React.ComponentProps<typeof InputDock> = {
  input: "",
  historyCommands: [],
  showInterrupt: true,
  canInterrupt: false,
  onInputChange: () => {},
  onSendInput: () => {},
  onInterrupt: () => {},
  onNavigateHistory: () => {},
  onSelectHistoryCommand: () => {},
  onJumpToBottom: () => {},
  showConversationJump: false,
  isComposingRef: { current: false },
};

function renderDock(override?: Partial<React.ComponentProps<typeof InputDock>>): string {
  return renderToStaticMarkup(<InputDock {...baseProps} {...override} />);
}

describe("InputDock interrupt placement", () => {
  it("renders interrupt button when enabled for ACP mode", () => {
    const html = renderDock({ showInterrupt: true, canInterrupt: true });
    expect(html).toContain("Interrupt");
    expect(/input-interrupt-button[^>]*disabled/.test(html)).toBe(false);
  });

  it("renders disabled interrupt button when run is not interruptible", () => {
    const html = renderDock({ showInterrupt: true, canInterrupt: false });
    expect(html).toContain("Interrupt");
    expect(/input-interrupt-button[^>]*disabled/.test(html)).toBe(true);
  });

  it("does not render interrupt button when ACP mode is absent", () => {
    const html = renderDock({ showInterrupt: false });
    expect(html).not.toContain("Interrupt");
  });
});
