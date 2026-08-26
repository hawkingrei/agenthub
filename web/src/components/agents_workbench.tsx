import React from "react";
import { InputDock } from "./input_dock";
import { OutputBody } from "./output_body";
import { AgentsWorkbenchProps } from "./agents_workbench_types";
import { useAgentsWorkbenchPanel } from "./use_agents_workbench_panel";

function AgentsWorkbenchView({
  activeAgent,
  activeAgentRecord,
  activeSessionId,
  developerMode,
  acpTab,
  acpView,
  eventMeta,
  isAgentActive,
  outputs,
  terminalOutputs,
  scopedAcpPermissionHistory,
  isOutputLoading,
  isConversationLoading,
  terminalRef,
  input,
  inputImages,
  inputHistory,
  ansi,
  canControlAcp,
  canInterruptAcpRun,
  acpModeId,
  acpModelId,
  acpConfigId,
  acpConfigValue,
  isComposingRef,
  onLoadOlderEvents,
  onTerminalScroll,
  onSelectTab,
  onAcpModeIdChange,
  onAcpModelIdChange,
  onAcpConfigIdChange,
  onAcpConfigValueChange,
  onAcpSetMode,
  onAcpSetModel,
  onAcpSetConfig,
  onAcpCancel,
  onAcpClearSession,
  onInputChange,
  onInputImagesChange,
  onSelectInputHistory,
  onNavigateInputHistory,
  onSendAcpInput,
  onJumpToTerminalBottom,
  showTerminalJump,
}: AgentsWorkbenchProps) {
  const {
    acpPanelProps,
    showInputDock,
    inputDockJumpMode,
    setInputDockHeight,
    onSendInput,
  } = useAgentsWorkbenchPanel({
    activeAgent,
    activeAgentRecord,
    activeSessionId,
    developerMode,
    acpTab,
    acpView,
    eventMeta,
    isAgentActive,
    outputs,
    terminalOutputs,
    scopedAcpPermissionHistory,
    isOutputLoading,
    isConversationLoading,
    terminalRef,
    input,
    inputImages,
    inputHistory,
    ansi,
    canControlAcp,
    canInterruptAcpRun,
    acpModeId,
    acpModelId,
    acpConfigId,
    acpConfigValue,
    isComposingRef,
    onLoadOlderEvents,
    onTerminalScroll,
    onSelectTab,
    onAcpModeIdChange,
    onAcpModelIdChange,
    onAcpConfigIdChange,
    onAcpConfigValueChange,
    onAcpSetMode,
    onAcpSetModel,
    onAcpSetConfig,
    onAcpCancel,
    onAcpClearSession,
    onInputChange,
    onSelectInputHistory,
    onNavigateInputHistory,
    onSendAcpInput,
    onJumpToTerminalBottom,
    showTerminalJump,
  });

  return (
    <>
      <OutputBody
        terminalRef={terminalRef}
        onTerminalScroll={onTerminalScroll}
        isOutputLoading={isOutputLoading}
        isConversationLoading={isConversationLoading}
        outputs={outputs}
        ansi={ansi}
        acpPanelProps={acpPanelProps}
      />
      {showInputDock ? (
        <InputDock
          key={activeAgent ?? "standalone-acp"}
          input={input}
          images={inputImages}
          enableImages={true}
          historyCommands={inputHistory}
          showInterrupt={acpView.hasAcp}
          canInterrupt={canInterruptAcpRun}
          onHeightChange={setInputDockHeight}
          onInputChange={onInputChange}
          onImagesChange={onInputImagesChange}
          onSendInput={onSendInput}
          onInterrupt={onAcpCancel}
          onNavigateHistory={onNavigateInputHistory}
          onSelectHistoryCommand={onSelectInputHistory}
          onJumpToBottom={inputDockJumpMode.onJumpToBottom}
          showConversationJump={inputDockJumpMode.showConversationJump}
          isComposingRef={isComposingRef}
        />
      ) : null}
    </>
  );
}

export const AgentsWorkbench = React.memo(AgentsWorkbenchView);
AgentsWorkbench.displayName = "AgentsWorkbench";
