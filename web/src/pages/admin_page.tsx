import { UnstyledButton } from "@mantine/core";
import { useEffect, useState } from "react";
import { AuditRecord, DeviceRecord, SafePath, VapidInfo } from "../api";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import { ActionButton } from "../ui/primitives";
import {
  AdminAuditsSection,
  AdminDevicesSection,
  AdminJoinSection,
  AdminSafePathsSection,
  AdminSystemSection,
  AdminUiSection,
  AdminVapidSection,
} from "./admin_page_sections";
import {
  ADMIN_APP_CLASS,
  ADMIN_HEADER_CLASS,
  ADMIN_PRIMARY_BUTTON_CLASS,
  ADMIN_SECTION_CLASS,
  ADMIN_SESSION_CLASS,
  ADMIN_TAB_BAR_CLASS,
  ADMIN_TAB_BUTTON_ACTIVE_CLASS,
  ADMIN_TAB_BUTTON_BASE_CLASS,
  ADMIN_TAB_BUTTON_IDLE_CLASS,
  ADMIN_TITLE_CLASS,
  ADMIN_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

export type AdminSafePathsSectionProps = {
  safePaths: SafePath[];
  selectedSafePaths: Set<string>;
  safePathInput: string;
  setSafePathInput: (value: string) => void;
  onAddSafePath: () => void;
  onToggleSafePath: (path: string) => void;
  onToggleAllSafePaths: () => void;
  onDeleteSelectedSafePaths: () => void;
  onDeleteSafePath: (path: string) => void;
};

export type AdminDevicesSectionProps = {
  devices: DeviceRecord[];
  onRevokeDevice: (id: string) => void;
};

export type AdminAuditsSectionProps = {
  audits: AuditRecord[];
};

export type AdminJoinSectionProps = {
  onCreateJoin: () => void;
  joinUrl: string | null;
  joinToken: string | null;
  joinPin: string | null;
};

export type AdminVapidSectionProps = {
  vapidInfo: VapidInfo | null;
  onRotateVapid: () => void;
};

export type AdminUiSectionProps = {
  developerMode: boolean;
  onDeveloperModeChange: (value: boolean) => void;
};

export type AdminSystemSectionProps = {
  passkeyEnabled: boolean | null;
  onPasskeyEnabledChange: (value: boolean) => void;
};

export type AdminPageProps = {
  auth: AuthState;
  error: string | null;
  setError: (value: string | null) => void;
  safePaths: AdminSafePathsSectionProps;
  devices: AdminDevicesSectionProps;
  audits: AdminAuditsSectionProps;
  join: AdminJoinSectionProps;
  vapid: AdminVapidSectionProps;
  ui: AdminUiSectionProps;
  system: AdminSystemSectionProps;
};

export function AdminPage(props: AdminPageProps) {
  const [tab, setTab] = useState<
    "safe" | "devices" | "audits" | "join" | "vapid" | "ui" | "system"
  >("safe");
  const [joinLinkCopyState, setJoinLinkCopyState] = useState<"idle" | "copied" | "failed">(
    "idle"
  );

  useEffect(() => {
    setJoinLinkCopyState("idle");
  }, [props.join.joinUrl]);

  const handleCopyJoinLink = async () => {
    if (!props.join.joinUrl) return;
    try {
      await navigator.clipboard.writeText(props.join.joinUrl);
      setJoinLinkCopyState("copied");
    } catch {
      setJoinLinkCopyState("failed");
    }
  };

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
          <ActionButton
            tone="primary"
            size="md"
            className={ADMIN_PRIMARY_BUTTON_CLASS}
            onClick={props.join.onCreateJoin}
          >
            Create Join Token
          </ActionButton>
        </div>
        <div className={ADMIN_TAB_BAR_CLASS}>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "safe" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("safe")}
          >
            Safe Paths
          </UnstyledButton>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "devices" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("devices")}
          >
            Devices
          </UnstyledButton>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "audits" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("audits")}
          >
            Login Audits
          </UnstyledButton>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "join" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("join")}
          >
            Join Device
          </UnstyledButton>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "vapid" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("vapid")}
          >
            VAPID Keys
          </UnstyledButton>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "ui" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("ui")}
          >
            UI
          </UnstyledButton>
          <UnstyledButton
            className={`${ADMIN_TAB_BUTTON_BASE_CLASS} ${tab === "system" ? ADMIN_TAB_BUTTON_ACTIVE_CLASS : ADMIN_TAB_BUTTON_IDLE_CLASS}`}
            onClick={() => setTab("system")}
          >
            System
          </UnstyledButton>
        </div>

        <div className="admin-panel">
          {tab === "safe" ? <AdminSafePathsSection safePaths={props.safePaths} /> : null}

          {tab === "devices" ? <AdminDevicesSection devices={props.devices} /> : null}

          {tab === "audits" ? <AdminAuditsSection audits={props.audits} /> : null}

          {tab === "join" ? (
            <AdminJoinSection
              join={props.join}
              joinLinkCopyState={joinLinkCopyState}
              onCopyJoinLink={() => {
                void handleCopyJoinLink();
              }}
            />
          ) : null}
          {tab === "vapid" ? <AdminVapidSection vapid={props.vapid} /> : null}
          {tab === "ui" ? <AdminUiSection ui={props.ui} /> : null}
          {tab === "system" ? <AdminSystemSection system={props.system} /> : null}
        </div>
      </section>
    </div>
  );
}
