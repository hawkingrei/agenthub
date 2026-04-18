import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TeamPageHeader } from "./team_page_header";

const connectionBadge = {
  label: "ONLINE · SSE CONNECTED",
  title: "connected",
  tone: "ok" as const,
};

const lensItems = [
  { value: "chat", label: "Chat", active: true },
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
          connectionBadge={connectionBadge}
          username="root"
          isRoot
          headerShellClassName="shell"
          headerIconButtonClassName="icon"
          headerMutedButtonClassName="muted"
          headerStatusClassName="status"
          lensItems={lensItems}
          onToggleSidebar={vi.fn()}
          onSelectLens={vi.fn()}
          onNavigateToSelector={vi.fn()}
          onNavigate={vi.fn()}
          onLogout={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Teams");
    expect(html).toContain("Choose a team");
    expect(html).toContain("ONLINE · SSE CONNECTED");
  });

  it("renders workbench controls outside the selector route", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamPageHeader
          isSelectorRoute={false}
          teamsSidebarCollapsed={false}
          teamPanelToggleLabel="Hide teams panel"
          connectionBadge={connectionBadge}
          username="root"
          isRoot
          headerShellClassName="shell"
          headerIconButtonClassName="icon"
          headerMutedButtonClassName="muted"
          headerStatusClassName="status"
          lensItems={lensItems}
          onToggleSidebar={vi.fn()}
          onSelectLens={vi.fn()}
          onNavigateToSelector={vi.fn()}
          onNavigate={vi.fn()}
          onLogout={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Hide teams panel");
    expect(html).toContain("Workspace");
    expect(html).toContain("Chat");
    expect(html).toContain("Tasks");
    expect(html).toContain("Teams");
    expect(html).toContain("Open workbench menu");
  });
});
