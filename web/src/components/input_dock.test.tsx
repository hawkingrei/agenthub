// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  ACP_INPUT_MAX_IMAGE_BYTES,
  InputDock,
  validateInputImageFiles,
} from "./input_dock";
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

  it("renders standalone image attachments and attachment controls", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <InputDock
          input=""
          images={[
            {
              id: "image-1",
              file_name: "diagram.png",
              mime_type: "image/png",
              data: "aW1hZ2U=",
            },
          ]}
          enableImages={true}
          historyCommands={[]}
          showInterrupt={false}
          canInterrupt={false}
          onInputChange={vi.fn()}
          onImagesChange={vi.fn()}
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

    expect(html).toContain('data-acp-input-images="true"');
    expect(html).toContain('aria-label="Attach images"');
    expect(html).toContain('aria-label="Remove diagram.png"');
    expect(html).toContain("data:image/png;base64,aW1hZ2U=");
  });

  it("validates image type, count, and size before reading files", () => {
    expect(
      validateInputImageFiles([], [
        new File([new Uint8Array([1])], "diagram.png", { type: "image/png" }),
      ])
    ).toBeNull();
    expect(
      validateInputImageFiles([], [
        new File([new Uint8Array([1])], "diagram.svg", { type: "image/svg+xml" }),
      ])
    ).toContain("PNG");
    expect(
      validateInputImageFiles([], [
        new File([new Uint8Array(ACP_INPUT_MAX_IMAGE_BYTES + 1)], "large.png", {
          type: "image/png",
        }),
      ])
    ).toContain("5 MiB");
  });
});
