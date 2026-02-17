import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { OutputHeader } from "./components/output_header";
import { AgentRecord } from "./api";

const baseAgent: AgentRecord = {
  id: "agent-1",
  name: "Alpha",
  workdir: "/tmp",
  command: "agenthub",
  args: [],
  worktree_mode: "use_existing",
  worktree_repo: null,
  worktree_ref: null,
  code_mode: true,
  status: "running",
  created_at: 1,
  updated_at: 10,
};

const renderHeader = (props: Partial<React.ComponentProps<typeof OutputHeader>>) =>
  renderToStaticMarkup(
    <OutputHeader
      activeAgent={baseAgent}
      activeSessionId="session-12345678"
      agentsCollapsed={false}
      hasAcp={true}
      thinkingStartTs={null}
      onToggleAgents={() => {}}
      {...props}
    />
  );

describe("OutputHeader", () => {
  it("shows empty agent prompt when no active agent", () => {
    const html = renderToStaticMarkup(
      <OutputHeader
        activeAgent={null}
        activeSessionId={null}
        agentsCollapsed={false}
        hasAcp={false}
        thinkingStartTs={null}
        onToggleAgents={() => {}}
      />
    );
    expect(html).toContain("No agent selected");
  });

  it("renders agent meta when active agent exists", () => {
    const html = renderHeader({});
    expect(html).toContain("running");
    expect(html).toContain("Code mode on");
    expect(html).toContain("Session session-");
    expect(html).toContain("Updated");
    expect(html).toContain("output-agents-toggle");
  });

  it("shows subtitle row when ACP is absent", () => {
    const html = renderHeader({ hasAcp: false });
    expect(html).toContain("/tmp");
  });

  it("hides subtitle row when ACP is present", () => {
    const html = renderHeader({ hasAcp: true });
    expect(html).not.toContain("/tmp");
  });

  it("omits session label when no session id is active", () => {
    const html = renderHeader({ activeSessionId: null });
    expect(html).not.toContain("Session");
  });

  it("renders model tag when label is provided", () => {
    const html = renderHeader({ modelLabel: "gpt-4o" });
    expect(html).toContain("gpt-4o");
  });
});
