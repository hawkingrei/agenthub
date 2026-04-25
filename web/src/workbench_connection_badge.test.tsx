import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { deriveConnectionBadge } from "./connection_status";
import { WorkbenchConnectionBadge } from "./components/workbench_connection_badge";

describe("WorkbenchConnectionBadge", () => {
  it("renders shared online idle label", () => {
    const badge = deriveConnectionBadge(true, false, "idle");
    const html = renderToStaticMarkup(
      <WorkbenchConnectionBadge badge={badge} className="session-connection" />
    );
    expect(html).toContain("Online · SSE idle");
    expect(html).toContain('role="status"');
    expect(html).toContain('aria-live="polite"');
    expect(html).toContain("session-connection muted");
    expect(html).toContain("session-connection-dot");
  });

  it("renders shared offline label", () => {
    const badge = deriveConnectionBadge(false, false, "idle");
    const html = renderToStaticMarkup(
      <WorkbenchConnectionBadge badge={badge} className="session-connection" />
    );
    expect(html).toContain("Offline · SSE disconnected");
    expect(html).toContain("session-connection bad");
  });
});
