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
          const globalIndex = isFrozenView ? idx : windowOffset + idx;
          const key = getConversationItemKey(msg, globalIndex);
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
            return (
              <ToolCallBubble key={key} msg={msg} ansi={ansi} />
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

type ToolCallBubbleProps = {
  msg: Extract<ConversationItem, { kind: "tool_call" }>;
  ansi: (input: string) => string;
};

function ToolCallBubble({ msg, ansi }: ToolCallBubbleProps) {
  const isLive = isToolCallLive(msg.status);
  const [open, setOpen] = React.useState(isLive);
  const wasLiveRef = React.useRef(isLive);

  React.useEffect(() => {
    setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
    wasLiveRef.current = isLive;
  }, [isLive]);

  return (
    <div className="acp-bubble tool_call">
      <details
        className="acp-tool-fold"
        open={open}
        onToggle={(event) => {
          setOpen(event.currentTarget.open);
        }}
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
          <pre className="acp-content">
            {renderAnsiTerminalOutput(
              ansi(unescapeLineBreaks(msg.terminal_output))
            )}
          </pre>
        )}
      </details>
    </div>
  );
}

export function deriveToolCallOpenState(
  prevOpen: boolean,
  wasLive: boolean,
  isLive: boolean
): boolean {
  if (isLive) return true;
  if (wasLive) return false;
  return prevOpen;
}

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

function getConversationItemKey(msg: ConversationItem, fallback: number): string {
  if (msg.kind === "tool_call") return `tool_call:${msg.id}`;
  if (msg.event_id != null) return `${msg.kind}:event:${msg.event_id}`;
  if (msg.seq) return `${msg.kind}:seq:${msg.seq}`;
  return `${msg.kind}:idx:${fallback}`;
}

const ANSI_SPAN_TAG_PATTERN = /<\/?span(?: style="([a-zA-Z0-9:#;(),.%\s-]*)")?>/g;
const ANSI_STYLE_VALUE_PATTERN = /^[a-zA-Z0-9#(),.%\s-]+$/;
const ANSI_ALLOWED_STYLE_PROPERTIES: Record<string, keyof React.CSSProperties> = {
  color: "color",
  "background-color": "backgroundColor",
  "font-weight": "fontWeight",
  "font-style": "fontStyle",
  "text-decoration": "textDecoration",
  "text-decoration-line": "textDecorationLine",
  "text-decoration-style": "textDecorationStyle",
};

type AnsiSegment = {
  text: string;
  style?: React.CSSProperties;
};

function renderAnsiTerminalOutput(input: string): React.ReactNode[] {
  return parseAnsiSegments(input).map((segment, index) => {
    if (segment.style) {
      return (
        <span key={index} style={segment.style}>
          {segment.text}
        </span>
      );
    }
    return <React.Fragment key={index}>{segment.text}</React.Fragment>;
  });
}

function parseAnsiSegments(input: string): AnsiSegment[] {
  const segments: AnsiSegment[] = [];
  const styleStack: React.CSSProperties[] = [];
  let cursor = 0;
  ANSI_SPAN_TAG_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = ANSI_SPAN_TAG_PATTERN.exec(input)) != null) {
    if (match.index > cursor) {
      pushAnsiSegment(segments, input.slice(cursor, match.index), styleStack);
    }
    if (match[0].startsWith("</")) {
      if (styleStack.length > 0) {
        styleStack.pop();
      } else {
        pushAnsiSegment(segments, match[0], styleStack);
      }
    } else {
      styleStack.push(parseAnsiStyle(match[1] ?? ""));
    }
    cursor = ANSI_SPAN_TAG_PATTERN.lastIndex;
  }
  if (cursor < input.length) {
    pushAnsiSegment(segments, input.slice(cursor), styleStack);
  }
  return segments;
}

function pushAnsiSegment(
  segments: AnsiSegment[],
  text: string,
  styleStack: React.CSSProperties[]
): void {
  if (!text) return;
  const style = mergeAnsiStyles(styleStack);
  segments.push(style ? { text, style } : { text });
}

function mergeAnsiStyles(styleStack: React.CSSProperties[]): React.CSSProperties | undefined {
  if (styleStack.length === 0) return undefined;
  const merged: React.CSSProperties = {};
  for (const style of styleStack) {
    Object.assign(merged, style);
  }
  return Object.keys(merged).length > 0 ? merged : undefined;
}

function parseAnsiStyle(rawStyle: string): React.CSSProperties {
  const parsed: React.CSSProperties = {};
  for (const entry of rawStyle.split(";")) {
    const separator = entry.indexOf(":");
    if (separator < 0) continue;
    const rawKey = entry.slice(0, separator).trim().toLowerCase();
    const rawValue = entry.slice(separator + 1).trim();
    const targetKey = ANSI_ALLOWED_STYLE_PROPERTIES[rawKey];
    if (!targetKey) continue;
    if (!rawValue || !ANSI_STYLE_VALUE_PATTERN.test(rawValue)) continue;
    (parsed as Record<string, string>)[targetKey] = rawValue;
  }
  return parsed;
}
