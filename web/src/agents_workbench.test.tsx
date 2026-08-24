import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpView } from "./acp";
import { AgentRecord } from "./api";
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
  const activeAgentRecord: AgentRecord = {
    id: "agent-1",
    name: "agent-1",
    command: "agenthub",
    args: [],
    workdir: "/repo/workdir",
    status: "running",
    created_at: 1_775_491_200,
    updated_at: 1_775_491_200,
    code_mode: false,
    worktree_mode: "use_existing",
  };

  return {
    activeAgent: "agent-1",
    activeAgentRecord,
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
    expect(html).toContain("Thread");
    expect(html).toContain("/repo/workdir");
    expect(html).toContain('data-standalone-acp-context="true"');
    expect(html).toContain('aria-label="Attach images"');
  });

  it("summarizes active Codex subagents in the standalone header", () => {
    const html = renderWorkbench({
      acpView: {
        ...baseView,
        toolCalls: [
          {
            id: "codex-subagent:child",
            title: "Subagent child",
            status: "in_progress",
            meta: { agenthub: { kind: "codex_subagent" } },
          },
        ],
      },
    });

    expect(html).toContain("1 active / 1 total");
  });

  it("prefers live Codex reasoning and mode context in the standalone header", () => {
    const html = renderWorkbench({
      activeAgentRecord: {
        ...createProps().activeAgentRecord!,
        thinking_level: "high",
        codex_acp_default_mode: "full-access",
      },
      acpView: {
        ...baseView,
        currentMode: "read-only",
        configOptions: [
          {
            id: "reasoning_effort",
            label: "Reasoning effort",
            currentValueId: "ultra",
            selectOptions: [],
          },
        ],
      },
    });

    expect(html).toContain("Ultra");
    expect(html).toContain("Read Only");
    expect(html).not.toContain("Full Access");
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
