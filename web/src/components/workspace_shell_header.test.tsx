import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MantineProvider } from "@mantine/core";
import { WorkspaceShellHeader } from "./workspace_shell_header";

function renderHeader(
  props: Partial<React.ComponentProps<typeof WorkspaceShellHeader>> = {}
) {
  return renderToStaticMarkup(
    <MantineProvider>
      <WorkspaceShellHeader
        activeSurface="workspace"
        title="Workspace"
        subtitle="Shell"
        sidebarToggleLabel="Hide sidebar"
        sidebarCollapsed={false}
        onToggleSidebar={() => {}}
        username="root"
        isRoot={true}
        headerShellClassName="header-shell"
        headerIconButtonClassName="icon-button"
        menuButtonClassName="menu-button"
        connectionBadge={{
          tone: "muted",
          label: "Online · SSE idle",
          title: "Network online. No active SSE stream target.",
        }}
        headerStatusClassName="header-status"
        lensItems={[
          { value: "channels", label: "Channels", active: true },
          { value: "tasks", label: "Tasks", active: false },
        ]}
        onSelectLens={() => {}}
        onNavigate={() => {}}
        onLogout={() => {}}
        {...props}
      />
    </MantineProvider>
  );
}

describe("WorkspaceShellHeader", () => {
  it("renders title, connection badge, lenses, and compact menu affordances", () => {
    const html = renderHeader();
    expect(html).toContain("Workspace");
    expect(html).toContain("Shell");
    expect(html).toContain("Online · SSE idle");
    expect(html).toContain("header-status");
    expect(html).toContain("Channels");
    expect(html).toContain("Tasks");
    expect(html).toContain("aria-label=\"Hide sidebar\"");
    expect(html).toContain("aria-label=\"Open workbench menu\"");
  });

  it("gives the lens bar a dedicated mobile row so tabs do not crowd the title lane", () => {
    const html = renderHeader();
    expect(html).toContain("max-md:flex-wrap");
    expect(html).toContain("max-md:order-3");
    expect(html).toContain("max-md:basis-full");
  });

  it("omits optional chrome branches when props are absent", () => {
    const html = renderHeader({
      subtitle: null,
      sidebarToggleLabel: null,
      onToggleSidebar: null,
      connectionBadge: null,
      headerStatusClassName: null,
      lensItems: [],
      onSelectLens: null,
    });
    expect(html).toContain("Workspace");
    expect(html).not.toContain("Online · SSE idle");
    expect(html).not.toContain("Channels");
    expect(html).not.toContain("Hide sidebar");
  });
});
