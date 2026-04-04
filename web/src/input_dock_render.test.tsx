import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { InputDock } from "./components/input_dock";
import { INPUT_DOCK_HISTORY_MENU_CLASS } from "./ui/tailwind_classes";

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
    const actionsRowPos = html.indexOf('data-input-actions-row="true"');
    const editorRowPos = html.indexOf('data-input-editor-row="true"');
    expect(actionsRowPos).toBeGreaterThanOrEqual(0);
    expect(editorRowPos).toBeGreaterThanOrEqual(0);
    expect(actionsRowPos).toBeLessThan(editorRowPos);
    expect(html).toContain('aria-label="Input actions"');
  });

  it("renders a dedicated send button class for larger tap target styling", () => {
    const html = renderDock();
    expect(html).toContain("bg-notion-accent");
    expect(html).toContain('aria-label="Send input"');
  });

  it("keeps textarea and send button in the same editor row", () => {
    const html = renderDock();
    const editorRowStart = html.indexOf('data-input-editor-row="true"');
    expect(editorRowStart).toBeGreaterThanOrEqual(0);
    const textareaPos = html.indexOf("<textarea", editorRowStart);
    const sendPos = html.indexOf("bg-notion-accent", editorRowStart);
    expect(textareaPos).toBeGreaterThan(editorRowStart);
    expect(sendPos).toBeGreaterThan(textareaPos);
  });

  it("renders textarea with stable form attributes for browser accessibility checks", () => {
    const html = renderDock();
    expect(html).toContain('name="acp_input"');
    expect(html).toMatch(/<textarea[^>]*id="/);
  });

  it("renders history as an overlay menu instead of changing dock layout flow", () => {
    const html = renderDock({ historyCommands: ["git status"] });
    expect(html).toContain("input-history relative");
    expect(html).toContain('aria-haspopup="menu"');
    expect(html).toContain('aria-label="Show sent command history"');
    expect(INPUT_DOCK_HISTORY_MENU_CLASS).toContain("absolute");
    expect(INPUT_DOCK_HISTORY_MENU_CLASS).toContain("bottom-[calc(100%+0.5rem)]");
  });

  it("keeps jump-to-bottom control outside the editor grid flow", () => {
    const html = renderDock({ showConversationJump: true });
    const shellPos = html.indexOf('class="input-dock-shell');
    const dockRootPos = html.indexOf('class="input docked');
    const jumpPos = html.indexOf('class="acp-jump-bottom');
    const editorRowPos = html.indexOf('data-input-editor-row="true"');
    const textareaPos = html.indexOf("<textarea", editorRowPos);
    expect(shellPos).toBeGreaterThanOrEqual(0);
    expect(dockRootPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeGreaterThanOrEqual(0);
    expect(editorRowPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeLessThan(dockRootPos);
    expect(jumpPos).toBeLessThan(editorRowPos);
    expect(textareaPos).toBeGreaterThan(editorRowPos);
  });

  it("stretches the dock to the panel edges instead of using a fixed centered width", () => {
    expect(renderDock()).toContain("input-dock-shell flex self-stretch");
    expect(renderDock()).toContain("mx-4");
    expect(renderDock()).toContain("w-full");
    expect(renderDock()).not.toContain("left-1/2");
    expect(renderDock()).not.toContain("-translate-x-1/2");
  });
});
