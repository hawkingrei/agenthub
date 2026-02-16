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
};

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
}: AcpDebugProps) {
  const [tab, setTab] = React.useState<DebugTab>("session");
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
      }, 1600);
    } catch {
      setCopiedPermissionId(null);
    }
  }, []);

  return (
    <div className="acp-debug">
      <div className="acp-debug-tabs">
        <button
          className={tab === "session" ? "tab active" : "tab"}
          onClick={() => setTab("session")}
        >
          Session Controls
        </button>
        <button
          className={tab === "runtime" ? "tab active" : "tab"}
          onClick={() => setTab("runtime")}
        >
          Runtime
        </button>
        <button
          className={tab === "permissions" ? "tab active" : "tab"}
          onClick={() => setTab("permissions")}
        >
          Permissions
        </button>
        <button
          className={tab === "raw" ? "tab active" : "tab"}
          onClick={() => setTab("raw")}
        >
          Raw Events
        </button>
      </div>
      {tab === "session" && (
        <div className="acp-controls">
          <h4>Session Controls</h4>
          <div className="acp-control-meta">
            Current mode: {currentMode ?? "unknown"}
          </div>
          <div className="form-row">
            <input
              placeholder="Mode ID"
              value={acpModeId}
              onChange={(e) => onAcpModeIdChange(e.target.value)}
            />
            <button onClick={onAcpSetMode} disabled={!canControlAcp}>
              Set Mode
            </button>
          </div>
          <div className="form-row">
            <input
              placeholder="Model ID"
              value={acpModelId}
              onChange={(e) => onAcpModelIdChange(e.target.value)}
            />
            <button onClick={onAcpSetModel} disabled={!canControlAcp}>
              Set Model
            </button>
          </div>
          <div className="form-row">
            <input
              placeholder="Config ID"
              value={acpConfigId}
              onChange={(e) => onAcpConfigIdChange(e.target.value)}
            />
            <input
              placeholder="Config Value ID"
              value={acpConfigValue}
              onChange={(e) => onAcpConfigValueChange(e.target.value)}
            />
            <button onClick={onAcpSetConfig} disabled={!canControlAcp}>
              Set Config
            </button>
          </div>
          <div className="form-row">
            <button onClick={onAcpCancel} disabled={!canControlAcp}>
              Cancel Run
            </button>
            <button onClick={onAcpClearSession}>Clear Session</button>
          </div>
        </div>
      )}
      {tab === "runtime" && (
        <div className="acp-runtime">
          <h4>Runtime Metrics</h4>
          <div className="acp-runtime-grid">
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
        <div className="acp-permissions">
          <h4>Permissions</h4>
          {acpPermissionHistory.length === 0 && (
            <div className="empty">No permissions yet.</div>
          )}
          {acpPermissionHistory.map((permission) => {
            const toolCall = toPermissionToolCall(permission.tool_call);
            const copied = copiedPermissionId === permission.id;
            const canJump = Boolean(permission.tool_call_id?.trim());
            return (
              <div key={permission.id} className="acp-permission">
                <div className="head">
                  <button
                    className="acp-permission-toggle"
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
                    className="acp-permission-copy"
                    type="button"
                    onClick={() => void handleCopyPermission(permission)}
                  >
                    {copied ? "Copied" : "Copy"}
                  </button>
                </div>
                <div className="acp-permission-submeta">
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
                  <div className="acp-permission-options mono">
                    no linked tool call in conversation
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
      {tab === "raw" && (
        <div className="acp-raw-wrapper">
          <h4>Raw Events</h4>
          <ul className="acp-raw" ref={rawRef}>
            {rawEvents.map((evt, idx) => (
              <li key={`${evt.ts}-${idx}`}>
                <div className="meta">
                  <span>{new Date(evt.ts * 1000).toLocaleTimeString()}</span>
                  <span className="mono">{evt.type}</span>
                </div>
                <pre className="acp-content">
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
    <div className="acp-runtime-card">
      <div className="acp-runtime-label">{label}</div>
      <div className="acp-runtime-value">{children}</div>
    </div>
  );
}

export type { AcpDebugProps, AcpRuntimeMetrics };
