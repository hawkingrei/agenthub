import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { AgentsRootPage, AgentsRootPageProps } from "./components/agents_root_page";
import { AgentsPanelProps } from "./components/agents_panel";
import { AgentsWorkbenchProps } from "./components/agents_workbench_types";
import { OutputHeaderProps } from "./components/output_header";
import { AuthState } from "./types";

const baseAuth: AuthState = {
  token: "token-root",
  userId: "user-1",
  username: "root",
  role: "root",
};

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

const baseProps: AgentsRootPageProps = {
  appRootRef: React.createRef<HTMLDivElement>(),
  appHeaderRef: React.createRef<HTMLElement>(),
  auth: null,
  normalizedError: null,
  onClearError: () => {},
  authBusy: null,
  rootInitialized: true,
  username: "",
  password: "",
  displayName: "",
  setUsername: vi.fn(),
  setPassword: vi.fn(),
  setDisplayName: vi.fn(),
  onLogin: async () => {},
  onRegister: async () => {},
  agentsCollapsed: false,
  onCollapseAgents: () => {},
  onExpandAgents: () => {},
  connectionBadge: {
    tone: "muted",
    label: "ONLINE · SSE IDLE",
    title: "Network online. No active SSE stream target.",
  },
  onLogout: () => {},
  navigateWorkbenchRoute: () => {},
  workspaceRef: React.createRef<HTMLElement>(),
  workspaceStyle: undefined,
  onAgentsSplitterPointerDown: () => {},
  agentsPanelProps: baseAgentsPanelProps,
  outputHeaderProps: baseOutputHeaderProps,
  workbenchProps: baseWorkbenchProps,
  showCreateAgent: false,
  createAgentModalProps: {
    agentName: "",
    setAgentName: vi.fn(),
    agentWorkdir: "",
    setAgentWorkdir: vi.fn(),
    agentPresetId: "codex_default",
    setAgentPresetId: vi.fn(),
    worktreeMode: "use_existing",
    setWorktreeMode: vi.fn(),
    worktreeRepo: "",
    setWorktreeRepo: vi.fn(),
    worktreeRef: "",
    setWorktreeRef: vi.fn(),
    codeMode: true,
    setCodeMode: vi.fn(),
    worktreeError: null,
    createBusy: false,
    onCreateAgent: vi.fn(),
    onClose: vi.fn(),
  },
  agentNodeSectionProps: null,
  permissionModalProps: null,
};

function renderHtml(
  override: Partial<AgentsRootPageProps> = {}
): string {
  return renderToStaticMarkup(
    <MantineProvider>
      <AgentsRootPage {...baseProps} {...override} />
    </MantineProvider>
  );
}

describe("AgentsRootPage", () => {
  it("renders the login form when auth is absent", () => {
    const html = renderHtml({
      rootInitialized: false,
    });

    expect(html).toContain("Login");
    expect(html).toContain("Initialize Root");
    expect(html).toContain('name="username"');
    expect(html).toContain('name="password"');
    expect(html).toContain('name="display_name"');
  });

  it("renders the authenticated agents workbench shell when auth is present", () => {
    const html = renderHtml({
      auth: baseAuth,
    });

    expect(html).toContain("Open workbench menu");
    expect(html).toContain("No agent selected");
    expect(html).toContain("Create Agent");
  });
});
