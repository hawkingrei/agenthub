type InputDockJumpModeArgs = {
  hasAcp: boolean;
  showConversationJump: boolean;
  jumpToConversationBottom: () => void;
  showTerminalJump: boolean;
  jumpToTerminalBottom: () => void;
};

export function resolveInputDockJumpMode({
  hasAcp,
  showConversationJump,
  jumpToConversationBottom,
  showTerminalJump,
  jumpToTerminalBottom,
}: InputDockJumpModeArgs): {
  showConversationJump: boolean;
  onJumpToBottom: () => void;
} {
  if (hasAcp) {
    return {
      showConversationJump,
      onJumpToBottom: jumpToConversationBottom,
    };
  }
  return {
    showConversationJump: showTerminalJump,
    onJumpToBottom: jumpToTerminalBottom,
  };
}
