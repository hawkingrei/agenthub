import React, { useState } from "react";
import { AuditRecord, DeviceRecord, SafePath, VapidInfo } from "../api";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";

type AdminProps = {
  auth: AuthState;
  error: string | null;
  setError: (value: string | null) => void;
  safePaths: SafePath[];
  selectedSafePaths: Set<string>;
  onToggleSafePath: (path: string) => void;
  onToggleAllSafePaths: () => void;
  onDeleteSelectedSafePaths: () => void;
  devices: DeviceRecord[];
  audits: AuditRecord[];
  vapidInfo: VapidInfo | null;
  onRotateVapid: () => void;
  onAddSafePath: () => void;
  onDeleteSafePath: (path: string) => void;
  onRevokeDevice: (id: string) => void;
  onCreateJoin: () => void;
  joinQr: string | null;
  joinToken: string | null;
  joinPin: string | null;
  safePathInput: string;
  setSafePathInput: (value: string) => void;
};

export function AdminPage(props: AdminProps) {
  const [tab, setTab] = useState<
    "safe" | "devices" | "audits" | "join" | "vapid"
  >("safe");
  return (
    <div className="app">
      <header>
        <h1>AgentHub Admin</h1>
        <div className="session">
          <a className="icon-button" href="/" title="Back" aria-label="Back">
            <i className="bi bi-arrow-left" aria-hidden="true" />
          </a>
          <span>{props.auth.username}</span>
        </div>
      </header>

      {props.error && (
        <ErrorBanner message={props.error} onClose={() => props.setError(null)} />
      )}

      <section className="admin">
        <div className="toolbar">
          <h2>Admin</h2>
          <button onClick={props.onCreateJoin}>Create Join QR</button>
        </div>
        <div className="tab-bar">
          <button
            className={tab === "safe" ? "tab active" : "tab"}
            onClick={() => setTab("safe")}
          >
            Safe Paths
          </button>
          <button
            className={tab === "devices" ? "tab active" : "tab"}
            onClick={() => setTab("devices")}
          >
            Devices
          </button>
          <button
            className={tab === "audits" ? "tab active" : "tab"}
            onClick={() => setTab("audits")}
          >
            Login Audits
          </button>
          <button
            className={tab === "join" ? "tab active" : "tab"}
            onClick={() => setTab("join")}
          >
            Join Device
          </button>
          <button
            className={tab === "vapid" ? "tab active" : "tab"}
            onClick={() => setTab("vapid")}
          >
            VAPID Keys
          </button>
        </div>

        <div className="admin-panel">
          {tab === "safe" && (
            <div className="card">
              <h3>Safe Paths</h3>
              <div className="form-row">
                <input
                  placeholder="Add safe path"
                  value={props.safePathInput}
                  onChange={(e) => props.setSafePathInput(e.target.value)}
                />
                <button onClick={props.onAddSafePath}>Add Path</button>
              </div>
              <div className="form-row">
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={
                      props.safePaths.length > 0 &&
                      props.safePaths.every((p) =>
                        props.selectedSafePaths.has(p.path)
                      )
                    }
                    onChange={props.onToggleAllSafePaths}
                  />
                  Select All
                </label>
                <button onClick={props.onDeleteSelectedSafePaths}>
                  Delete Selected
                </button>
              </div>
              <ul>
                {props.safePaths.map((p) => (
                  <li key={p.path}>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={props.selectedSafePaths.has(p.path)}
                        onChange={() => props.onToggleSafePath(p.path)}
                      />
                    </label>
                    <span>{p.path}</span>
                    <button onClick={() => props.onDeleteSafePath(p.path)}>
                      Delete
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "devices" && (
            <div className="card">
              <h3>Devices</h3>
              <ul>
                {props.devices.map((device) => (
                  <li key={device.id}>
                    <span>
                      {device.name} - {device.status}
                    </span>
                    {device.status === "active" && (
                      <button onClick={() => props.onRevokeDevice(device.id)}>
                        Revoke
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "audits" && (
            <div className="card">
              <h3>Login Audits</h3>
              <ul>
                {props.audits.map((audit) => (
                  <li key={audit.id}>
                    <span>
                      {new Date(audit.ts * 1000).toLocaleString()} -
                      {` ${audit.event}`}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "join" && (
            <div className="card join-card">
              <h3>Join Device</h3>
              {props.joinQr && <img src={props.joinQr} alt="join qr" />}
              {props.joinToken && <p>Token: {props.joinToken}</p>}
              {props.joinPin && <p>PIN: {props.joinPin}</p>}
            </div>
          )}
          {tab === "vapid" && (
            <div className="card">
              <h3>VAPID Keys</h3>
              {props.vapidInfo ? (
                <div className="kv-list">
                  <div className="kv-row">
                    <span className="label">Subject</span>
                    <span className="value">{props.vapidInfo.subject}</span>
                  </div>
                  <div className="kv-row">
                    <span className="label">Public Key</span>
                    <span className="value mono">
                      {props.vapidInfo.public_key}
                    </span>
                  </div>
                  <div className="kv-row">
                    <span className="label">Keys Path</span>
                    <span className="value mono">
                      {props.vapidInfo.keys_path}
                    </span>
                  </div>
                </div>
              ) : (
                <p>VAPID keys not loaded.</p>
              )}
              {/* TODO: add copy button and rotate confirmation. */}
              <div className="form-row">
                <button onClick={props.onRotateVapid}>Rotate Keys</button>
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
