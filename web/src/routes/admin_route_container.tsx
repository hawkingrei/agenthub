import React, { Suspense } from "react";
import type {
  AppLinkerRecord,
  AuditRecord,
  DeviceRecord,
  SafePath,
  SlockLinkAttemptResponse,
  VapidInfo,
} from "../api";
import { AuthState } from "../types";
import { RouteFallback } from "./route_fallback";
import type { AdminPageProps } from "../pages/admin_page";
import { APP_ROOT_CLASS } from "../ui/tailwind_classes";

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
  slockLinker: AppLinkerRecord | null;
  slockLinkAttempt: SlockLinkAttemptResponse | null;
  slockApiOrigin: string;
  setSlockApiOrigin: (value: string) => void;
  slockClientId: string;
  setSlockClientId: (value: string) => void;
  slockClientSecret: string;
  setSlockClientSecret: (value: string) => void;
  slockReturnUrl: string;
  setSlockReturnUrl: (value: string) => void;
  slockScopesInput: string;
  setSlockScopesInput: (value: string) => void;
  slockCallbackInput: string;
  setSlockCallbackInput: (value: string) => void;
  onSaveSlockLinker: () => void;
  onCreateSlockLinkAttempt: () => void;
  onExchangeSlockCode: () => void;
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
  slockLinker,
  slockLinkAttempt,
  slockApiOrigin,
  setSlockApiOrigin,
  slockClientId,
  setSlockClientId,
  slockClientSecret,
  setSlockClientSecret,
  slockReturnUrl,
  setSlockReturnUrl,
  slockScopesInput,
  setSlockScopesInput,
  slockCallbackInput,
  setSlockCallbackInput,
  onSaveSlockLinker,
  onCreateSlockLinkAttempt,
  onExchangeSlockCode,
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
    linkers: {
      slockLinker,
      slockLinkAttempt,
      slockApiOrigin,
      setSlockApiOrigin,
      slockClientId,
      setSlockClientId,
      slockClientSecret,
      setSlockClientSecret,
      slockReturnUrl,
      setSlockReturnUrl,
      slockScopesInput,
      setSlockScopesInput,
      slockCallbackInput,
      setSlockCallbackInput,
      onSaveSlockLinker,
      onCreateSlockLinkAttempt,
      onExchangeSlockCode,
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
    <div className={APP_ROOT_CLASS} ref={appRootRef as React.Ref<HTMLDivElement>}>
      <Suspense fallback={<RouteFallback label="Loading admin console..." />}>
        <LazyAdminPage {...pageProps} />
      </Suspense>
    </div>
  );
}
