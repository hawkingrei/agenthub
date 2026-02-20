import React from "react";
import { AcpRawEvent } from "../acp";
import { AcpPermissionRecord } from "../api";

type DebugTab = "session" | "runtime" | "permissions" | "raw";

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
  currentMode: string | null;
  rawEvents: AcpRawEvent[];
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
  onAcpSetMode: () => void;
  onAcpSetModel: () => void;
  onAcpSetConfig: () => void;
  onAcpCancel: () => void;
  onAcpClearSession: () => void;
  onJumpToPermissionHistory: (permission: AcpPermissionRecord) => void;
  runtimeMetrics: AcpRuntimeMetrics;
  initialTab?: DebugTab;
};

const COPIED_STATE_RESET_DELAY_MS = 1600;

export function AcpDebug({
  currentMode,
  rawEvents,
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
  onAcpSetMode,
  onAcpSetModel,
  onAcpSetConfig,
  onAcpCancel,
  onAcpClearSession,
  onJumpToPermissionHistory,
  runtimeMetrics,
  initialTab,
}: AcpDebugProps) {
  const [tab, setTab] = React.useState<DebugTab>(initialTab ?? "session");
  const [copiedPermissionId, setCopiedPermissionId] = React.useState<string | null>(null);
  const copiedResetTimerRef = React.useRef<number | null>(null);
  const rawRef = React.useRef<HTMLUListElement | null>(null);
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
    `tab rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm ${
      isActive
        ? "active bg-slate-900 text-white shadow-sm"
        : "text-slate-600 hover:bg-white hover:text-slate-900"
    }`;
  const debugInputClassName =
    "w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 shadow-sm outline-none transition focus:border-slate-400 focus:ring-2 focus:ring-slate-300";
  const debugSecondaryButtonClassName =
    "inline-flex items-center justify-center rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 transition hover:border-slate-400 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60";
  const debugPrimaryButtonClassName =
    "inline-flex items-center justify-center rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60";

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
    <div className="acp-debug flex min-h-0 flex-1 flex-col gap-3 p-3 sm:p-4">
      <div className="acp-debug-tabs flex flex-wrap gap-2 rounded-lg border border-slate-200 bg-slate-50 p-1">
        <button
          className={debugTabClassName(tab === "session")}
          onClick={() => setTab("session")}
        >
          Session Controls
        </button>
        <button
          className={debugTabClassName(tab === "runtime")}
          onClick={() => setTab("runtime")}
        >
          Runtime
        </button>
        <button
          className={debugTabClassName(tab === "permissions")}
          onClick={() => setTab("permissions")}
        >
          Permissions
        </button>
        <button
          className={debugTabClassName(tab === "raw")}
          onClick={() => setTab("raw")}
        >
          Raw Events
        </button>
      </div>
      {tab === "session" && (
        <div className="acp-controls space-y-3 rounded-xl border border-slate-200 bg-white p-3 shadow-sm sm:p-4">
          <h4 className="text-sm font-semibold text-slate-900 sm:text-base">Session Controls</h4>
          <div className="acp-control-meta text-sm text-slate-600">
            Current mode: {currentMode ?? "unknown"}
          </div>
          <div className="form-row flex flex-col gap-2 sm:flex-row">
            <input
              className={debugInputClassName}
              placeholder="Mode ID"
              value={acpModeId}
              onChange={(e) => onAcpModeIdChange(e.target.value)}
            />
            <button className={debugSecondaryButtonClassName} onClick={onAcpSetMode} disabled={!canControlAcp}>
              Set Mode
            </button>
          </div>
          <div className="form-row flex flex-col gap-2 sm:flex-row">
            <input
              className={debugInputClassName}
              placeholder="Model ID"
              value={acpModelId}
              onChange={(e) => onAcpModelIdChange(e.target.value)}
            />
            <button className={debugSecondaryButtonClassName} onClick={onAcpSetModel} disabled={!canControlAcp}>
              Set Model
            </button>
          </div>
          <div className="form-row grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
            <input
              className={debugInputClassName}
              placeholder="Config ID"
              value={acpConfigId}
              onChange={(e) => onAcpConfigIdChange(e.target.value)}
            />
            <input
              className={debugInputClassName}
              placeholder="Config Value ID"
              value={acpConfigValue}
              onChange={(e) => onAcpConfigValueChange(e.target.value)}
            />
            <button className={debugSecondaryButtonClassName} onClick={onAcpSetConfig} disabled={!canControlAcp}>
              Set Config
            </button>
          </div>
          <div className="form-row flex flex-wrap gap-2">
            <button className={debugPrimaryButtonClassName} onClick={onAcpCancel} disabled={!canControlAcp}>
              Cancel Run
            </button>
            <button className={debugSecondaryButtonClassName} onClick={onAcpClearSession}>
              Clear Session
            </button>
          </div>
        </div>
      )}
      {tab === "runtime" && (
        <div className="acp-runtime space-y-3 rounded-xl border border-slate-200 bg-white p-3 shadow-sm sm:p-4">
          <h4 className="text-sm font-semibold text-slate-900 sm:text-base">Runtime Metrics</h4>
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
        <div className="acp-permissions space-y-3 rounded-xl border border-slate-200 bg-white p-3 shadow-sm sm:p-4">
          <h4>Permissions</h4>
          {acpPermissionHistory.length === 0 && (
            <div className="empty rounded-lg border border-dashed border-slate-300 bg-slate-50 px-3 py-5 text-sm text-slate-500">
              No permissions yet.
            </div>
          )}
          {acpPermissionHistory.map((permission) => {
            const toolCall = toPermissionToolCall(permission.tool_call);
            const copied = copiedPermissionId === permission.id;
            const canJump = Boolean(permission.tool_call_id?.trim());
            return (
              <div key={permission.id} className="acp-permission rounded-xl border border-slate-200 bg-slate-50/70 p-3">
                <div className="head flex items-start justify-between gap-2">
                  <button
                    className="acp-permission-toggle flex min-w-0 flex-1 flex-col items-start gap-1 rounded-lg border border-transparent px-2 py-1 text-left transition hover:border-slate-300 hover:bg-white disabled:cursor-not-allowed disabled:opacity-60"
                    type="button"
                    onClick={() => onJumpToPermissionHistory(permission)}
                    disabled={!canJump}
                  >
                    <span className="title">
                      {derivePermissionTitle(permission, toolCall)}
                    </span>
                    <span className="meta">{permission.status}</span>
                  </button>
                  <button
                    className={`acp-permission-copy ${debugSecondaryButtonClassName}`}
                    type="button"
                    onClick={() => void handleCopyPermission(permission)}
                  >
                    {copied ? "Copied" : "Copy"}
                  </button>
                </div>
                <div className="acp-permission-submeta mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-600">
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
                  <div className="acp-permission-options mono mt-2 rounded-lg border border-amber-200 bg-amber-50 px-2 py-1.5 text-xs text-amber-700">
                    no linked tool call in conversation
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
      {tab === "raw" && (
        <div className="acp-raw-wrapper space-y-3 rounded-xl border border-slate-200 bg-white p-3 shadow-sm sm:p-4">
          <h4 className="text-sm font-semibold text-slate-900 sm:text-base">Raw Events</h4>
          <ul className="acp-raw max-h-[420px] space-y-2 overflow-auto pr-1" ref={rawRef}>
            {rawEvents.map((evt, idx) => (
              <li key={`${evt.ts}-${idx}`} className="rounded-lg border border-slate-200 bg-slate-50 p-2">
                <div className="meta flex flex-wrap items-center gap-2 text-xs text-slate-500">
                  <span>{new Date(evt.ts * 1000).toLocaleTimeString()}</span>
                  <span className="mono">{evt.type}</span>
                </div>
                <pre className="acp-content mt-2 overflow-auto rounded-md border border-slate-200 bg-white p-2 text-xs leading-5 text-slate-700">
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
    <div className="acp-runtime-card rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
      <div className="acp-runtime-label text-xs font-medium uppercase tracking-wide text-slate-500">
        {label}
      </div>
      <div className="acp-runtime-value mt-1 text-sm font-semibold text-slate-900">{children}</div>
    </div>
  );
}

export type { AcpDebugProps, AcpRuntimeMetrics };
