import React from "react";
import { ConversationItem, formatConversationPreview, isToolCallLive } from "../conversation";
import { renderMarkdown } from "../markdown";

type AcpConversationProps = {
  items: ConversationItem[];
  windowOffset: number;
  isFrozenView: boolean;
  shouldAutoCollapse: boolean;
  collapseCutoff: number;
  stickToBottom: boolean;
  pendingCount: number;
  avgHeight: number;
  onScroll: () => void;
  containerRef: React.RefObject<HTMLDivElement>;
  ansi: (input: string) => string;
};

export function AcpConversation({
  items,
  windowOffset,
  isFrozenView,
  shouldAutoCollapse,
  collapseCutoff,
  stickToBottom,
  pendingCount,
  avgHeight,
  onScroll,
  containerRef,
  ansi,
}: AcpConversationProps) {
  return (
    <div className="acp-conversation" ref={containerRef} onScroll={onScroll}>
      <div className="acp-conversation-inner">
        {items.map((msg, idx) => {
          const key = `${windowOffset + idx}-${msg.kind}`;
          const globalIndex = isFrozenView ? idx : windowOffset + idx;
          const autoCollapse = shouldAutoCollapse && globalIndex < collapseCutoff;
          if (msg.kind === "agent_thinking") {
            const preview = autoCollapse
              ? formatConversationPreview(msg.text, 80)
              : "";
            const summary = msg.live
              ? "Thinking (live)"
              : autoCollapse
                ? `Thinking: ${preview}`
                : "Thinking (collapsed)";
            return (
              <div key={key} className="acp-bubble agent_thinking">
                <details className="acp-thought-fold" open={msg.live}>
                  <summary>{summary}</summary>
                  <div className="acp-text">
                    <pre>{msg.text}</pre>
                  </div>
                </details>
              </div>
            );
          }
          if (msg.kind === "agent_plan") {
            const preview = autoCollapse
              ? formatConversationPreview(msg.text, 80)
              : "";
            const summary = autoCollapse
              ? `Plan: ${preview}`
              : "Plan (collapsed)";
            return (
              <div key={key} className="acp-bubble agent_plan">
                <details className="acp-thought-fold">
                  <summary>{summary}</summary>
                  <div className="acp-text">
                    <pre>{msg.text}</pre>
                  </div>
                </details>
              </div>
            );
          }
          if (msg.kind === "tool_call") {
            const isLive = isToolCallLive(msg.status);
            return (
              <div key={key} className="acp-bubble tool_call">
                <details
                  className="acp-tool-fold"
                  {...(isLive ? { open: true } : {})}
                >
                  <summary>
                    <span className="acp-tool-title">
                      Tool Call
                      {msg.title ? `: ${msg.title}` : ""}
                    </span>
                    {msg.status && (
                      <span className="acp-tool-status">{msg.status}</span>
                    )}
                  </summary>
                  {msg.content && (
                    <div className="acp-text">
                      <pre>{unescapeLineBreaks(msg.content)}</pre>
                    </div>
                  )}
                  {msg.raw_input && (
                    <pre className="acp-content">
                      {formatToolCallPayload(msg.raw_input)}
                    </pre>
                  )}
                  {msg.raw_output && (
                    <pre className="acp-content">
                      {formatToolCallPayload(msg.raw_output)}
                    </pre>
                  )}
                  {msg.terminal_output && (
                    <pre
                      className="acp-content"
                      dangerouslySetInnerHTML={{
                        __html: ansi(unescapeLineBreaks(msg.terminal_output)),
                      }}
                    />
                  )}
                </details>
              </div>
            );
          }
          if (msg.kind === "agent_message") {
            return (
              <div key={key} className="acp-bubble agent_message">
                <div
                  className="acp-text"
                  dangerouslySetInnerHTML={{
                    __html: renderMarkdown(msg.text),
                  }}
                />
              </div>
            );
          }
          return (
            <div key={key} className="acp-bubble user_message">
              <div
                className="acp-text"
                dangerouslySetInnerHTML={{
                  __html: renderMarkdown(msg.text),
                }}
              />
            </div>
          );
        })}
        {!stickToBottom && pendingCount > 0 && (
          <div
            className="acp-conversation-spacer"
            style={{ height: Math.round(pendingCount * avgHeight) }}
          />
        )}
      </div>
    </div>
  );
}

export type { AcpConversationProps };

function unescapeLineBreaks(text: string): string {
  return text
    .replace(/\\r\\n/g, "\n")
    .replace(/\\n/g, "\n")
    .replace(/\\t/g, "\t")
    .replace(/\\r/g, "\n");
}

function formatToolCallPayload(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return unescapeLineBreaks(value);
  return unescapeLineBreaks(JSON.stringify(value, null, 2));
}
