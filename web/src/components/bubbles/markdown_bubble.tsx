import React from "react";
import {
  CONVERSATION_MESSAGE_STACK_ROW_CLASS,
  ACP_MESSAGE_BUBBLE_AGENT_CLASS,
  ACP_MESSAGE_BUBBLE_USER_CLASS,
} from "../../ui/tailwind_classes";
import { ThreadRichText } from "../thread_rich_text";

export type MarkdownBubbleProps = {
  className: "agent_message" | "user_message";
  text: string;
  markdownRenderVersion: number;
};

export const MarkdownBubble = React.memo(function MarkdownBubble({
  className,
  text,
  markdownRenderVersion,
}: MarkdownBubbleProps) {
  const isAgent = className === "agent_message";
  return (
    <div
      className={`acp-row ${CONVERSATION_MESSAGE_STACK_ROW_CLASS} ${isAgent ? "items-start" : "items-end"}`}
    >
      <div
        data-acp-message-bubble={isAgent ? "agent" : "user"}
        data-markdown-render-version={markdownRenderVersion}
        className={isAgent ? ACP_MESSAGE_BUBBLE_AGENT_CLASS : ACP_MESSAGE_BUBBLE_USER_CLASS}
      >
        <ThreadRichText text={text} />
      </div>
    </div>
  );
});
