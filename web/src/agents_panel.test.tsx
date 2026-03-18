import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentsPanel } from "./components/agents_panel";
import { AgentRecord } from "./api";

const agents: AgentRecord[] = [
  {
    id: "agent-1",
    name: "Alpha",
    workdir: "/tmp/alpha",
    command: "agenthub",
    args: [],
    target_node_id: null,
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: true,
    status: "running",
    created_at: 1,
    updated_at: 2,
  },
  {
    id: "agent-2",
    name: "Beta",
    workdir: "/tmp/beta",
    command: "agenthub",
    args: [],
    target_node_id: null,
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: false,
    status: "idle",
    created_at: 1,
    updated_at: 2,
  },
];

const baseProps = {
  agents,
  activeAgent: "agent-1",
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

describe("AgentsPanel", () => {
  it("renders collapsed rail when collapsed", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel {...baseProps} agentsCollapsed={true} />
    );
    expect(html).toContain("agents-rail");
    expect(html).toContain('aria-label="Show agents"');
    expect(html).toContain('aria-label="Create agent"');
    expect(html).toContain("Agents");
    expect(html).toContain("Running");
    expect(html).not.toContain("Create Agent");
    expect(html).not.toContain('aria-label="Hide agents"');
    expect(html).not.toContain("agents-backdrop");
  });

  it("renders collapsed permission indicator when pending permissions exist", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel
        {...baseProps}
        agentsCollapsed={true}
        hasPendingPermissions={true}
      />
    );
    expect(html).toContain("agents-rail-dot");
    expect(html).toContain('role="img"');
    expect(html).toContain('aria-label="Pending permissions"');
  });

  it("renders list and actions when expanded", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel {...baseProps} agentsCollapsed={false} />
    );
    expect(html).toContain("agents-backdrop");
    expect(html).toContain("Agents</h2>");
    expect(html).not.toContain('href="/teams"');
    expect(html).toContain('aria-label="Hide agents"');
    expect(html).toContain("Create Agent");
    expect(html).toContain("Alpha");
    expect(html).toContain("Beta");
    expect(html).toContain("agents-workbench-row");
    expect(html).toContain("agents-workbench-name");
    expect(html).toContain("running");
  });

  it("renders per-agent permission indicator in expanded mode", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel
        {...baseProps}
        agentsCollapsed={false}
        pendingPermissionCounts={{ "agent-1": 2 }}
      />
    );
    expect(html).toContain("agents-workbench-permission-dot");
    expect(html).toContain('aria-label="2 pending permissions for Alpha"');
  });

  it("renders singular pending-permission label for one pending item", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel
        {...baseProps}
        agentsCollapsed={false}
        pendingPermissionCounts={{ "agent-2": 1 }}
      />
    );
    expect(html).toContain('aria-label="1 pending permission for Beta"');
  });

  it("omits per-agent indicator when count is zero", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel
        {...baseProps}
        agentsCollapsed={false}
        pendingPermissionCounts={{ "agent-1": 0 }}
      />
    );
    expect(html).not.toContain("agents-workbench-permission-dot");
  });

  it("renders spinning start icon when agent is starting", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel
        {...baseProps}
        agentsCollapsed={false}
        startingAgentIds={{ "agent-2": true }}
      />
    );
    expect(html).toContain("bi-arrow-repeat");
    expect(html).toContain("animate-spin");
    expect(html).toContain('aria-label="Starting"');
  });

  it("marks the expanded panel for compact two-line rows when requested", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel {...baseProps} agentsCollapsed={false} compactRows={true} />
    );
    expect(html).toContain("agents-panel-compact-rows");
  });

  it("shows remote node badge and renders remote-start affordance copy", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel
        {...baseProps}
        agentsCollapsed={false}
        agents={[
          {
            ...agents[0],
            id: "agent-remote",
            name: "Remote",
            status: "created",
            target_node_id: "node-east",
          },
        ]}
        activeAgent="agent-remote"
      />
    );
    expect(html).toContain("node:node-east");
    expect(html).toContain("Start on node node-east");
    expect(html).toContain('aria-label="Start agent on node node-east"');
  });
});
