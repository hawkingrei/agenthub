import React from "react";
import {
  type ExploreGroupConversationItem,
  type MessageConversationItem,
  flattenExploreGroupToolCalls,
  formatConversationPreview,
  unescapeLineBreaks,
} from "../conversation";
import { resolveToolGroupStatusClassName } from "./acp_tool_call_meta";
import { deriveToolCallOpenState, isToolCallEffectivelyLive, useAutoCollapseToolFoldWhenOutOfView } from "./acp_tool_fold";
import { ToolCallBubble } from "./acp_tool_call_bubble";
import {
  ACP_TOOL_CARD_CLASS,
  ACP_TOOL_GROUP_LIST_CLASS,
  ACP_TOOL_ROW_CLASS,
  ACP_TOOL_SUMMARY_CLASS,
  ACP_TOOL_TITLE_CLASS,
  ExploreGroupBubbleProps,
  deriveToolGroupStatusSummary,
} from "./acp_tool_bubble_shared";
import { ThinkingBubble } from "./bubbles/thinking_bubble";

export const ExploreGroupBubble = React.memo(
  function ExploreGroupBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    defaultCollapsed = false,
    onSubmitRequestUserInput,
  }: ExploreGroupBubbleProps) {
    const calls = React.useMemo(() => flattenExploreGroupToolCalls(msg.items), [msg.items]);
    const isLive = React.useMemo(
      () => calls.some((call) => isToolCallEffectivelyLive(call.status, runStatus)),
      [calls, runStatus]
    );
    const [open, setOpen] = React.useState(
      () => !defaultCollapsed && !autoCollapse && isLive
    );
    const detailsRef = React.useRef<HTMLDetailsElement | null>(null);
    const handleAutoCollapse = React.useCallback(() => {
      setOpen((prev) => (prev ? false : prev));
    }, []);
    const wasLiveRef = React.useRef(isLive);
    const wasAutoCollapseRef = React.useRef(autoCollapse);
    const titlePreview = React.useMemo(() => summarizeExploreGroupPreview(msg.items), [msg.items]);
    const statusSummary = React.useMemo(
      () => deriveToolGroupStatusSummary(calls, runStatus, isToolCallEffectivelyLive),
      [calls, runStatus]
    );

    React.useEffect(() => {
      if (defaultCollapsed) {
        return;
      }
      setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
      wasLiveRef.current = isLive;
    }, [defaultCollapsed, isLive]);
    React.useEffect(() => {
      if (defaultCollapsed) {
        setOpen(false);
      }
    }, [defaultCollapsed]);
    React.useEffect(() => {
      if (autoCollapse && !wasAutoCollapseRef.current) {
        setOpen(false);
      }
      wasAutoCollapseRef.current = autoCollapse;
    }, [autoCollapse]);
    useAutoCollapseToolFoldWhenOutOfView({
      detailsRef,
      enabled: true,
      onCollapse: handleAutoCollapse,
    });

    let thinkingIndex = 0;
    let toolIndex = 0;
    return (
      <div className={ACP_TOOL_ROW_CLASS}>
        <div className={`acp-bubble tool_call tool_call_group explore_group tool-call-enter ${ACP_TOOL_CARD_CLASS}`}>
          <details
            className="acp-tool-group-fold acp-explore-group-fold"
            ref={detailsRef}
            open={open}
            onToggle={(event) => {
              setOpen(event.currentTarget.open);
            }}
          >
            <summary className={ACP_TOOL_SUMMARY_CLASS}>
              <span className={`${ACP_TOOL_TITLE_CLASS} acp-tool-group-title`}>
                Explore ({calls.length} tools)
                {titlePreview ? (
                  <span className="ml-2 font-normal text-notion-text-muted">· {titlePreview}</span>
                ) : (
                  ""
                )}
              </span>
              {statusSummary ? (
                <span className={resolveToolGroupStatusClassName(statusSummary.tone)}>
                  {statusSummary.label}
                </span>
              ) : null}
            </summary>
            <div className={`acp-tool-group-list acp-explore-group-list ${ACP_TOOL_GROUP_LIST_CLASS}`}>
              {msg.items.map((item, idx) => {
                if (item.kind === "agent_thinking") {
                  thinkingIndex += 1;
                  return (
                    <ExploreThinkingEntry
                      key={`thinking:${item.event_id ?? item.seq ?? idx}`}
                      item={item}
                      index={thinkingIndex}
                    />
                  );
                }
                const groupedCalls = item.kind === "tool_call" ? [item] : item.calls;
                return (
                  <div
                    key={`tool:${item.event_id ?? item.seq ?? idx}`}
                    className="acp-tool-group-item space-y-2"
                  >
                    {groupedCalls.map((call) => {
                      toolIndex += 1;
                      return (
                        <div
                          key={`${call.id}:${call.event_id ?? call.seq ?? toolIndex}`}
                          data-tool-call-id={call.id}
                        >
                          <ToolCallBubble
                            msg={call}
                            ansi={ansi}
                            runStatus={runStatus}
                            autoCollapse={autoCollapse}
                            defaultCollapsed={defaultCollapsed}
                            grouped={true}
                            indexLabel={`#${toolIndex}`}
                            onSubmitRequestUserInput={onSubmitRequestUserInput}
                          />
                        </div>
                      );
                    })}
                  </div>
                );
              })}
            </div>
          </details>
        </div>
      </div>
    );
  },
  (prev, next) =>
    prev.msg === next.msg &&
    prev.ansi === next.ansi &&
    prev.runStatus === next.runStatus &&
    prev.autoCollapse === next.autoCollapse &&
    prev.defaultCollapsed === next.defaultCollapsed &&
    prev.onSubmitRequestUserInput === next.onSubmitRequestUserInput
);

function ExploreThinkingEntry({
  item,
  index,
}: {
  item: MessageConversationItem & { kind: "agent_thinking" };
  index: number;
}) {
  return (
    <div className="acp-tool-group-item acp-explore-thinking-item">
      <ThinkingBubble
        text={item.text}
        live={item.live}
        summaryPrefix={`Explore #${index}`}
        grouped={true}
      />
    </div>
  );
}

function summarizeExploreGroupPreview(
  items: ExploreGroupConversationItem["items"]
): string {
  const firstThought = items.find((item) => item.kind === "agent_thinking");
  if (!firstThought) return "";
  return formatConversationPreview(unescapeLineBreaks(firstThought.text), 72);
}
