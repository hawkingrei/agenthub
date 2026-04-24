import { UnstyledButton } from "@mantine/core";
import React from "react";
import { AcpConfigOption, AcpRawEvent } from "../acp";
import { AcpPermissionRecord } from "../api";
import { shouldDisplayPermissionRecord } from "../app_permission_polling";
import { OutputLine } from "../output_cache";
import { ActionButton, ToolbarRow, cx } from "../ui/primitives";
import {
  ACP_DEBUG_EMPTY_CLASS,
  ACP_DEBUG_PERMISSION_SUBMETA_CLASS,
  ACP_DEBUG_PERMISSION_TOGGLE_CLASS,
  ACP_DEBUG_PERMISSION_WARNING_CLASS,
  ACP_DEBUG_RAW_PRE_CLASS,
  ACP_DEBUG_ROOT_CLASS,
  ACP_DEBUG_SECTION_CLASS,
  ACP_DEBUG_TABS_CLASS,
  ACP_TAB_BUTTON_BASE_CLASS,
  ACP_TAB_BUTTON_ACTIVE_CLASS,
  ACP_TAB_BUTTON_IDLE_CLASS,
} from "../ui/tailwind_classes";

type DebugTab = "terminal" | "session" | "runtime" | "permissions" | "raw";
type DebugTerminalFilter = "all" | "stderr" | "system";

type AcpRuntimeMetrics = {
  totalConversationItems: number;
  sourceConversationItems: number;
  renderedConversationItems: number;
  pendingConversationItems: number;
  virtualizedConversation: boolean;
  stickToBottom: boolean;
  averageConversationHeight: number;
  rawEventCount: number;
  toolCallCount: number;
  messageCount: number;
  markdownCacheHits: number;
  markdownCacheMisses: number;
  ansiCacheHits: number;
  ansiCacheMisses: number;
  payloadParses: number;
  payloadParseFailures: number;
};

type AcpDebugProps = {
  terminalOutputs: OutputLine[];
  ansi: (input: string) => string;
  terminalRef?: React.RefObject<HTMLDivElement>;
  onTerminalScroll?: () => void;
  showTerminalJump?: boolean;
  onJumpToTerminalBottom?: () => void;
  currentMode: string | null;
  rawEvents: AcpRawEvent[];
  configOptions: AcpConfigOption[];
  acpPermissionHistory: AcpPermissionRecord[];
  acpModeId: string;
  acpModelId: string;
  acpConfigId: string;
  acpConfigValue: string;
  onAcpModeIdChange: (value: string) => void;
  onAcpModelIdChange: (value: string) => void;
  onAcpConfigIdChange: (value: string) => void;
  onAcpConfigValueChange: (value: string) => void;
  canControlAcp: boolean;
  canSetMode?: boolean;
  canSetModel?: boolean;
  canSetConfig?: boolean;
  canCancelRun?: boolean;
  canClearSession?: boolean;
  onAcpSetMode: (value: string) => void;
  onAcpSetModel: (value: string) => void;
  onAcpSetConfig: () => void;
  onAcpCancel: () => void;
  onAcpClearSession: () => void;
  acpClearSessionLabel?: string;
  onJumpToPermissionHistory: (permission: AcpPermissionRecord) => void;
  runtimeMetrics: AcpRuntimeMetrics;
  initialTab?: DebugTab;
};

const COPIED_STATE_RESET_DELAY_MS = 1600;
const DEBUG_TERMINAL_CONTAINER_CLASS =
  "terminal flex-1 min-h-0 overflow-auto rounded-lg border border-notion-border bg-brand-primary p-3 font-mono text-[12px] leading-relaxed text-white shadow-inner";
const DEBUG_TERMINAL_LINE_BASE_CLASS = "line m-0 min-w-max whitespace-pre [tab-size:2]";
const DEBUG_TERMINAL_STDOUT_CLASS = "text-white";
const DEBUG_TERMINAL_STDERR_CLASS = "text-red-300";
const DEBUG_TERMINAL_SYSTEM_CLASS = "text-cyan-300";

function renderConfigSelectOptions(
  placeholder: string,
  options: Array<{ valueId: string; label: string }>
): React.ReactNode {
  return (
    <>
      <option value="">{placeholder}</option>
      {options.map((option) => (
        <option key={option.valueId} value={option.valueId}>
          {option.label}
        </option>
      ))}
    </>
  );
}

function AcpDebugView({
  terminalOutputs,
  ansi,
  terminalRef,
  onTerminalScroll,
  showTerminalJump = false,
  onJumpToTerminalBottom,
  currentMode,
  rawEvents,
  configOptions,
  acpPermissionHistory,
  acpModeId,
  acpModelId,
  acpConfigId,
  acpConfigValue,
  onAcpModeIdChange,
  onAcpModelIdChange,
  onAcpConfigIdChange,
  onAcpConfigValueChange,
  canControlAcp,
  canSetMode,
  canSetModel,
  canSetConfig,
  canCancelRun,
  canClearSession,
  onAcpSetMode,
  onAcpSetModel,
  onAcpSetConfig,
  onAcpCancel,
  onAcpClearSession,
  acpClearSessionLabel,
  onJumpToPermissionHistory,
  runtimeMetrics,
  initialTab,
}: AcpDebugProps) {
  const [tab, setTab] = React.useState<DebugTab>(initialTab ?? "session");
  const [terminalFilter, setTerminalFilter] = React.useState<DebugTerminalFilter>("all");
  const [copiedPermissionId, setCopiedPermissionId] = React.useState<string | null>(null);
  const copiedResetTimerRef = React.useRef<number | null>(null);
  const rawRef = React.useRef<HTMLUListElement | null>(null);
  const visiblePermissionHistory = acpPermissionHistory.filter(shouldDisplayPermissionRecord);
  const markdownTotal = runtimeMetrics.markdownCacheHits + runtimeMetrics.markdownCacheMisses;
  const ansiTotal = runtimeMetrics.ansiCacheHits + runtimeMetrics.ansiCacheMisses;
  const payloadParseSuccess = Math.max(
    0,
    runtimeMetrics.payloadParses - runtimeMetrics.payloadParseFailures
  );
  const markdownHitRate = markdownTotal > 0
    ? Math.round((runtimeMetrics.markdownCacheHits / markdownTotal) * 100)
    : 0;
  const ansiHitRate = ansiTotal > 0
    ? Math.round((runtimeMetrics.ansiCacheHits / ansiTotal) * 100)
    : 0;
  const debugTabClassName = (isActive: boolean) =>
    cx(
      "acp-debug-tab",
      ACP_TAB_BUTTON_BASE_CLASS,
      isActive ? ACP_TAB_BUTTON_ACTIVE_CLASS : ACP_TAB_BUTTON_IDLE_CLASS
    );
  const debugInputClassName =
    "w-full rounded-md border border-notion-border bg-white px-3 py-1.5 text-[14px] text-notion-text outline-none transition focus:border-notion-accent focus:ring-2 focus:ring-notion-accent/10";
  const modeConfigOption = React.useMemo(
    () => configOptions.find((option) => option.id === "mode") ?? null,
    [configOptions]
  );
  const modelConfigOption = React.useMemo(
    () => configOptions.find((option) => option.id === "model") ?? null,
    [configOptions]
  );
  const resolvedModeId = acpModeId || modeConfigOption?.currentValueId || "";
  const resolvedModelId = acpModelId || modelConfigOption?.currentValueId || "";
  const canApplyMode = canSetMode ?? canControlAcp;
  const canApplyModel = canSetModel ?? canControlAcp;
  const canApplyConfig = canSetConfig ?? canControlAcp;
  const canCancel = canCancelRun ?? canControlAcp;
  const canClear = canClearSession ?? true;
  const terminalCounts = React.useMemo(() => {
    let stdout = 0;
    let stderr = 0;
    let system = 0;
    for (const line of terminalOutputs) {
      if (line.stream === "stderr") {
        stderr += 1;
      } else if (line.stream === "system") {
        system += 1;
      } else {
        stdout += 1;
      }
    }
    return { stdout, stderr, system, total: terminalOutputs.length };
  }, [terminalOutputs]);
  const filteredTerminalOutputs = React.useMemo(() => {
    if (terminalFilter === "all") return terminalOutputs;
    return terminalOutputs.filter((line) => line.stream === terminalFilter);
  }, [terminalFilter, terminalOutputs]);

  React.useEffect(() => {
    if (tab !== "raw") return;
    const el = rawRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [tab, rawEvents.length]);
  React.useEffect(() => {
    return () => {
      if (copiedResetTimerRef.current != null) {
        window.clearTimeout(copiedResetTimerRef.current);
      }
    };
  }, []);

  const handleCopyPermission = React.useCallback(async (permission: AcpPermissionRecord) => {
    const content = buildPermissionCopyText(permission);
    try {
      await copyTextToClipboard(content);
      setCopiedPermissionId(permission.id);
      if (copiedResetTimerRef.current != null) {
        window.clearTimeout(copiedResetTimerRef.current);
      }
      copiedResetTimerRef.current = window.setTimeout(() => {
        setCopiedPermissionId((prev) => (prev === permission.id ? null : prev));
        copiedResetTimerRef.current = null;
      }, COPIED_STATE_RESET_DELAY_MS);
    } catch {
      setCopiedPermissionId(null);
    }
  }, []);

  return (
    <div className={ACP_DEBUG_ROOT_CLASS}>
      <div className={ACP_DEBUG_TABS_CLASS}>
        <UnstyledButton
          className={debugTabClassName(tab === "terminal")}
          type="button"
          onClick={() => setTab("terminal")}
        >
          Terminal
        </UnstyledButton>
        <UnstyledButton
          className={debugTabClassName(tab === "session")}
          type="button"
          onClick={() => setTab("session")}
        >
          Session Controls
        </UnstyledButton>
        <UnstyledButton
          className={debugTabClassName(tab === "runtime")}
          type="button"
          onClick={() => setTab("runtime")}
        >
          Runtime
        </UnstyledButton>
        <UnstyledButton
          className={debugTabClassName(tab === "permissions")}
          type="button"
          onClick={() => setTab("permissions")}
        >
          Permissions
        </UnstyledButton>
        <UnstyledButton
          className={debugTabClassName(tab === "raw")}
          type="button"
          onClick={() => setTab("raw")}
        >
          Raw Events
        </UnstyledButton>
      </div>
      {tab === "terminal" && (
        <div className={`acp-terminal-wrapper ${ACP_DEBUG_SECTION_CLASS}`}>
          <ToolbarRow className="items-start">
            <div className="min-w-0">
              <h4 className="text-sm font-bold text-notion-text uppercase tracking-widest">Terminal</h4>
              <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-notion-text-muted">
                <span>{terminalCounts.stdout} stdout</span>
                <span>{terminalCounts.stderr} stderr</span>
                <span>{terminalCounts.system} system</span>
              </div>
            </div>
            <div className="flex flex-wrap items-center justify-end gap-2">
              <div className="flex flex-wrap items-center gap-1 bg-notion-sidebar p-1 rounded-md border border-notion-border">
                <UnstyledButton
                  className={debugTabClassName(terminalFilter === "all")}
                  type="button"
                  onClick={() => setTerminalFilter("all")}
                >
                  All
                </UnstyledButton>
                <UnstyledButton
                  className={debugTabClassName(terminalFilter === "stderr")}
                  type="button"
                  onClick={() => setTerminalFilter("stderr")}
                >
                  Stderr
                </UnstyledButton>
                <UnstyledButton
                  className={debugTabClassName(terminalFilter === "system")}
                  type="button"
                  onClick={() => setTerminalFilter("system")}
                >
                  System
                </UnstyledButton>
              </div>
              {showTerminalJump ? (
                <ActionButton
                  className="whitespace-nowrap"
                  size="sm"
                  tone="secondary"
                  type="button"
                  onClick={onJumpToTerminalBottom}
                >
                  Jump to latest
                </ActionButton>
              ) : null}
            </div>
          </ToolbarRow>
          {terminalOutputs.length === 0 ? (
            <div className={ACP_DEBUG_EMPTY_CLASS}>No terminal output yet.</div>
          ) : filteredTerminalOutputs.length === 0 ? (
            <div className={ACP_DEBUG_EMPTY_CLASS}>
              No {terminalFilter} output in this session.
            </div>
          ) : (
            <div className="h-[420px] min-h-[220px]">
              <DebugTerminalOutput
                outputs={filteredTerminalOutputs}
                ansi={ansi}
                containerRef={terminalRef}
                onScroll={onTerminalScroll}
              />
            </div>
          )}
        </div>
      )}
      {tab === "session" && (
        <div className={`acp-controls ${ACP_DEBUG_SECTION_CLASS}`}>
          <h4 className="text-sm font-bold text-notion-text uppercase tracking-widest">Session Controls</h4>
          <div className="acp-control-meta text-[13px] text-notion-text-muted italic">
            Current mode: {currentMode ?? "unknown"}
          </div>
          <div className="form-row grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
            {modeConfigOption?.selectOptions.length ? (
              <select
                aria-label="ACP mode"
                className={debugInputClassName}
                name="acp-mode"
                value={resolvedModeId}
                onChange={(e) => onAcpModeIdChange(e.currentTarget.value)}
              >
                {renderConfigSelectOptions(
                  "Select mode",
                  modeConfigOption.selectOptions
                )}
              </select>
            ) : (
              <input
                aria-label="ACP mode"
                className={debugInputClassName}
                name="acp-mode"
                placeholder="Mode ID"
                value={resolvedModeId}
                onChange={(e) => onAcpModeIdChange(e.currentTarget.value)}
              />
            )}
            <ActionButton
              className="whitespace-nowrap"
              size="sm"
              tone="secondary"
              onClick={() => onAcpSetMode(resolvedModeId)}
              disabled={!canApplyMode || !resolvedModeId}
            >
              Set Mode
            </ActionButton>
          </div>
          <div className="form-row grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
            {modelConfigOption?.selectOptions.length ? (
              <select
                aria-label="ACP model"
                className={debugInputClassName}
                name="acp-model"
                value={resolvedModelId}
                onChange={(e) => onAcpModelIdChange(e.currentTarget.value)}
              >
                {renderConfigSelectOptions(
                  "Select model",
                  modelConfigOption.selectOptions
                )}
              </select>
            ) : (
              <input
                aria-label="ACP model"
                className={debugInputClassName}
                name="acp-model"
                placeholder="Model ID"
                value={resolvedModelId}
                onChange={(e) => onAcpModelIdChange(e.currentTarget.value)}
              />
            )}
            <ActionButton
              className="whitespace-nowrap"
              size="sm"
              tone="secondary"
              onClick={() => onAcpSetModel(resolvedModelId)}
              disabled={!canApplyModel || !resolvedModelId}
            >
              Set Model
            </ActionButton>
          </div>
          <div className="form-row grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
            <input
              aria-label="ACP config ID"
              className={debugInputClassName}
              id="acp-config-id"
              name="acp-config-id"
              placeholder="Config ID"
              value={acpConfigId}
              onChange={(e) => onAcpConfigIdChange(e.currentTarget.value)}
            />
            <input
              aria-label="ACP config value"
              className={debugInputClassName}
              id="acp-config-value"
              name="acp-config-value"
              placeholder="Config Value ID"
              value={acpConfigValue}
              onChange={(e) => onAcpConfigValueChange(e.currentTarget.value)}
            />
            <ActionButton
              className="whitespace-nowrap"
              size="sm"
              tone="secondary"
              onClick={onAcpSetConfig}
              disabled={!canApplyConfig}
            >
              Set Config
            </ActionButton>
          </div>
          <div className="form-row flex flex-wrap gap-2">
            <ActionButton tone="primary" size="sm" onClick={onAcpCancel} disabled={!canCancel}>
              Cancel Run
            </ActionButton>
            <ActionButton tone="secondary" size="sm" onClick={onAcpClearSession} disabled={!canClear}>
              {acpClearSessionLabel ?? "Clear Session"}
            </ActionButton>
          </div>
        </div>
      )}
      {tab === "runtime" && (
        <div className={`acp-runtime ${ACP_DEBUG_SECTION_CLASS}`}>
          <h4 className="text-sm font-bold text-notion-text uppercase tracking-widest">Runtime Metrics</h4>
          <div className="acp-runtime-grid grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
            <RuntimeMetricCard label="Conversation (total/source/rendered)">
              {runtimeMetrics.totalConversationItems}
              {" / "}
              {runtimeMetrics.sourceConversationItems}
              {" / "}
              {runtimeMetrics.renderedConversationItems}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Pending Items">
              {runtimeMetrics.pendingConversationItems}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Virtualized">
              {runtimeMetrics.virtualizedConversation ? "yes" : "no"}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Stick To Bottom">
              {runtimeMetrics.stickToBottom ? "yes" : "no"}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Average Row Height">
              {runtimeMetrics.averageConversationHeight}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Raw / Tool / Message">
              {runtimeMetrics.rawEventCount}
              {" / "}
              {runtimeMetrics.toolCallCount}
              {" / "}
              {runtimeMetrics.messageCount}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Markdown Cache">
              {runtimeMetrics.markdownCacheHits}
              {" / "}
              {runtimeMetrics.markdownCacheMisses}
              {" ("}
              {markdownHitRate}
              {"% hit)"}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="ANSI Cache">
              {runtimeMetrics.ansiCacheHits}
              {" / "}
              {runtimeMetrics.ansiCacheMisses}
              {" ("}
              {ansiHitRate}
              {"% hit)"}
            </RuntimeMetricCard>
            <RuntimeMetricCard label="Payload JSON Parse">
              {payloadParseSuccess}
              {" success / "}
              {runtimeMetrics.payloadParseFailures}
              {" fail"}
            </RuntimeMetricCard>
          </div>
        </div>
      )}
      {tab === "permissions" && (
        <div className={`acp-permissions ${ACP_DEBUG_SECTION_CLASS}`}>
          <h4 className="text-sm font-bold text-notion-text uppercase tracking-widest">Permissions</h4>
          {visiblePermissionHistory.length === 0 && (
            <div className={ACP_DEBUG_EMPTY_CLASS}>
              No permissions yet.
            </div>
          )}
          {visiblePermissionHistory.map((permission) => {
            const toolCall = toPermissionToolCall(permission.tool_call);
            const copied = copiedPermissionId === permission.id;
            const canJump = Boolean(permission.tool_call_id?.trim());
            return (
              <div key={permission.id} className="acp-permission rounded-lg border border-notion-border bg-notion-sidebar/30 p-3">
                <div className="head flex items-start justify-between gap-2">
                  <UnstyledButton
                    className={ACP_DEBUG_PERMISSION_TOGGLE_CLASS}
                    type="button"
                    onClick={() => onJumpToPermissionHistory(permission)}
                    disabled={!canJump}
                  >
                    <span className="text-[14px] font-bold text-notion-text">
                      {derivePermissionTitle(permission, toolCall)}
                    </span>
                    <span className="text-[11px] font-bold uppercase tracking-wider text-notion-text-muted">{permission.status}</span>
                  </UnstyledButton>
                  <ActionButton
                    className="acp-permission-copy"
                    size="sm"
                    tone="secondary"
                    type="button"
                    onClick={() => void handleCopyPermission(permission)}
                  >
                    {copied ? "Copied" : "Copy"}
                  </ActionButton>
                </div>
                <div className={ACP_DEBUG_PERMISSION_SUBMETA_CLASS}>
                  <span className="mono">{permission.id}</span>
                  {permission.tool_call_id && (
                    <span className="mono">tool_call {permission.tool_call_id}</span>
                  )}
                  <span>created {formatPermissionTimestamp(permission.created_at)}</span>
                  {permission.responded_at != null && (
                    <span>responded {formatPermissionTimestamp(permission.responded_at)}</span>
                  )}
                </div>
                {!canJump && (
                  <div className={ACP_DEBUG_PERMISSION_WARNING_CLASS}>
                    no linked tool call in conversation
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
      {tab === "raw" && (
        <div className={`acp-raw-wrapper ${ACP_DEBUG_SECTION_CLASS}`}>
          <h4 className="text-sm font-bold text-notion-text uppercase tracking-widest">Raw Events</h4>
          <ul className="acp-raw max-h-[420px] space-y-2 overflow-auto pr-1" ref={rawRef}>
            {rawEvents.map((evt, idx) => (
              <li key={`${evt.ts}-${idx}`} className="rounded-lg border border-notion-border bg-white p-2">
                <div className="meta flex flex-wrap items-center gap-2 text-[11px] text-notion-text-muted font-bold uppercase">
                  <span>{new Date(evt.ts * 1000).toLocaleTimeString()}</span>
                  <span className="mono">{evt.type}</span>
                </div>
                <pre className={ACP_DEBUG_RAW_PRE_CLASS}>
                  {JSON.stringify(evt.payload, null, 2)}
                </pre>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export const AcpDebug = React.memo(AcpDebugView);
AcpDebug.displayName = "AcpDebug";

export type PermissionToolCall = {
  title?: string;
};

function toPermissionToolCall(toolCall: unknown): PermissionToolCall | null {
  if (!toolCall || typeof toolCall !== "object" || Array.isArray(toolCall)) {
    return null;
  }
  const candidate = toolCall as { title?: unknown };
  return {
    title: typeof candidate.title === "string" ? candidate.title : undefined,
  };
}

export function derivePermissionTitle(
  permission: AcpPermissionRecord,
  toolCall: PermissionToolCall | null
): string {
  if (toolCall?.title) return toolCall.title;
  if (permission.tool_call_id) return permission.tool_call_id;
  return "Permission Request";
}

export function buildPermissionCopyText(permission: AcpPermissionRecord): string {
  return JSON.stringify(
    {
      permission_id: permission.id,
      agent_id: permission.agent_id,
      session_id: permission.session_id,
      acp_session_id: permission.acp_session_id ?? null,
      tool_call_id: permission.tool_call_id ?? null,
      status: permission.status,
      selected_option_id: permission.selected_option_id ?? null,
      created_at: permission.created_at,
      responded_at: permission.responded_at ?? null,
      options: permission.options,
      tool_call: permission.tool_call ?? null,
    },
    null,
    2
  );
}

function formatPermissionTimestamp(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

function DebugTerminalOutput({
  outputs,
  ansi,
  containerRef,
  onScroll,
}: {
  outputs: OutputLine[];
  ansi: (input: string) => string;
  containerRef?: React.RefObject<HTMLDivElement>;
  onScroll?: () => void;
}) {
  return (
    <div
      className={DEBUG_TERMINAL_CONTAINER_CLASS}
      ref={containerRef}
      onScroll={onScroll}
    >
      {outputs.map((line) => {
        const toneClass =
          line.stream === "stderr"
            ? DEBUG_TERMINAL_STDERR_CLASS
            : line.stream === "system"
              ? DEBUG_TERMINAL_SYSTEM_CLASS
              : DEBUG_TERMINAL_STDOUT_CLASS;
        return (
          <pre
            key={`id-${line.event_id}`}
            className={`${DEBUG_TERMINAL_LINE_BASE_CLASS} ${line.stream} ${toneClass}`}
            dangerouslySetInnerHTML={{ __html: ansi(line.message) }}
          />
        );
      })}
    </div>
  );
}

async function copyTextToClipboard(text: string): Promise<void> {
  if (
    typeof navigator !== "undefined" &&
    navigator.clipboard &&
    typeof navigator.clipboard.writeText === "function"
  ) {
    await navigator.clipboard.writeText(text);
    return;
  }
  if (typeof document === "undefined") {
    throw new Error("clipboard unavailable");
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!ok) {
    throw new Error("clipboard write failed");
  }
}

function RuntimeMetricCard({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="acp-runtime-card rounded-lg border border-notion-border bg-notion-sidebar/30 px-3 py-2 shadow-sm">
      <div className="acp-runtime-label text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">
        {label}
      </div>
      <div className="acp-runtime-value mt-1 text-[14px] font-bold text-notion-text">{children}</div>
    </div>
  );
}

export type { AcpDebugProps, AcpRuntimeMetrics };
