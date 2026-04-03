import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
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
      developerMode={true}
      hasAcp={true}
      thinkingStartTs={null}
      {...props}
    />
  );

describe("OutputHeader", () => {
  it("shows empty agent prompt when no active agent", () => {
    const html = renderToStaticMarkup(
      <OutputHeader
        activeAgent={null}
        activeSessionId={null}
        developerMode={true}
        hasAcp={false}
        thinkingStartTs={null}
      />
    );
    expect(html).toContain("No agent selected");
  });

  it("renders agent meta when active agent exists", () => {
    const html = renderHeader({});
    expect(html).toContain("running");
    expect(html).toContain("Details");
    expect(html).toContain("mode");
    expect(html).toContain("on");
    expect(html).toContain("updated");
    expect(html).not.toContain("output-agents-toggle");
  });

  it("shows subtitle row when ACP is absent", () => {
    const html = renderHeader({ hasAcp: false });
    expect(html).toContain("/tmp");
  });

  it("hides subtitle row when ACP is present", () => {
    const html = renderHeader({ hasAcp: true });
    expect(html).not.toContain("/tmp");
  });

  it("does not expose session metadata even when a session is active", () => {
    const html = renderHeader({});
    expect(html).not.toContain("session");
  });

  it("omits session label when no session id is active", () => {
    const html = renderHeader({ activeSessionId: null });
    expect(html).not.toContain("session");
  });

  it("hides session metadata when developer mode is off", () => {
    const html = renderHeader({ developerMode: false });
    expect(html).toContain("Details");
    expect(html).toContain("mode");
    expect(html).toContain("on");
    expect(html).not.toContain("session");
    expect(html).not.toContain("updated");
  });

  it("renders model tag when label is provided", () => {
    const html = renderHeader({ modelLabel: "gpt-4o" });
    expect(html).toContain("gpt-4o");
  });

  it("merges run and thinking state into a single status badge", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(15_000);
    const html = renderHeader({ runStatus: "running", thinkingStartTs: 10 });
    expect(html).toContain("running · thinking 5s");
    expect(html).not.toContain("class=\"acp-thinking\"");
    nowSpy.mockRestore();
  });
});
