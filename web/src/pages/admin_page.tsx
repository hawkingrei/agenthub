import React, { useState } from "react";
import { AuditRecord, DeviceRecord, SafePath, VapidInfo } from "../api";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import {
  ADMIN_APP_CLASS,
  ADMIN_CARD_CLASS,
  ADMIN_CARD_TITLE_CLASS,
  ADMIN_DANGER_BUTTON_CLASS,
  ADMIN_EMPTY_TEXT_CLASS,
  ADMIN_FORM_ROW_CLASS,
  ADMIN_HEADER_CLASS,
  ADMIN_INPUT_CLASS,
  ADMIN_KV_LIST_CLASS,
  ADMIN_KV_ROW_CLASS,
  ADMIN_LABEL_CLASS,
  ADMIN_LIST_CLASS,
  ADMIN_LIST_ITEM_CLASS,
  ADMIN_MUTED_TEXT_CLASS,
  ADMIN_PRIMARY_BUTTON_CLASS,
  ADMIN_QR_CLASS,
  ADMIN_SECONDARY_BUTTON_CLASS,
  ADMIN_SECTION_CLASS,
  ADMIN_SESSION_CLASS,
  ADMIN_TAB_BAR_CLASS,
  ADMIN_TAB_BUTTON_ACTIVE_CLASS,
  ADMIN_TAB_BUTTON_BASE_CLASS,
  ADMIN_TAB_BUTTON_IDLE_CLASS,
  ADMIN_TITLE_CLASS,
  ADMIN_TOOLBAR_CLASS,
  ADMIN_VALUE_CLASS,
} from "../ui/tailwind_classes";

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

type AdminToggleFieldProps = {
  checked: boolean;
  disabled?: boolean;
  label: string;
  description: string;
  onChange: (value: boolean) => void;
};

function AdminToggleField({
  checked,
  disabled = false,
  label,
  description,
  onChange,
}: AdminToggleFieldProps) {
  return (
    <label
      className={`flex items-start gap-3 rounded-xl border border-notion-border bg-notion-sidebar/20 px-4 py-3 ${
        disabled ? "opacity-60" : "cursor-pointer hover:bg-notion-hover/50"
      }`}
    >
      <input
        type="checkbox"
        className="mt-0.5 h-4 w-4 rounded border-notion-border text-notion-accent focus:ring-notion-accent/20"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span className="min-w-0 space-y-1">
        <span className="block text-sm font-semibold text-notion-text">{label}</span>
        <span className={ADMIN_MUTED_TEXT_CLASS}>{description}</span>
      </span>
    </label>
  );
}

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
                <AdminToggleField
                  checked={props.developerMode}
                  onChange={props.onDeveloperModeChange}
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
                <AdminToggleField
                  checked={props.passkeyEnabled ?? false}
                  disabled={props.passkeyEnabled === null}
                  onChange={props.onPasskeyEnabledChange}
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
