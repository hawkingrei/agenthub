import React from "react";
import {
  CONVERSATION_MESSAGE_STACK_ROW_CLASS,
  ACP_MESSAGE_BUBBLE_AGENT_CLASS,
  ACP_MESSAGE_BUBBLE_USER_CLASS,
} from "../../ui/tailwind_classes";
import { ThreadRichText } from "../thread_rich_text";
import type { AcpMedia } from "../../acp";
import { AcpMediaGallery } from "../acp_media_gallery";

export type MarkdownBubbleProps = {
  className: "agent_message" | "user_message";
  text: string;
  media?: AcpMedia[];
  delivery?: string;
  markdownRenderVersion: number;
};

export const MarkdownBubble = React.memo(function MarkdownBubble({
  className,
  text,
  media,
  delivery,
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
        {delivery === "async" ? (
          <div className="mb-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-indigo-600">
            Background update
          </div>
        ) : null}
        <div className="space-y-2">
          {text ? <ThreadRichText text={text} /> : null}
          <AcpMediaGallery media={media} />
        </div>
      </div>
    </div>
  );
});
