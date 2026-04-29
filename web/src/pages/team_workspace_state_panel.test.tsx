import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { TeamLoadingPanel, TeamUnavailablePanel } from "./team_workspace_state_panel";

describe("team_workspace_state_panel", () => {
  it("renders the shared loading fallback for team bootstrap state", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamLoadingPanel />
      </MantineProvider>
    );

    expect(html).toContain("Loading team workspace...");
    expect(html).toContain("data-workspace-panel-loading=\"true\"");
    expect(html).toContain("AgentHub is loading the workspace context and team metadata.");
  });

  it("renders the shared unavailable placeholder and back action", () => {
    const onBackToSelector = vi.fn();
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamUnavailablePanel onBackToSelector={onBackToSelector} />
      </MantineProvider>
    );

    expect(html).toContain("Teams");
    expect(html).toContain("This team is unavailable");
    expect(html).toContain("Return to the team list and choose another one.");
    expect(html).toContain("Back to teams");
    expect(html).toContain("data-workspace-lens-placeholder=\"teams\"");
  });
});
