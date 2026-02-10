import React from "react";
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
    expect(html).toContain("Agents");
    expect(html).toContain("Running");
    expect(html).not.toContain("Create Agent");
  });

  it("renders list and actions when expanded", () => {
    const html = renderToStaticMarkup(
      <AgentsPanel {...baseProps} agentsCollapsed={false} />
    );
    expect(html).toContain("Agents</h2>");
    expect(html).toContain("Create Agent");
    expect(html).toContain("Alpha");
    expect(html).toContain("Beta");
    expect(html).toContain("running");
  });
});
