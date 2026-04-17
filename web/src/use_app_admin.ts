import { useState, useEffect, useCallback } from "react";
import { AuthState } from "./types";
import { 
  api, 
  SafePath, 
  DeviceRecord, 
  AuditRecord, 
  VapidInfo, 
  parseApiErrorMessage 
} from "./api";

export function useAppAdmin(auth: AuthState | null, isAdminRoute: boolean) {
  const token = auth?.token ?? null;
  const [safePaths, setSafePaths] = useState<SafePath[]>([]);
  const [devices, setDevices] = useState<DeviceRecord[]>([]);
  const [audits, setAudits] = useState<AuditRecord[]>([]);
  const [vapidInfo, setVapidInfo] = useState<VapidInfo | null>(null);
  const [passkeyEnabled, setPasskeyEnabled] = useState<boolean | null>(null);
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [selectedSafePaths, setSelectedSafePaths] = useState<Set<string>>(() => new Set());
  const [safePathInput, setSafePathInput] = useState("");
  const [joinPin, setJoinPin] = useState<string | null>(null);
  const [joinToken, setJoinToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .authStatus()
      .then((res) => {
        setRootInitialized(res.root_initialized);
        setPasskeyEnabled(res.passkey_enabled);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!token || auth?.role !== "root" || !isAdminRoute) {
      setSafePaths([]);
      setDevices([]);
      setAudits([]);
      setVapidInfo(null);
      return;
    }
    api.listSafePaths(token).then(setSafePaths).catch(() => {});
    api.listDevices(token).then(setDevices).catch(() => {});
    api.listAudits(token).then(setAudits).catch(() => {});
    api.getVapidInfo(token).then(setVapidInfo).catch(() => {});
    api.getAdminSettings(token).then(res => {
      setPasskeyEnabled(res.passkey_enabled);
    }).catch(() => {});
  }, [token, auth?.role, isAdminRoute]);

  const onPasskeyEnabledChange = useCallback(async (enabled: boolean) => {
    if (!token || auth?.role !== "root") return;
    try {
      await api.setPasskeyEnabled(token, enabled);
      setPasskeyEnabled(enabled);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, auth?.role]);

  const onAddSafePath = useCallback(async () => {
    if (!token) return;
    try {
      const path = safePathInput.trim();
      if (!path) return;
      await api.addSafePath(token, path);
      const list = await api.listSafePaths(token);
      setSafePaths(list);
      setSafePathInput("");
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, safePathInput]);

  const onDeleteSafePath = useCallback(async (path: string) => {
    if (!token) return;
    try {
      await api.deleteSafePath(token, path);
      const list = await api.listSafePaths(token);
      setSafePaths(list);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token]);

  const onRevokeDevice = useCallback(async (id: string) => {
    if (!token) return;
    try {
      await api.revokeDevice(token, id);
      const list = await api.listDevices(token);
      setDevices(list);
      const items = await api.listAudits(token);
      setAudits(items);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token]);

  const onRotateVapid = useCallback(async () => {
    if (!token) return;
    try {
      await api.rotateVapid(token);
      const info = await api.getVapidInfo(token);
      setVapidInfo(info);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token]);

  const onCreateJoin = useCallback(async () => {
    if (!token) return;
    try {
      const data = await api.joinStartAdmin(token);
      setJoinPin(data.pin);
      setJoinToken(data.token);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token]);

  const onToggleSafePath = useCallback((path: string) => {
    setSelectedSafePaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const onToggleAllSafePaths = useCallback(() => {
    setSelectedSafePaths((prev) => {
      if (prev.size === safePaths.length) return new Set();
      return new Set(safePaths.map((p) => p.path));
    });
  }, [safePaths]);

  const onDeleteSelectedSafePaths = useCallback(async () => {
    if (!token) return;
    try {
      for (const path of selectedSafePaths) {
        await api.deleteSafePath(token, path);
      }
      const list = await api.listSafePaths(token);
      setSafePaths(list);
      setSelectedSafePaths(new Set());
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, selectedSafePaths]);

  return {
    safePaths,
    devices,
    audits,
    vapidInfo,
    passkeyEnabled,
    rootInitialized,
    selectedSafePaths,
    safePathInput,
    setSafePathInput,
    joinPin,
    joinToken,
    error,
    setError,
    onPasskeyEnabledChange,
    onAddSafePath,
    onDeleteSafePath,
    onRevokeDevice,
    onRotateVapid,
    onCreateJoin,
    onToggleSafePath,
    onToggleAllSafePaths,
    onDeleteSelectedSafePaths,
  };
}
