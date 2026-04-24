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

export function AcpConversationItemRow({
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
