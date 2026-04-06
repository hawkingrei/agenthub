import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentsRouteShellView } from "./components/agents_route_shell";
import { AgentsPanelProps } from "./components/agents_panel";
import { OutputHeaderProps } from "./components/output_header";

const baseAgentsPanelProps: AgentsPanelProps = {
  agents: [],
  activeAgent: null,
  agentsCollapsed: false,
  compactRows: false,
  hasPendingPermissions: false,
  pendingPermissionCounts: {},
  startingAgentIds: {},
  onCollapse: () => {},
  onExpand: () => {},
  onCreateAgent: () => {},
  onSelectAgent: () => {},
  onToggleCodeMode: () => {},
  onStartAgent: () => {},
  onStopAgent: () => {},
  onDeleteAgent: () => {},
};

const baseOutputHeaderProps: OutputHeaderProps = {
  activeAgent: null,
  activeSessionId: null,
  developerMode: false,
  hasAcp: false,
  thinkingStartTs: null,
  runStatus: null,
  modelLabel: null,
};

function renderShell(
  override: Partial<React.ComponentProps<typeof AgentsRouteShellView>> = {}
): string {
  return renderToStaticMarkup(
    <MantineProvider>
      <AgentsRouteShellView
        agentsCollapsed={false}
        workspaceRef={React.createRef<HTMLElement>()}
        workspaceStyle={undefined}
        onAgentsSplitterPointerDown={() => {}}
        agentsPanelProps={baseAgentsPanelProps}
        outputHeaderProps={baseOutputHeaderProps}
        workbenchNode={<div>Workbench marker</div>}
        {...override}
      />
    </MantineProvider>
  );
}

describe("AgentsRouteShellView", () => {
  it("renders the splitter and injected workbench when expanded", () => {
    const html = renderShell();
    expect(html).toContain("Resize agents sidebar");
    expect(html).toContain("Workbench marker");
    expect(html).toContain("No agent selected");
  });

  it("omits the splitter when the agents panel is collapsed", () => {
    const html = renderShell({
      agentsCollapsed: true,
      agentsPanelProps: {
        ...baseAgentsPanelProps,
        agentsCollapsed: true,
      },
    });
    expect(html).not.toContain("Resize agents sidebar");
    expect(html).toContain("Workbench marker");
  });

  it("hides the ACP header on narrow layouts when ACP is active", () => {
    const html = renderShell({
      outputHeaderProps: {
        ...baseOutputHeaderProps,
        hasAcp: true,
      },
    });
    expect(html).toContain("max-[720px]:hidden shrink-0");
  });
});
