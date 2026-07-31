import React from "react";
import { type AcpConversationBubbleProps, AcpConversationBubble } from "./acp_conversation_bubble";
import {
  getConversationItemKey,
  getConversationItemToolCallId,
  isConversationItemFocusedToolCall,
} from "./acp_conversation_item_meta";

type AcpConversationItemRowProps = AcpConversationBubbleProps & {
  focusedToolCallId?: string | null;
};

export { getConversationItemKey } from "./acp_conversation_item_meta";

function AcpConversationItemRowInner({
  focusedToolCallId,
  ...bubbleProps
}: AcpConversationItemRowProps) {
  const { msg, globalIndex } = bubbleProps;
  const key = getConversationItemKey(msg, globalIndex);
  const isFocusedToolCall = isConversationItemFocusedToolCall(
    msg,
    focusedToolCallId ?? null
  );
  return (
    <div
      className={`acp-conversation-item${isFocusedToolCall ? " is-focused ring-2 ring-sky-300 ring-offset-2 ring-offset-white" : ""}`}
      data-conversation-item-key={key}
      data-tool-call-id={getConversationItemToolCallId(msg)}
    >
      <AcpConversationBubble {...bubbleProps} />
    </div>
  );
}

export const AcpConversationItemRow = React.memo(
  AcpConversationItemRowInner,
  areAcpConversationItemRowPropsEqual
);

export function areAcpConversationItemRowPropsEqual(
  prev: Readonly<AcpConversationItemRowProps>,
  next: Readonly<AcpConversationItemRowProps>
): boolean {
  if (prev.msg !== next.msg) return false;
  if (prev.globalIndex !== next.globalIndex) return false;
  if (prev.latestVisibleGlobalIndex !== next.latestVisibleGlobalIndex) return false;
  if (prev.shouldAutoCollapse !== next.shouldAutoCollapse) return false;
  if (prev.collapseCutoff !== next.collapseCutoff) return false;
  if (prev.toolCallsDefaultCollapsed !== next.toolCallsDefaultCollapsed) return false;
  if (prev.isFrozenView !== next.isFrozenView) return false;
  if (prev.runStatus !== next.runStatus) return false;
  if (prev.ansi !== next.ansi) return false;
  if (prev.markdownRenderVersion !== next.markdownRenderVersion) return false;
  if (prev.onSubmitRequestUserInput !== next.onSubmitRequestUserInput) return false;
  return (
    isConversationItemFocusedToolCall(prev.msg, prev.focusedToolCallId ?? null) ===
    isConversationItemFocusedToolCall(next.msg, next.focusedToolCallId ?? null)
  );
}
