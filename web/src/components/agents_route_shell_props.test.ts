import React from "react";
import { describe, expect, it, vi } from "vitest";
import {
  buildAgentsPanelProps,
  buildAgentsWorkbenchProps,
  buildOutputHeaderProps,
} from "./agents_route_shell_props";
import { AgentsPanelProps } from "./agents_panel";
import { AgentsWorkbenchProps } from "./agents_workbench_types";
import { OutputHeaderProps } from "./output_header";

const baseAgentsPanelProps: AgentsPanelProps = {
  agents: [],
  activeAgent: null,
  agentsCollapsed: false,
  compactRows: false,
  hasPendingPermissions: false,
  pendingPermissionCounts: {},
  startingAgentIds: {},
  onCollapse: vi.fn(),
  onExpand: vi.fn(),
  onCreateAgent: vi.fn(),
  onSelectAgent: vi.fn(),
  onToggleCodeMode: vi.fn(),
  onStartAgent: vi.fn(),
  onStopAgent: vi.fn(),
  onDeleteAgent: vi.fn(),
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

describe("agents route shell props helpers", () => {
  it("keeps panel and header props unchanged", () => {
    expect(buildAgentsPanelProps(baseAgentsPanelProps)).toBe(baseAgentsPanelProps);
    expect(buildOutputHeaderProps(baseOutputHeaderProps)).toBe(
      baseOutputHeaderProps
    );
  });

  it("returns null workbench props without an active agent", () => {
    expect(
      buildAgentsWorkbenchProps({
        ...baseWorkbenchProps,
        activeAgent: null,
      })
    ).toBeNull();
  });

  it("preserves workbench wiring when an active agent exists", () => {
    const result = buildAgentsWorkbenchProps(baseWorkbenchProps);

    expect(result).toEqual(baseWorkbenchProps);
    expect(result).not.toBeNull();
    expect(result?.activeAgent).toBe("agent-1");
    expect(result?.onSendAcpInput).toBe(baseWorkbenchProps.onSendAcpInput);
  });
});
