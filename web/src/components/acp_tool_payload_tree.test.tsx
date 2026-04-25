import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  renderStructuredPayloadValue,
  summarizePayloadValue,
  type StructuredPayloadRenderers,
} from "./acp_tool_payload_tree";

const TEST_RENDERERS: StructuredPayloadRenderers = {
  renderText: (text) => <div data-kind="text">{text}</div>,
  renderPlainText: (text) => <pre data-kind="plain">{text}</pre>,
};

function renderStructuredPayload(value: unknown): string {
  return renderToStaticMarkup(
    <div>{renderStructuredPayloadValue(value, 0, TEST_RENDERERS)}</div>
  );
}

describe("acp_tool_payload_tree", () => {
  it("summarizes numeric-key objects as array-like payloads", () => {
    expect(
      summarizePayloadValue({
        0: "alpha",
        1: "beta",
      })
    ).toBe("Array(2) · alpha, beta");
  });

  it("prefers aggregated output and hides debug-only payload fields", () => {
    const html = renderStructuredPayload({
      aggregated_output: "done",
      stdout: "",
      stderr: "",
      call_id: "call-123",
      cwd: "/tmp/demo",
      source: "tool",
      success: true,
      context: "- keep plain",
      extra: "value",
    });

    expect(html).toContain("<dt>aggregated_output</dt>");
    expect(html).toContain("<pre data-kind=\"plain\">done</pre>");
    expect(html).toContain("<dt>context</dt>");
    expect(html).toContain("<pre data-kind=\"plain\">- keep plain</pre>");
    expect(html).toContain("<dt>extra</dt>");
    expect(html).not.toContain("<dt>stdout</dt>");
    expect(html).not.toContain("<dt>stderr</dt>");
    expect(html).not.toContain("call-123");
    expect(html).not.toContain("/tmp/demo");
    expect(html).not.toContain(">tool<");
  });
});
