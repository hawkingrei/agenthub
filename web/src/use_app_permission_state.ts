import { useState } from "react";
import { AcpPermissionRecord } from "./api";

export function useAppPermissionState() {
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>([]);
  const [pendingPermissionCounts, setPendingPermissionCounts] = useState<Record<string, number>>({});
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<AcpPermissionRecord[]>([]);

  return {
    acpPermissions,
    setAcpPermissions,
    pendingPermissionCounts,
    setPendingPermissionCounts,
    acpPermissionHistory,
    setAcpPermissionHistory,
  };
}
