import React, { useState } from "react";
import { Switch } from "@mantine/core";
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
  developerMode: boolean;
  onDeveloperModeChange: (value: boolean) => void;
  passkeyEnabled: boolean | null;
  onPasskeyEnabledChange: (value: boolean) => void;
};

const ADMIN_APP_CLASS =
  "app min-h-screen bg-gradient-to-b from-slate-100/80 to-white text-slate-900";
const ADMIN_HEADER_CLASS =
  "flex items-center justify-between border-b border-slate-200/80 bg-white/90 px-5 py-3 backdrop-blur";
const ADMIN_TITLE_CLASS = "text-lg font-semibold tracking-tight text-slate-900";
const ADMIN_SESSION_CLASS = "session flex items-center gap-2 text-sm text-slate-700";
const ADMIN_SECTION_CLASS =
  "admin mx-auto flex w-full max-w-5xl flex-col gap-4 px-4 py-5";
const ADMIN_TOOLBAR_CLASS =
  "admin-toolbar flex flex-wrap items-center justify-between gap-3 rounded-xl border border-slate-200/80 bg-white/85 px-4 py-3 shadow-sm";
const ADMIN_TAB_BAR_CLASS =
  "admin-tab-bar flex flex-wrap gap-2 rounded-xl border border-slate-200/80 bg-white/80 p-2 shadow-sm";
const ADMIN_TAB_BUTTON_BASE_CLASS =
  "admin-tab-button inline-flex items-center rounded-md px-3 py-1.5 text-sm font-medium transition";
const ADMIN_TAB_BUTTON_ACTIVE_CLASS =
  "border border-slate-900 bg-slate-900 text-white shadow-sm";
const ADMIN_TAB_BUTTON_IDLE_CLASS =
  "border border-transparent bg-white text-slate-700 hover:border-slate-300 hover:text-slate-900";
const ADMIN_CARD_CLASS =
  "admin-card rounded-xl border border-slate-200/80 bg-white/90 p-4 shadow-sm";
const ADMIN_CARD_TITLE_CLASS = "mb-3 text-base font-semibold text-slate-900";
const ADMIN_FORM_ROW_CLASS = "form-row mb-3 flex flex-wrap items-center gap-2";
const ADMIN_INPUT_CLASS =
  "min-w-[16rem] flex-1 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-slate-500 focus:ring-2 focus:ring-slate-200";
const ADMIN_PRIMARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg bg-slate-900 px-3 py-2 text-sm font-medium text-white transition hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60";
const ADMIN_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-800 transition hover:border-slate-500";
const ADMIN_DANGER_BUTTON_CLASS =
  "inline-flex items-center justify-center rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-medium text-rose-700 transition hover:border-rose-300 hover:bg-rose-100";
const ADMIN_LIST_CLASS = "space-y-2";
const ADMIN_LIST_ITEM_CLASS =
  "flex flex-wrap items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2";
const ADMIN_MUTED_TEXT_CLASS = "text-sm text-slate-600";
const ADMIN_QR_CLASS = "mt-2 max-w-xs rounded-lg border border-slate-200 bg-white p-2";
const ADMIN_KV_LIST_CLASS = "kv-list space-y-2";
const ADMIN_KV_ROW_CLASS =
  "kv-row grid gap-1 sm:grid-cols-[9rem_1fr] sm:items-start";
const ADMIN_LABEL_CLASS = "label text-sm font-medium text-slate-700";
const ADMIN_VALUE_CLASS = "value break-all text-sm text-slate-900";
const ADMIN_EMPTY_TEXT_CLASS = "text-sm text-slate-500";

export function AdminPage(props: AdminProps) {
  const [tab, setTab] = useState<
    "safe" | "devices" | "audits" | "join" | "vapid" | "ui" | "system"
  >("safe");
  return (
    <div className={ADMIN_APP_CLASS}>
      <header className={ADMIN_HEADER_CLASS}>
        <h1 className={ADMIN_TITLE_CLASS}>AgentHub Admin</h1>
        <div className={ADMIN_SESSION_CLASS}>
          <a className="icon-button" href="/" title="Back" aria-label="Back">
            <i className="bi bi-arrow-left" aria-hidden="true" />
          </a>
          <span>{props.auth.username}</span>
        </div>
      </header>

      {props.error && (
        <ErrorBanner message={props.error} onClose={() => props.setError(null)} />
      )}

      <section className={ADMIN_SECTION_CLASS}>
        <div className={ADMIN_TOOLBAR_CLASS}>
          <h2>Admin</h2>
          <button
            className={ADMIN_PRIMARY_BUTTON_CLASS}
            onClick={props.onCreateJoin}
          >
            Create Join QR
          </button>
        </div>
        <div className={ADMIN_TAB_BAR_CLASS}>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "safe" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("safe")}
          >
            Safe Paths
          </button>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "devices" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("devices")}
          >
            Devices
          </button>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "audits" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("audits")}
          >
            Login Audits
          </button>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "join" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("join")}
          >
            Join Device
          </button>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "vapid" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("vapid")}
          >
            VAPID Keys
          </button>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "ui" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("ui")}
          >
            UI
          </button>
          <button
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "system" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("system")}
          >
            System
          </button>
        </div>

        <div className="admin-panel">
          {tab === "safe" && (
            <div className={ADMIN_CARD_CLASS}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>Safe Paths</h3>
              <div className={ADMIN_FORM_ROW_CLASS}>
                <input
                  className={ADMIN_INPUT_CLASS}
                  placeholder="Add safe path"
                  value={props.safePathInput}
                  onChange={(e) => props.setSafePathInput(e.target.value)}
                />
                <button
                  className={ADMIN_PRIMARY_BUTTON_CLASS}
                  onClick={props.onAddSafePath}
                >
                  Add Path
                </button>
              </div>
              <div className={ADMIN_FORM_ROW_CLASS}>
                <label className="checkbox inline-flex items-center gap-2 text-sm text-slate-700">
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
                <button
                  className={ADMIN_DANGER_BUTTON_CLASS}
                  onClick={props.onDeleteSelectedSafePaths}
                >
                  Delete Selected
                </button>
              </div>
              <ul className={ADMIN_LIST_CLASS}>
                {props.safePaths.map((p) => (
                  <li className={ADMIN_LIST_ITEM_CLASS} key={p.path}>
                    <label className="checkbox inline-flex items-center">
                      <input
                        type="checkbox"
                        checked={props.selectedSafePaths.has(p.path)}
                        onChange={() => props.onToggleSafePath(p.path)}
                      />
                    </label>
                    <span className="mono flex-1 text-sm text-slate-800">{p.path}</span>
                    <button
                      className={ADMIN_DANGER_BUTTON_CLASS}
                      onClick={() => props.onDeleteSafePath(p.path)}
                    >
                      Delete
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "devices" && (
            <div className={ADMIN_CARD_CLASS}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>Devices</h3>
              <ul className={ADMIN_LIST_CLASS}>
                {props.devices.map((device) => (
                  <li className={ADMIN_LIST_ITEM_CLASS} key={device.id}>
                    <span className="text-sm text-slate-800">
                      {device.name} - {device.status}
                    </span>
                    {device.status === "active" && (
                      <button
                        className={ADMIN_DANGER_BUTTON_CLASS}
                        onClick={() => props.onRevokeDevice(device.id)}
                      >
                        Revoke
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "audits" && (
            <div className={ADMIN_CARD_CLASS}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>Login Audits</h3>
              <ul className={ADMIN_LIST_CLASS}>
                {props.audits.map((audit) => (
                  <li className={ADMIN_LIST_ITEM_CLASS} key={audit.id}>
                    <span className={ADMIN_MUTED_TEXT_CLASS}>
                      {new Date(audit.ts * 1000).toLocaleString()} -
                      {` ${audit.event}`}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "join" && (
            <div className={`${ADMIN_CARD_CLASS} join-card`}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>Join Device</h3>
              {props.joinQr && (
                <img
                  className={ADMIN_QR_CLASS}
                  src={props.joinQr}
                  alt="Join device QR code (encodes token and PIN)"
                />
              )}
              {props.joinToken && (
                <p className={ADMIN_MUTED_TEXT_CLASS}>Token: {props.joinToken}</p>
              )}
              {props.joinPin && (
                <p className={ADMIN_MUTED_TEXT_CLASS}>PIN: {props.joinPin}</p>
              )}
            </div>
          )}
          {tab === "vapid" && (
            <div className={ADMIN_CARD_CLASS}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>VAPID Keys</h3>
              {props.vapidInfo ? (
                <div className={ADMIN_KV_LIST_CLASS}>
                  <div className={ADMIN_KV_ROW_CLASS}>
                    <span className={ADMIN_LABEL_CLASS}>Subject</span>
                    <span className={ADMIN_VALUE_CLASS}>{props.vapidInfo.subject}</span>
                  </div>
                  <div className={ADMIN_KV_ROW_CLASS}>
                    <span className={ADMIN_LABEL_CLASS}>Public Key</span>
                    <span className={`${ADMIN_VALUE_CLASS} mono`}>
                      {props.vapidInfo.public_key}
                    </span>
                  </div>
                  <div className={ADMIN_KV_ROW_CLASS}>
                    <span className={ADMIN_LABEL_CLASS}>Keys Path</span>
                    <span className={`${ADMIN_VALUE_CLASS} mono`}>
                      {props.vapidInfo.keys_path}
                    </span>
                  </div>
                </div>
              ) : (
                <p className={ADMIN_EMPTY_TEXT_CLASS}>VAPID keys not loaded.</p>
              )}
              {/* TODO: add copy button and rotate confirmation. */}
              <div className={ADMIN_FORM_ROW_CLASS}>
                <button
                  className={ADMIN_SECONDARY_BUTTON_CLASS}
                  onClick={props.onRotateVapid}
                >
                  Rotate Keys
                </button>
              </div>
            </div>
          )}
          {tab === "ui" && (
            <div className={ADMIN_CARD_CLASS}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>UI Settings</h3>
              <div className="flex flex-col gap-3">
                <Switch
                  checked={props.developerMode}
                  onChange={(event) =>
                    props.onDeveloperModeChange(event.currentTarget.checked)
                  }
                  label="Developer Mode"
                  description="Applies to this browser only. Affects Agents and Teams."
                />
                <p className={ADMIN_MUTED_TEXT_CLASS}>
                  Default behavior: enabled in development and tests, disabled in
                  production builds.
                </p>
              </div>
            </div>
          )}
          {tab === "system" && (
            <div className={ADMIN_CARD_CLASS}>
              <h3 className={ADMIN_CARD_TITLE_CLASS}>System Configuration</h3>
              <div className="flex flex-col gap-3">
                <Switch
                  checked={props.passkeyEnabled ?? false}
                  disabled={props.passkeyEnabled === null}
                  onChange={(event) =>
                    props.onPasskeyEnabledChange(event.currentTarget.checked)
                  }
                  label="Enable Passkey"
                  description={
                    props.passkeyEnabled === null
                      ? "Loading configuration..."
                      : "Global setting. When disabled, only password login/registration is allowed."
                  }
                />
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
