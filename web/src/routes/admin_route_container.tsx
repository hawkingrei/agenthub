import React, { Suspense } from "react";
import type { AuditRecord, DeviceRecord, SafePath, VapidInfo } from "../api";
import { AuthState } from "../types";
import { RouteFallback } from "./route_fallback";
import type { AdminPageProps } from "../pages/admin_page";

const LazyAdminPage = React.lazy(async () => {
  const module = (await import("../pages/admin_page")) as typeof import("../pages/admin_page");
  return { default: module.AdminPage };
});

type AdminRouteContainerProps = {
  appRootRef: React.RefObject<HTMLDivElement | null>;
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
  joinUrl: string | null;
  joinToken: string | null;
  joinPin: string | null;
  safePathInput: string;
  setSafePathInput: (value: string) => void;
  developerMode: boolean;
  onDeveloperModeChange: (value: boolean) => void;
  passkeyEnabled: boolean | null;
  onPasskeyEnabledChange: (value: boolean) => void;
};

export function AdminRouteContainer({
  appRootRef,
  auth,
  error,
  setError,
  safePaths,
  selectedSafePaths,
  onToggleSafePath,
  onToggleAllSafePaths,
  onDeleteSelectedSafePaths,
  devices,
  audits,
  vapidInfo,
  onRotateVapid,
  onAddSafePath,
  onDeleteSafePath,
  onRevokeDevice,
  onCreateJoin,
  joinUrl,
  joinToken,
  joinPin,
  safePathInput,
  setSafePathInput,
  developerMode,
  onDeveloperModeChange,
  passkeyEnabled,
  onPasskeyEnabledChange,
}: AdminRouteContainerProps) {
  const pageProps: AdminPageProps = {
    auth,
    error,
    setError,
    safePaths: {
      safePaths,
      selectedSafePaths,
      safePathInput,
      setSafePathInput,
      onAddSafePath,
      onToggleSafePath,
      onToggleAllSafePaths,
      onDeleteSelectedSafePaths,
      onDeleteSafePath,
    },
    devices: {
      devices,
      onRevokeDevice,
    },
    audits: {
      audits,
    },
    join: {
      onCreateJoin,
      joinUrl,
      joinToken,
      joinPin,
    },
    vapid: {
      vapidInfo,
      onRotateVapid,
    },
    ui: {
      developerMode,
      onDeveloperModeChange,
    },
    system: {
      passkeyEnabled,
      onPasskeyEnabledChange,
    },
  };

  return (
    <div className="app bg-white" ref={appRootRef}>
      <Suspense fallback={<RouteFallback label="Loading admin console..." />}>
        <LazyAdminPage {...pageProps} />
      </Suspense>
    </div>
  );
}
