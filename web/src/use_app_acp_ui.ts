import { useState, useCallback, useMemo, useEffect } from "react";
import { resolveActiveAcpView } from "./app_agents_helpers";
import { isAgentActiveStatus } from "./agent_ws";
import { loadDeveloperModePreference, persistDeveloperModePreference } from "./ui/developer_mode";
import { formatAgentModelLabel } from "./agent_presets";
import { api, AgentRecord, parseApiErrorMessage } from "./api";
import { createAnsiRenderer } from "./app_utils";
import type { OutputLine } from "./output_cache";

export function useAppAcpUi(
  token: string | null,
  activeAgent: string | null,
  activeAgentRecord: AgentRecord | null,
  acpOutputs: OutputLine[],
  pendingPermissionCounts: Record<string, number>,
  setError: (msg: string | null) => void
) {
  const [acpTab, setAcpTab] = useState<"conversation" | "plan" | "debug">("conversation");
  const [developerMode, setDeveloperMode] = useState<boolean>(() => loadDeveloperModePreference());
  const [acpModeId, setAcpModeId] = useState("");
  const [acpModelId, setAcpModelId] = useState("");
  const [acpConfigId, setAcpConfigId] = useState("");
  const [acpConfigValue, setAcpConfigValue] = useState("");
  const [thinkingTick, setThinkingTick] = useState(0);

  const ansi = useMemo(() => createAnsiRenderer(), []);

  const handleDeveloperModeChange = useCallback((next: boolean) => {
    setDeveloperMode(next);
    persistDeveloperModePreference(next);
  }, []);

  const handleAcpTabSelect = useCallback(
    (next: "conversation" | "plan" | "debug") => {
      if (!developerMode && next === "debug") {
        setAcpTab("conversation");
        return;
      }
      setAcpTab(next);
    },
    [developerMode]
  );

  const acpView = useMemo(
    () => resolveActiveAcpView(activeAgent, acpOutputs),
    [activeAgent, acpOutputs]
  );

  const activeAgentModelLabel = useMemo(() => {
    if (!activeAgentRecord) return null;
    return formatAgentModelLabel(activeAgentRecord.command, activeAgentRecord.args);
  }, [activeAgentRecord]);

  const hasPendingPermissions = useMemo(() => {
    return Object.values(pendingPermissionCounts).some((count) => count > 0);
  }, [pendingPermissionCounts]);

  const isAgentActive = isAgentActiveStatus(activeAgentRecord?.status ?? null);
  const thinkingStartTs = acpView.thinkingStartTs;

  useEffect(() => {
    if (!thinkingStartTs) return;
    setThinkingTick(0);
    const timer = window.setInterval(() => {
      setThinkingTick((prev) => prev + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [thinkingStartTs]);

  const canControlAcp = Boolean(activeAgent && isAgentActive);
  const hasInProgressToolCall = acpView.toolCalls.some((call) => call.status === "in_progress");
  const canInterruptAcpRun = canControlAcp && (acpView.runStatus?.status === "running" || hasInProgressToolCall);

  const onAcpSetMode = useCallback(async (requestedModeId: string) => {
    if (!token || !activeAgent) return;
    const modeId = requestedModeId.trim();
    if (!modeId) {
      setError("mode id is required");
      return;
    }
    setError(null);
    try {
      await api.setAcpMode(token, activeAgent, modeId);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, setError]);

  const onAcpSetModel = useCallback(async (requestedModelId: string) => {
    if (!token || !activeAgent) return;
    const modelId = requestedModelId.trim();
    if (!modelId) {
      setError("model id is required");
      return;
    }
    setError(null);
    try {
      await api.setAcpModel(token, activeAgent, modelId);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, setError]);

  const onAcpSetConfig = useCallback(async () => {
    if (!token || !activeAgent) return;
    const trimmedId = acpConfigId.trim();
    const trimmedValue = acpConfigValue.trim();
    if (!trimmedId || !trimmedValue) {
      setError("config id and value are required");
      return;
    }
    setError(null);
    try {
      await api.setAcpConfig(token, activeAgent, trimmedId, trimmedValue);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, acpConfigId, acpConfigValue, setError]);

  const onAcpCancel = useCallback(async () => {
    if (!token || !activeAgent) return;
    setError(null);
    try {
      await api.cancelAcp(token, activeAgent);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, setError]);

  const onAcpClearSession = useCallback(async () => {
    if (!token || !activeAgent) return;
    setError(null);
    try {
      await api.clearAcpSession(token, activeAgent);
    } catch (err: unknown) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, setError]);

  return {
    acpTab,
    setAcpTab: handleAcpTabSelect,
    developerMode,
    setDeveloperMode: handleDeveloperModeChange,
    acpModeId,
    setAcpModeId,
    acpModelId,
    setAcpModelId,
    acpConfigId,
    setAcpConfigId,
    acpConfigValue,
    setAcpConfigValue,
    ansi,
    acpView,
    activeAgentModelLabel,
    hasPendingPermissions,
    isAgentActive,
    canControlAcp,
    canInterruptAcpRun,
    thinkingTick,
    onAcpSetMode,
    onAcpSetModel,
    onAcpSetConfig,
    onAcpCancel,
    onAcpClearSession,
  };
}
