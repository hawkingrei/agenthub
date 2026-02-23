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

  it("renders dedicated actions and editor rows to avoid overlap", () => {
    const html = renderDock({
      showInterrupt: true,
      canInterrupt: true,
      historyCommands: ["git status"],
    });
    const actionsRowPos = html.indexOf('class="input-row"');
    const editorRowPos = html.indexOf('class="input-editor-row"');
    expect(actionsRowPos).toBeGreaterThanOrEqual(0);
    expect(editorRowPos).toBeGreaterThanOrEqual(0);
    expect(actionsRowPos).toBeLessThan(editorRowPos);
    expect(html).toContain('aria-label="Input actions"');
  });

  it("renders a dedicated send button class for larger tap target styling", () => {
    const html = renderDock();
    expect(html).toContain('class="input-send-button"');
    expect(html).toContain('aria-label="Send input"');
  });

  it("keeps textarea and send button in the same editor row", () => {
    const html = renderDock();
    const editorRowStart = html.indexOf('class="input-editor-row"');
    expect(editorRowStart).toBeGreaterThanOrEqual(0);
    const textareaPos = html.indexOf("<textarea", editorRowStart);
    const sendPos = html.indexOf('class="input-send-button"', editorRowStart);
    expect(textareaPos).toBeGreaterThan(editorRowStart);
    expect(sendPos).toBeGreaterThan(textareaPos);
  });

  it("renders jump-to-bottom in a dedicated row above the editor", () => {
    const html = renderDock({ showConversationJump: true });
    const jumpRowPos = html.indexOf('class="input-jump-row"');
    const jumpPos = html.indexOf('class="jump-bottom"');
    const editorRowPos = html.indexOf('class="input-editor-row"');
    const textareaPos = html.indexOf("<textarea", editorRowPos);
    expect(jumpRowPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeGreaterThanOrEqual(0);
    expect(editorRowPos).toBeGreaterThanOrEqual(0);
    expect(jumpRowPos).toBeLessThan(editorRowPos);
    expect(jumpPos).toBeGreaterThan(jumpRowPos);
    expect(textareaPos).toBeGreaterThan(editorRowPos);
  });
});
