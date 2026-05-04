import React from "react";
import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { AuthState } from "../types";
import { AgentsRouteContainer } from "./agents_route_container";

vi.mock("../components/agents_root_page", () => ({
  AgentsRootPage: ({ rootWorkbenchNode }: { rootWorkbenchNode?: React.ReactNode }) => (
    <div data-testid="agents-root-page">{rootWorkbenchNode}</div>
  ),
}));

vi.mock("../components/agent_nodes_workbench", () => ({
  AgentNodesWorkbench: () => <div data-testid="agent-nodes-workbench">nodes workbench</div>,
}));

function renderRoute(activeWorkspaceLens: "search" | "channels" | "tasks" | "members" | "nodes", auth: AuthState | null) {
  return renderToStaticMarkup(
    <MantineProvider>
      <AgentsRouteContainer
        appRootRef={{ current: null }}
        appHeaderRef={{ current: null }}
        auth={auth}
        normalizedError={null}
        onClearError={vi.fn()}
        authBusy={null}
        rootInitialized={true}
        username=""
        password=""
        displayName=""
        setUsername={vi.fn()}
        setPassword={vi.fn()}
        setDisplayName={vi.fn()}
        onLogin={vi.fn(async () => {})}
        onRegister={vi.fn(async () => {})}
        agentsCollapsed={false}
        onCollapseAgents={vi.fn()}
        onExpandAgents={vi.fn()}
        connectionBadge={null as never}
        onLogout={vi.fn()}
        navigateWorkbenchRoute={vi.fn()}
        workspaceRef={{ current: null }}
        workspaceStyle={undefined}
        onAgentsSplitterPointerDown={vi.fn()}
        agentsPanelProps={{} as never}
        outputHeaderProps={{ developerMode: false, hasAcp: false } as never}
        showCreateAgent={false}
        createAgentModalProps={{} as never}
        agentNodeSectionProps={null}
        permissionModalProps={null}
        activeWorkspaceLens={activeWorkspaceLens}
        workbenchProps={null}
        nodes={[]}
        agents={[]}
        teams={[]}
        teamMemberAgentsById={{}}
        selectedNodeId=""
        nodeJoinBootstrap={null}
        nodeJoinBootstrapLoading={false}
        nodeJoinBootstrapError={null}
        updatingNodeIds={{}}
        deletingNodeIds={{}}
        onSelectNode={vi.fn()}
        onOpenNodeAgent={vi.fn()}
        onCreateAgent={vi.fn()}
        onUpdateNode={vi.fn()}
        onDeleteNode={vi.fn()}
      />
    </MantineProvider>
  );
}

describe("AgentsRouteContainer", () => {
  const rootAuth: AuthState = {
    token: "token",
    userId: "user-1",
    username: "root",
    role: "root",
  };

  it("renders shared workspace placeholders for cross-team lenses", () => {
    expect(renderRoute("search", rootAuth)).toContain("Shared search is still being wired in");
    expect(renderRoute("channels", rootAuth)).toContain(
      "Workspace channels aggregate across teams"
    );
    expect(renderRoute("tasks", rootAuth)).toContain("Workspace tasks aggregate across teams");
    expect(renderRoute("members", rootAuth)).toContain(
      "Workspace members aggregate across teams"
    );
  });

  it("renders the nodes workbench only for users who can manage nodes", () => {
    const rootHtml = renderRoute("nodes", rootAuth);
    expect(rootHtml).toContain("nodes workbench");

    const nonRootHtml = renderRoute("nodes", {
      ...rootAuth,
      role: "member",
    });
    expect(nonRootHtml).toContain("Machines unavailable");
    expect(nonRootHtml).not.toContain("nodes workbench");
  });
});
