import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpView } from "./acp";
import { AgentsWorkbench } from "./components/agents_workbench";
import { AgentsWorkbenchProps } from "./components/agents_workbench_types";

const baseView: AcpView = {
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
};

function createProps(
  override: Partial<AgentsWorkbenchProps> = {}
): AgentsWorkbenchProps {
  return {
    activeAgent: "agent-1",
    activeAgentRecord: {
      id: "agent-1",
      name: "agent-1",
      command: "agenthub",
      workdir: "/repo/workdir",
      status: "running",
      created_at: "2026-04-06T00:00:00Z",
      updated_at: "2026-04-06T00:00:00Z",
      code_mode: false,
      hidden: false,
      provider: null,
      provider_label: null,
      model: null,
      acp_session_id: null,
      agent_loop_enabled: false,
      agent_loop_prompt: null,
      agent_loop_max_steps: null,
      worktree_mode: null,
      worktree_name_template: null,
      profile_name: null,
      profile_path: null,
      session_id: null,
      node_id: null,
      self_update_enabled: false,
      self_update_interval_secs: null,
      next_self_update_at: null,
    },
    activeSessionId: "session-1",
    developerMode: true,
    acpTab: "conversation",
    acpView: baseView,
    eventMeta: {},
    isAgentActive: true,
    outputs: [],
    terminalOutputs: [],
    scopedAcpPermissionHistory: [],
    isOutputLoading: false,
    isConversationLoading: false,
    terminalRef: React.createRef<HTMLDivElement>(),
    input: "hello",
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
    ...override,
  };
}

function renderWorkbench(override?: Partial<AgentsWorkbenchProps>): string {
  return renderToStaticMarkup(
    <MantineProvider>
      <AgentsWorkbench {...createProps(override)} />
    </MantineProvider>
  );
}

describe("AgentsWorkbench", () => {
  it("shows the input dock for ACP conversation mode", () => {
    const html = renderWorkbench();
    expect(html).toContain("Send");
    expect(html).toContain("Conversation");
    expect(html).toContain("/repo/workdir");
  });

  it("hides the input dock when ACP debug owns the footer", () => {
    const html = renderWorkbench({
      acpTab: "debug",
      developerMode: true,
      acpView: { ...baseView, currentMode: "danger-full-access" },
    });
    expect(html).toContain("Loading debug...");
    expect(html).not.toContain("Send");
    expect(html).not.toContain("Jump to latest output");
  });
});
