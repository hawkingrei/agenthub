import React from "react";

export function useTeamMemberAcpPanelState() {
  const terminalRef = React.useRef<HTMLDivElement | null>(null);
  const [terminalShowJump, setTerminalShowJump] = React.useState(false);
  const [inputDockHeight, setInputDockHeight] = React.useState(0);
  const [acpModeId, setAcpModeId] = React.useState("");
  const [acpModelId, setAcpModelId] = React.useState("");
  const [acpConfigId, setAcpConfigId] = React.useState("");
  const [acpConfigValue, setAcpConfigValue] = React.useState("");

  const handleTerminalScroll = React.useCallback(() => {
    const element = terminalRef.current;
    if (!element) {
      setTerminalShowJump(false);
      return;
    }
    const remaining = element.scrollHeight - element.scrollTop - element.clientHeight;
    setTerminalShowJump(remaining > 48);
  }, []);

  const jumpToTerminalBottom = React.useCallback(() => {
    const element = terminalRef.current;
    if (!element) return;
    element.scrollTop = element.scrollHeight;
    setTerminalShowJump(false);
  }, []);

  const resetInputDockHeight = React.useCallback(() => {
    setInputDockHeight(0);
  }, []);

  return {
    terminalRef,
    terminalShowJump,
    inputDockHeight,
    acpModeId,
    acpModelId,
    acpConfigId,
    acpConfigValue,
    setInputDockHeight,
    setAcpModeId,
    setAcpModelId,
    setAcpConfigId,
    setAcpConfigValue,
    handleTerminalScroll,
    jumpToTerminalBottom,
    resetInputDockHeight,
  };
}
