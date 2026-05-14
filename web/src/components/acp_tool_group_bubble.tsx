import React from "react";
import { resolveToolGroupStatusClassName } from "./acp_tool_call_meta";
import { deriveToolCallOpenState, isToolCallEffectivelyLive, useAutoCollapseToolFoldWhenOutOfView } from "./acp_tool_fold";
import { ToolCallBubble } from "./acp_tool_call_bubble";
import {
  ACP_TOOL_CARD_CLASS,
  ACP_TOOL_GROUP_LIST_CLASS,
  ACP_TOOL_ROW_CLASS,
  ACP_TOOL_SUMMARY_CLASS,
  ACP_TOOL_TITLE_CLASS,
  ToolCallGroupBubbleProps,
  deriveToolGroupStatusSummary,
  summarizeToolGroupTitles,
} from "./acp_tool_bubble_shared";

export const ToolCallGroupBubble = React.memo(
  function ToolCallGroupBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    defaultCollapsed = false,
    onSubmitRequestUserInput,
  }: ToolCallGroupBubbleProps) {
    const isLive = React.useMemo(
      () => msg.calls.some((call) => isToolCallEffectivelyLive(call.status, runStatus)),
      [msg.calls, runStatus]
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
    const titlePreview = React.useMemo(() => summarizeToolGroupTitles(msg.calls), [msg.calls]);
    const statusSummary = React.useMemo(
      () => deriveToolGroupStatusSummary(msg.calls, runStatus, isToolCallEffectivelyLive),
      [msg.calls, runStatus]
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

    return (
      <div className={ACP_TOOL_ROW_CLASS}>
        <div className={`acp-bubble tool_call tool_call_group tool-call-enter ${ACP_TOOL_CARD_CLASS}`}>
          <details
            className="acp-tool-group-fold"
            ref={detailsRef}
            open={open}
            onToggle={(event) => {
              setOpen(event.currentTarget.open);
            }}
          >
            <summary className={ACP_TOOL_SUMMARY_CLASS}>
              <span className={`${ACP_TOOL_TITLE_CLASS} acp-tool-group-title`}>
                Tool Calls ({msg.calls.length})
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
            <div className={`acp-tool-group-list ${ACP_TOOL_GROUP_LIST_CLASS}`}>
              {msg.calls.map((call, idx) => (
                <div
                  key={`${call.id}:${call.event_id ?? call.seq ?? idx}`}
                  className="acp-tool-group-item"
                  data-tool-call-id={call.id}
                >
                  <ToolCallBubble
                    msg={call}
                    ansi={ansi}
                    runStatus={runStatus}
                    autoCollapse={autoCollapse}
                    defaultCollapsed={defaultCollapsed}
                    grouped={true}
                    indexLabel={`#${idx + 1}`}
                    onSubmitRequestUserInput={onSubmitRequestUserInput}
                  />
                </div>
              ))}
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
