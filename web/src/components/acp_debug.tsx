import React from "react";
import { AcpRawEvent } from "../acp";
import { AcpPermissionRecord } from "../api";

type DebugTab = "session" | "permissions" | "raw";

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
}: AcpDebugProps) {
  const [tab, setTab] = React.useState<DebugTab>("session");
  const rawRef = React.useRef<HTMLUListElement | null>(null);

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

export type { AcpDebugProps };
