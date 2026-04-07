import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  AgentsRouteShell,
  AgentsRouteShellView,
} from "./components/agents_route_shell";
import { AgentsPanelProps } from "./components/agents_panel";
import { AgentsWorkbenchProps } from "./components/agents_workbench_types";
import { OutputHeaderProps } from "./components/output_header";
import {
  OUTPUT_BODY_ACP_ROOT_CLASS,
  OUTPUT_BODY_ROOT_CLASS,
} from "./ui/tailwind_classes";

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

const baseWorkbenchProps: AgentsWorkbenchProps = {
  activeAgent: "agent-1",
  activeAgentRecord: {
    id: "agent-1",
    name: "Agent one",
    workdir: "/repo/workdir",
    command: "agenthub",
    args: [],
    target_node_id: null,
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: false,
    status: "running",
    created_at: 1,
    updated_at: 1,
  },
  activeSessionId: "session-1",
  developerMode: true,
  acpTab: "conversation",
  acpView: {
    hasAcp: true,
    toolCalls: [],
    messages: [],
    rawEvents: [],
    configOptions: [],
    plan: null,
    commands: [],
    currentMode: null,
    runStatus: null,
    thinkingStartTs: null,
  },
  eventMeta: {},
  isAgentActive: true,
  outputs: [],
  terminalOutputs: [],
  scopedAcpPermissionHistory: [],
  isOutputLoading: false,
  isConversationLoading: false,
  terminalRef: React.createRef<HTMLDivElement>(),
  input: "",
  inputHistory: [],
  ansi: (input) => input,
  canControlAcp: true,
  canInterruptAcpRun: true,
  acpModeId: "",
  acpModelId: "",
  acpConfigId: "",
  acpConfigValue: "",
  isComposingRef: { current: false },
  onLoadOlderEvents: async () => {},
  onTerminalScroll: () => {},
  onSelectTab: () => {},
  onAcpModeIdChange: () => {},
  onAcpModelIdChange: () => {},
  onAcpConfigIdChange: () => {},
  onAcpConfigValueChange: () => {},
  onAcpSetMode: () => {},
  onAcpSetModel: () => {},
  onAcpSetConfig: () => {},
  onAcpCancel: () => {},
  onAcpClearSession: () => {},
  onInputChange: () => {},
  onSelectInputHistory: () => {},
  onNavigateInputHistory: () => {},
  onSendAcpInput: async () => {},
  onJumpToTerminalBottom: () => {},
  showTerminalJump: false,
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

function renderRouteShell(
  override: Partial<React.ComponentProps<typeof AgentsRouteShell>> = {}
): string {
  return renderToStaticMarkup(
    <MantineProvider>
      <AgentsRouteShell
        agentsCollapsed={false}
        workspaceRef={React.createRef<HTMLElement>()}
        workspaceStyle={undefined}
        onAgentsSplitterPointerDown={() => {}}
        agentsPanelProps={baseAgentsPanelProps}
        outputHeaderProps={baseOutputHeaderProps}
        workbenchProps={baseWorkbenchProps}
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

describe("AgentsRouteShell", () => {
  it("renders the lazy workbench fallback when workbench props are present", () => {
    const html = renderRouteShell();

    expect(html).toContain("Loading...");
    expect(html).toContain(OUTPUT_BODY_ACP_ROOT_CLASS);
  });

  it("omits the workbench container when no active workbench props exist", () => {
    const html = renderRouteShell({
      workbenchProps: null,
    });

    expect(html).not.toContain("Loading...");
    expect(html).not.toContain(OUTPUT_BODY_ROOT_CLASS);
    expect(html).toContain("No agent selected");
  });
});
