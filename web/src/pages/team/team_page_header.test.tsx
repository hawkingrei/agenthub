import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TeamPageHeader } from "./team_page_header";

const lensItems = [
  { value: "channels", label: "Channels", active: true },
  { value: "tasks", label: "Tasks", active: false },
];

describe("TeamPageHeader", () => {
  it("renders selector header copy on the selector route", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamPageHeader
          isSelectorRoute
          teamsSidebarCollapsed={false}
          teamPanelToggleLabel="Hide teams panel"
          username="root"
          isRoot
          headerShellClassName="shell"
          headerIconButtonClassName="icon"
          lensItems={lensItems}
          onToggleSidebar={vi.fn()}
          onSelectLens={vi.fn()}
          onNavigate={vi.fn()}
          onLogout={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Teams");
    expect(html).not.toContain("ONLINE · SSE CONNECTED");
  });

  it("renders workbench controls outside the selector route", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamPageHeader
          isSelectorRoute={false}
          teamsSidebarCollapsed={false}
          teamPanelToggleLabel="Hide teams panel"
          username="root"
          isRoot
          headerShellClassName="shell"
          headerIconButtonClassName="icon"
          lensItems={lensItems}
          onToggleSidebar={vi.fn()}
          onSelectLens={vi.fn()}
          onNavigate={vi.fn()}
          onLogout={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Hide teams panel");
    expect(html).toContain("Workspace");
    expect(html).toContain("Channels");
    expect(html).toContain("Tasks");
    expect(html).not.toContain("ONLINE · SSE CONNECTED");
    expect(html).toContain("aria-label=\"Open workbench menu\"");
  });
});
