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
  runtimeMetrics,
}: AcpDebugProps) {
  const [tab, setTab] = React.useState<DebugTab>("session");
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
          {acpPermissionHistory.map((perm) => (
            <div key={perm.id} className="acp-permission">
              <div className="head">
                <div className="title">{perm.permission}</div>
                <div className="meta">{perm.status}</div>
              </div>
            </div>
          ))}
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
