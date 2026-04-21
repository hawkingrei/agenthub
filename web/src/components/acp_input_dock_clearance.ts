export const ACP_INPUT_DOCK_CONVERSATION_CLEARANCE_PX = 64;
export const ACP_INPUT_DOCK_CONVERSATION_MARGIN_PX = 8;

export function resolveAcpInputDockConversationClearance(
  inputDockHeight: number
): number {
  const normalizedHeight = Number.isFinite(inputDockHeight)
    ? Math.max(0, Math.ceil(inputDockHeight))
    : 0;
  return Math.max(
    ACP_INPUT_DOCK_CONVERSATION_CLEARANCE_PX,
    normalizedHeight + ACP_INPUT_DOCK_CONVERSATION_MARGIN_PX
  );
}
