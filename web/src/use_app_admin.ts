import { useState, useEffect, useCallback } from "react";
import { AuthState } from "./types";
import { 
  api, 
  SafePath, 
  DeviceRecord, 
  AuditRecord, 
  VapidInfo, 
  AppLinkerRecord,
  SlockLinkAttemptResponse,
  stringifyApiError 
} from "./api";

const DEFAULT_SLOCK_API_ORIGIN = "https://api.slock.ai";
const DEFAULT_SLOCK_SCOPES = "identity openid profile";

function defaultSlockReturnUrl(): string {
  if (typeof location === "undefined") {
    return "/api/linkers/slock/callback";
  }
  return `${location.origin}/api/linkers/slock/callback`;
}

function parseScopesInput(value: string): string[] {
  return value
    .split(/\s+/)
    .map((scope) => scope.trim())
    .filter(Boolean);
}

export function useAppAdmin(auth: AuthState | null, isAdminRoute: boolean) {
  const token = auth?.token ?? null;
  const [safePaths, setSafePaths] = useState<SafePath[]>([]);
  const [devices, setDevices] = useState<DeviceRecord[]>([]);
  const [audits, setAudits] = useState<AuditRecord[]>([]);
  const [vapidInfo, setVapidInfo] = useState<VapidInfo | null>(null);
  const [slockLinker, setSlockLinker] = useState<AppLinkerRecord | null>(null);
  const [slockLinkAttempt, setSlockLinkAttempt] =
    useState<SlockLinkAttemptResponse | null>(null);
  const [slockApiOrigin, setSlockApiOrigin] = useState(DEFAULT_SLOCK_API_ORIGIN);
  const [slockClientId, setSlockClientId] = useState("");
  const [slockClientSecret, setSlockClientSecret] = useState("");
  const [slockReturnUrl, setSlockReturnUrl] = useState(defaultSlockReturnUrl);
  const [slockScopesInput, setSlockScopesInput] = useState(DEFAULT_SLOCK_SCOPES);
  const [slockCallbackInput, setSlockCallbackInput] = useState("");
  const [passkeyEnabled, setPasskeyEnabled] = useState<boolean | null>(null);
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [selectedSafePaths, setSelectedSafePaths] = useState<Set<string>>(() => new Set());
  const [safePathInput, setSafePathInput] = useState("");
  const [joinUrl, setJoinUrl] = useState<string | null>(null);
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
      .catch(() => {
        setRootInitialized(true);
      });
  }, []);

  useEffect(() => {
    if (!token || auth?.role !== "root" || !isAdminRoute) {
      setSafePaths([]);
      setDevices([]);
      setAudits([]);
      setVapidInfo(null);
      setSlockLinker(null);
      setSlockLinkAttempt(null);
      setJoinUrl(null);
      setJoinPin(null);
      setJoinToken(null);
      return;
    }
    api.listSafePaths(token).then(setSafePaths).catch(() => {});
    api.listDevices(token).then(setDevices).catch(() => {});
    api.listAudits(token).then(setAudits).catch(() => {});
    api.getVapidInfo(token).then(setVapidInfo).catch(() => {});
    api.getAdminSettings(token).then(res => {
      setPasskeyEnabled(res.passkey_enabled);
    }).catch(() => {});
    api.listLinkers(token).then((items) => {
      const slock = items.find((item) => item.connector_id === "slock") ?? null;
      setSlockLinker(slock);
      if (slock) {
        setSlockApiOrigin(slock.api_origin);
        setSlockClientId(slock.client_id);
        setSlockReturnUrl(slock.return_url);
        setSlockScopesInput(slock.scopes.join(" "));
      }
    }).catch(() => {});
  }, [token, auth?.role, isAdminRoute]);

  const onPasskeyEnabledChange = useCallback(async (enabled: boolean) => {
    if (!token || auth?.role !== "root") return;
    try {
      await api.setPasskeyEnabled(token, enabled);
      setPasskeyEnabled(enabled);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
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
      setError(stringifyApiError(err));
    }
  }, [token, safePathInput]);

  const onDeleteSafePath = useCallback(async (path: string) => {
    if (!token) return;
    try {
      await api.deleteSafePath(token, path);
      const list = await api.listSafePaths(token);
      setSafePaths(list);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
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
      setError(stringifyApiError(err));
    }
  }, [token]);

  const onRotateVapid = useCallback(async () => {
    if (!token) return;
    try {
      await api.rotateVapid(token);
      const info = await api.getVapidInfo(token);
      setVapidInfo(info);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [token]);

  const onCreateJoin = useCallback(async () => {
    if (!token) return;
    try {
      const data = await api.joinStartAdmin(token);
      setJoinPin(data.pin);
      setJoinToken(data.token);
      setJoinUrl(`${location.origin}/join?token=${encodeURIComponent(data.token)}`);
    } catch (err: unknown) {
      setJoinPin(null);
      setJoinToken(null);
      setJoinUrl(null);
      setError(stringifyApiError(err));
    }
  }, [token]);

  const onSaveSlockLinker = useCallback(async () => {
    if (!token || auth?.role !== "root") return;
    try {
      const record = await api.upsertSlockLinker(token, {
        api_origin: slockApiOrigin,
        client_id: slockClientId,
        client_secret: slockClientSecret.trim() || null,
        return_url: slockReturnUrl,
        scopes: parseScopesInput(slockScopesInput),
      });
      setSlockLinker(record);
      setSlockClientSecret("");
      setSlockLinkAttempt(null);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [
    token,
    auth?.role,
    slockApiOrigin,
    slockClientId,
    slockClientSecret,
    slockReturnUrl,
    slockScopesInput,
  ]);

  const onCreateSlockLinkAttempt = useCallback(async () => {
    if (!token || auth?.role !== "root") return;
    try {
      const attempt = await api.createSlockLinkAttempt(token);
      setSlockLinkAttempt(attempt);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [token, auth?.role]);

  const onExchangeSlockCode = useCallback(async () => {
    if (!token || auth?.role !== "root") return;
    try {
      const value = slockCallbackInput.trim();
      if (!value) return;
      const payload = value.includes("://")
        ? { callback_url: value }
        : { code: value, state: slockLinkAttempt?.state ?? null };
      const record = await api.exchangeSlockCode(token, payload);
      setSlockLinker(record);
      setSlockCallbackInput("");
      setSlockLinkAttempt(null);
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [token, auth?.role, slockCallbackInput, slockLinkAttempt?.state]);

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
      setError(stringifyApiError(err));
    }
  }, [token, selectedSafePaths]);

  return {
    safePaths,
    devices,
    audits,
    vapidInfo,
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
    passkeyEnabled,
    rootInitialized,
    selectedSafePaths,
    safePathInput,
    setSafePathInput,
    joinUrl,
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
    onSaveSlockLinker,
    onCreateSlockLinkAttempt,
    onExchangeSlockCode,
    onToggleSafePath,
    onToggleAllSafePaths,
    onDeleteSelectedSafePaths,
  };
}
