// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { InputDock } from "./input_dock";
import {
  INPUT_DOCK_ROOT_CLASS,
  INPUT_DOCK_SEND_BUTTON_CLASS,
  INPUT_DOCK_TEXTAREA_CLASS,
  TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS,
  TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS,
} from "../ui/tailwind_classes";

function expectStaticClassTokens(html: string, className: string): void {
  for (const token of className.split(/\s+/).filter(Boolean)) {
    expect(html).toContain(token.split("&").join("&amp;"));
  }
}

describe("InputDock", () => {
  it("shares the lightweight composer language used by Team channel and thread inputs", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <InputDock
          input="hello"
          historyCommands={[]}
          showInterrupt={false}
          canInterrupt={false}
          onInputChange={vi.fn()}
          onSendInput={vi.fn()}
          onInterrupt={vi.fn()}
          onNavigateHistory={vi.fn()}
          onSelectHistoryCommand={vi.fn()}
          onJumpToBottom={vi.fn()}
          showConversationJump={false}
          isComposingRef={{ current: false }}
        />
      </MantineProvider>
    );

    expect(html).toContain('data-acp-input-dock="true"');
    expect(html).toContain('data-input-editor-row="true"');
    expect(html).toContain('data-input-actions-row="true"');
    expect(html).toContain('name="acp_input"');
    expect(html).toContain("Enter to send");
    expect(html).toContain('aria-label="Send input"');
    expectStaticClassTokens(html, INPUT_DOCK_ROOT_CLASS);
    expectStaticClassTokens(html, TEAM_MESSAGE_COMPOSER_EDITOR_ROW_CLASS);
    expectStaticClassTokens(html, TEAM_MESSAGE_COMPOSER_ACTIONS_ROW_CLASS);
    expectStaticClassTokens(html, TEAM_MESSAGE_COMPOSER_HELPER_TEXT_CLASS);
    expectStaticClassTokens(html, INPUT_DOCK_TEXTAREA_CLASS);
    expectStaticClassTokens(html, INPUT_DOCK_SEND_BUTTON_CLASS);
  });
});
