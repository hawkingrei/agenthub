import React from "react";
import {
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
      className={`acp-row group relative mb-1 flex w-full flex-col rounded-lg border-2 border-transparent px-2 py-2 transition hover:border-black hover:bg-white active:border-black active:bg-white ${isAgent ? "items-start" : "items-end"}`}
    >
      <div
        data-acp-message-bubble={isAgent ? "agent" : "user"}
        className={isAgent ? ACP_MESSAGE_BUBBLE_AGENT_CLASS : ACP_MESSAGE_BUBBLE_USER_CLASS}
      >
        <ThreadRichText key={markdownRenderVersion} text={text} />
      </div>
    </div>
  );
});
