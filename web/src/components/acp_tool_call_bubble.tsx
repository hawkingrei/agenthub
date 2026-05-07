import React from "react";
import { ACP_TOOL_STATUS_CLASS, ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS } from "../ui/tailwind_classes";
import { formatRequestUserInputSummary, parseRequestUserInputQuestions, parseRequestUserInputResponse } from "../request_user_input";
import { extractToolCallDetails, formatToolCallDurationLabel, selectToolCallOutputForDisplay } from "./acp_tool_call_meta";
import {
  TOOL_PAYLOAD_PREVIEW_LIMIT,
  TerminalOutputView,
  ToolCallDetailsView,
  ToolPayloadView,
  ToolTextContent,
  hasToolPayload,
  normalizeToolPayload,
  shouldAutoExpandToolContent,
  summarizeToolPayload,
} from "./acp_tool_content";
import { FoldSection, deriveToolCallOpenState, isToolCallEffectivelyLive, useAutoCollapseToolFoldWhenOutOfView } from "./acp_tool_fold";
import { RequestUserInputCard, RequestUserInputResultCard } from "./acp_request_user_input_cards";
import {
  ACP_TOOL_CARD_CLASS,
  ACP_TOOL_CARD_NESTED_CLASS,
  ACP_TOOL_ROW_CLASS,
  ACP_TOOL_SUMMARY_CLASS,
  ACP_TOOL_TITLE_CLASS,
  ToolCallBubbleProps,
  deriveToolCallHint,
  formatTerminalActivityLabel,
  formatToolCallStatus,
  getToolCallStatusMark,
  resolveEffectiveToolCallStatus,
} from "./acp_tool_bubble_shared";
import { formatConversationPreview, unescapeLineBreaks } from "../conversation";

export const ToolCallBubble = React.memo(
  function ToolCallBubble({
    msg,
    ansi,
    runStatus,
    autoCollapse = false,
    grouped = false,
    indexLabel,
    onSubmitRequestUserInput,
  }: ToolCallBubbleProps) {
    const isLive = isToolCallEffectivelyLive(msg.status, runStatus);
    const [open, setOpen] = React.useState(() => !autoCollapse && isLive);
    const detailsRef = React.useRef<HTMLDetailsElement | null>(null);
    const handleAutoCollapse = React.useCallback(() => {
      setOpen((prev) => (prev ? false : prev));
    }, []);
    const wasLiveRef = React.useRef(isLive);
    const wasAutoCollapseRef = React.useRef(autoCollapse);
    const callHint = deriveToolCallHint(msg.title, msg.raw_input, msg.content);
    const requestUserInputQuestions = React.useMemo(
      () => parseRequestUserInputQuestions(msg.id, msg.raw_input),
      [msg.id, msg.raw_input]
    );
    const requestUserInputResponse = React.useMemo(
      () => parseRequestUserInputResponse(msg.raw_output),
      [msg.raw_output]
    );
    const requestUserInputSummary = React.useMemo(
      () => (requestUserInputQuestions ? formatRequestUserInputSummary(requestUserInputQuestions) : ""),
      [requestUserInputQuestions]
    );
    const contentText = React.useMemo(
      () => (msg.content ? unescapeLineBreaks(msg.content) : ""),
      [msg.content]
    );
    const contentShouldDefaultOpen = React.useMemo(
      () => shouldAutoExpandToolContent(contentText),
      [contentText]
    );
    const inputPayload = React.useMemo(() => normalizeToolPayload(msg.raw_input), [msg.raw_input]);
    const outputPayloadSource = React.useMemo(
      () => selectToolCallOutputForDisplay(msg.title, msg.raw_output),
      [msg.title, msg.raw_output]
    );
    const outputPayload = React.useMemo(
      () => normalizeToolPayload(outputPayloadSource),
      [outputPayloadSource]
    );
    const inputPreview = React.useMemo(
      () => summarizeToolPayload(inputPayload, TOOL_PAYLOAD_PREVIEW_LIMIT),
      [inputPayload]
    );
    const outputPreview = React.useMemo(
      () => summarizeToolPayload(outputPayload, TOOL_PAYLOAD_PREVIEW_LIMIT),
      [outputPayload]
    );
    const effectiveStatus = resolveEffectiveToolCallStatus(msg.status, runStatus);
    const statusLabel = formatToolCallStatus(effectiveStatus);
    const durationLabel = React.useMemo(
      () => formatToolCallDurationLabel(msg.raw_output),
      [msg.raw_output]
    );
    const statusMark = getToolCallStatusMark(effectiveStatus);
    const terminalActivityPreview = React.useMemo(() => {
      const activities = msg.terminal_activities;
      const last =
        activities && activities.length > 0
          ? activities[activities.length - 1]
          : undefined;
      return last ? formatTerminalActivityLabel(last) : "";
    }, [msg.terminal_activities]);

    React.useEffect(() => {
      setOpen((prevOpen) => deriveToolCallOpenState(prevOpen, wasLiveRef.current, isLive));
      wasLiveRef.current = isLive;
    }, [isLive]);
    React.useEffect(() => {
      if (autoCollapse && !wasAutoCollapseRef.current) {
        setOpen(false);
      }
      wasAutoCollapseRef.current = autoCollapse;
    }, [autoCollapse]);
    useAutoCollapseToolFoldWhenOutOfView({
      detailsRef,
      enabled: !grouped,
      onCollapse: handleAutoCollapse,
    });

    const title = grouped
      ? `${indexLabel ? `${indexLabel} ` : ""}${msg.title || "Tool Call"}`
      : `Tool Call${msg.title ? `: ${msg.title}` : ""}`;
    const effectiveHint = requestUserInputSummary || callHint;
    const hasRequestUserInputCard =
      requestUserInputQuestions != null && requestUserInputQuestions.length > 0;
    const showPendingRequestUserInputCard = isLive && hasRequestUserInputCard;
    const showResolvedRequestUserInputCard = !isLive && hasRequestUserInputCard;

    return (
      <div
        className={
          grouped
            ? "acp-bubble tool_call acp-tool-group-entry my-1"
            : `acp-bubble tool_call tool-call-enter ${ACP_TOOL_ROW_CLASS}`
        }
      >
        <details
          className={`acp-tool-fold${grouped ? " acp-tool-fold-nested" : ""} ${ACP_TOOL_CARD_CLASS} ${grouped ? ACP_TOOL_CARD_NESTED_CLASS : ""}`}
          ref={detailsRef}
          open={open}
          onToggle={(event) => {
            setOpen(event.currentTarget.open);
          }}
        >
          <summary className={ACP_TOOL_SUMMARY_CLASS}>
            <span className={`${ACP_TOOL_TITLE_CLASS} flex items-start gap-2`}>
              {statusMark ? (
                <span
                  className={`acp-tool-status-mark tone-${statusMark.tone} mt-1 inline-flex h-2 w-2 rounded-full ${statusMark.tone === "success" ? "bg-emerald-500" : "bg-rose-500"}`}
                  title={statusMark.label}
                  aria-label={statusMark.label}
                >
                  <span className="acp-tool-status-dot" />
                </span>
              ) : null}
              <span className="min-w-0 flex-1">
                {title}
                {effectiveHint ? (
                  <span className="ml-2 font-normal text-notion-text-muted">· {effectiveHint}</span>
                ) : (
                  ""
                )}
              </span>
            </span>
            {msg.status ? (
              <span className={`${ACP_TOOL_STATUS_CLASS} ${ACP_TOOL_STATUS_SINGLE_DEFAULT_CLASS}`}>
                {statusLabel}
                {durationLabel ? ` · ${durationLabel}` : ""}
              </span>
            ) : null}
          </summary>
          <div className="px-2 pb-2">
            {showPendingRequestUserInputCard && requestUserInputQuestions ? (
              <RequestUserInputCard
                toolCallId={msg.id}
                questions={requestUserInputQuestions}
                canSubmit={typeof onSubmitRequestUserInput === "function"}
                onSubmitRequestUserInput={onSubmitRequestUserInput}
              />
            ) : null}
            {showResolvedRequestUserInputCard && requestUserInputQuestions ? (
              <RequestUserInputResultCard
                questions={requestUserInputQuestions}
                response={requestUserInputResponse}
                statusLabel={statusLabel}
              />
            ) : null}
            {!hasRequestUserInputCard && msg.content ? (
              <FoldSection
                label="Content"
                preview={formatConversationPreview(contentText, 88)}
                defaultOpen={contentShouldDefaultOpen}
                lazyRender={true}
              >
                <ToolTextContent text={contentText} tone="terminal" />
              </FoldSection>
            ) : null}
            {!hasRequestUserInputCard && hasToolPayload(inputPayload) ? (
              <FoldSection
                label="Input"
                preview={inputPreview}
                defaultOpen={false}
                parentOpen={open}
                lazyRender={true}
              >
                <ToolPayloadView payload={inputPayload} />
              </FoldSection>
            ) : null}
            {!hasRequestUserInputCard && hasToolPayload(outputPayload) ? (
              <FoldSection
                label="Output"
                preview={outputPreview}
                defaultOpen={false}
                parentOpen={open}
                lazyRender={true}
              >
                <ToolPayloadView payload={outputPayload} />
              </FoldSection>
            ) : null}
            {msg.id ? (
              <FoldSection
                label="Detailed"
                preview={`call_id=${formatConversationPreview(msg.id, 40)}`}
                defaultOpen={false}
                parentOpen={open}
                lazyRender={false}
              >
                <ToolCallDetailsView details={extractToolCallDetails(msg.id, msg.raw_output, msg.title)} />
              </FoldSection>
            ) : null}
            {msg.terminal_activities && msg.terminal_activities.length > 0 ? (
              <FoldSection
                label="Activity"
                preview={terminalActivityPreview}
                defaultOpen={false}
                lazyRender={true}
              >
                <div className="space-y-1 text-sm text-notion-text-muted">
                  {msg.terminal_activities.map((activity, index) => (
                    <div key={`${activity.kind}:${activity.command ?? ""}:${index}`}>
                      {formatTerminalActivityLabel(activity)}
                    </div>
                  ))}
                </div>
              </FoldSection>
            ) : null}
            {msg.terminal_output ? (
              <FoldSection
                label="Terminal"
                preview={formatConversationPreview(unescapeLineBreaks(msg.terminal_output), 88)}
                defaultOpen={false}
                lazyRender={true}
              >
                <TerminalOutputView text={unescapeLineBreaks(msg.terminal_output)} ansi={ansi} />
              </FoldSection>
            ) : null}
          </div>
        </details>
      </div>
    );
  },
  (prev, next) =>
    prev.msg === next.msg &&
    prev.ansi === next.ansi &&
    prev.runStatus === next.runStatus &&
    prev.autoCollapse === next.autoCollapse &&
    prev.grouped === next.grouped &&
    prev.indexLabel === next.indexLabel &&
    prev.onSubmitRequestUserInput === next.onSubmitRequestUserInput
);
