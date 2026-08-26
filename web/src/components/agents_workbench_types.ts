import React from "react";
import { AcpView } from "../acp";
import { AcpPermissionRecord, AgentInputImage, AgentRecord } from "../api";
import { OutputLine } from "../output_cache";
import { AcpConversationEventMeta } from "../hooks/use_acp_conversation";

export type SendAcpInputOptions = {
  recordHistory?: boolean;
  clearComposer?: boolean;
  images?: AgentInputImage[];
};

export type AgentsWorkbenchProps = {
  activeAgent: string;
  activeAgentRecord: AgentRecord | null;
  activeSessionId: string | null;
  developerMode: boolean;
  acpTab: "conversation" | "plan" | "debug";
  acpView: AcpView;
  eventMeta: Record<string, AcpConversationEventMeta>;
  isAgentActive: boolean;
  outputs: OutputLine[];
  terminalOutputs: OutputLine[];
  scopedAcpPermissionHistory: AcpPermissionRecord[];
  isOutputLoading: boolean;
  isConversationLoading: boolean;
  terminalRef: React.RefObject<HTMLDivElement | null>;
  input: string;
  inputImages?: AgentInputImage[];
  inputHistory: string[];
  ansi: (input: string) => string;
  canControlAcp: boolean;
  canInterruptAcpRun: boolean;
  acpModeId: string;
  acpModelId: string;
  acpConfigId: string;
  acpConfigValue: string;
  isComposingRef: React.MutableRefObject<boolean>;
  onLoadOlderEvents: () => Promise<void>;
  onTerminalScroll: () => void;
  onSelectTab: (tab: "conversation" | "plan" | "debug") => void;
  onAcpModeIdChange: (value: string) => void;
  onAcpModelIdChange: (value: string) => void;
  onAcpConfigIdChange: (value: string) => void;
  onAcpConfigValueChange: (value: string) => void;
  onAcpSetMode: (value: string) => void;
  onAcpSetModel: (value: string) => void;
  onAcpSetConfig: () => void;
  onAcpCancel: () => void;
  onAcpClearSession: () => void;
  onInputChange: (value: string) => void;
  onInputImagesChange?: (images: AgentInputImage[]) => void;
  onSelectInputHistory: (value: string) => void;
  onNavigateInputHistory: (direction: "up" | "down") => void;
  onSendAcpInput: (
    rawText: string,
    options?: SendAcpInputOptions
  ) => Promise<void>;
  onJumpToTerminalBottom: () => void;
  showTerminalJump: boolean;
};
