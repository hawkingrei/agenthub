import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import { WorkspacePanelLoadingFallback } from "./workspace_panel_loading_fallback";

describe("WorkspacePanelLoadingFallback", () => {
  it("renders the shared shell loading chrome", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <WorkspacePanelLoadingFallback className="loading-shell-card" />
      </MantineProvider>
    );

    expect(html).toContain("Loading workspace panel...");
    expect(html).toContain("loading this workspace surface");
    expect(html).toContain("data-workspace-panel-loading=\"true\"");
    expect(html).toContain("loading-shell-card");
  });
});
