import { useState, useCallback, useRef, useEffect } from "react";
import { AuthState } from "./types";
import { api, OutputLine } from "./api";
import { compareEventOrder } from "./seq_order";
import { 
  loadOutputCaches, 
  saveOutputCaches 
} from "./storage/output_cache_storage";
import { 
  DEFAULT_OUTPUT_CACHE_MAX_EVENTS, 
  DEFAULT_OUTPUT_CACHE_MAX_SESSIONS 
} from "./storage/output_cache_budget";
import { 
  buildAcpCacheSlice, 
  buildOutputCacheSlice, 
  isSameOutputList, 
  limitOutputCacheSessions, 
  mergeOutputs, 
  mergeOutputsPreserveHistory, 
  mergeOutputsPreserveOlderWithLimit, 
  mergeOutputsWithLimit, 
  replaceAcpCacheSlice, 
  selectCachedOutputs 
} from "./output_cache";
import { 
  buildLatestLiveSessionMap, 
  resolveLiveSessionSwitch, 
  routeLiveOutputBatch 
} from "./app_live_output";
import { isAgentActiveStatus } from "./agent_ws";
import { 
  getMaxEventCursor, 
  isCursorNewer, 
  EventCursor 
} from "./event_polling";
import { resolveOutputHistoryKey, resolveSessionScopedEvents } from "./app_agents_helpers";

const LIVE_OUTPUT_RETENTION_LIMIT = 1200;
const LIVE_ACP_OUTPUT_RETENTION_LIMIT = 1200;

export function useAppOutputCache(
  auth: AuthState | null,
  activeAgent: string | null,
  activeAgentStatus: string | null,
  setAgents: React.Dispatch<React.SetStateAction<AgentRecord[]>>
) {
  const token = auth?.token ?? null;
  const eventLimit = 80;
  const maxCachedEvents = DEFAULT_OUTPUT_CACHE_MAX_EVENTS;
  const maxCachedSessions = DEFAULT_OUTPUT_CACHE_MAX_SESSIONS;

  const [outputs, setOutputs] = useState<OutputLine[]>([]);
  const [acpOutputs, setAcpOutputs] = useState<OutputLine[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [agentSessions, setAgentSessions] = useState<Record<string, string>>({});
  
  const initialCachesRef = useRef(loadOutputCaches(maxCachedEvents, maxCachedSessions));
  const [outputCache, setOutputCache] = useState<Record<string, OutputLine[]>>(initialCachesRef.current.outputCache);
  const [acpOutputCache, setAcpOutputCache] = useState<Record<string, OutputLine[]>>(initialCachesRef.current.acpOutputCache);

  const outputsRef = useRef(outputs);
  const acpOutputsRef = useRef(acpOutputs);
  const outputCacheRef = useRef(outputCache);
  const acpOutputCacheRef = useRef(acpOutputCache);
  const activeAgentRef = useRef(activeAgent);
  const activeSessionIdRef = useRef(activeSessionId);
  const activeAgentStatusRef = useRef(activeAgentStatus);
  const lastEventCursorRef = useRef<Record<string, EventCursor>>({});
  const loadSeq = useRef(0);
  const outputsKeyRef = useRef<string | null>(null);
  const [eventMeta, setEventMeta] = useState<Record<string, {
    oldestId: number | null;
    hasMore: boolean;
    loading: boolean;
    loaded: boolean;
  }>>({});

  useEffect(() => { outputsRef.current = outputs; }, [outputs]);
  useEffect(() => { acpOutputsRef.current = acpOutputs; }, [acpOutputs]);
  useEffect(() => { outputCacheRef.current = outputCache; }, [outputCache]);
  useEffect(() => { acpOutputCacheRef.current = acpOutputCache; }, [acpOutputCache]);
  useEffect(() => { activeAgentRef.current = activeAgent; }, [activeAgent]);
  useEffect(() => { activeSessionIdRef.current = activeSessionId; }, [activeSessionId]);
  useEffect(() => { activeAgentStatusRef.current = activeAgentStatus; }, [activeAgentStatus]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      saveOutputCaches(outputCache, acpOutputCache, maxCachedEvents, maxCachedSessions);
    }, 500);
    return () => window.clearTimeout(timer);
  }, [outputCache, acpOutputCache, maxCachedEvents, maxCachedSessions]);

  const updateOutputCacheEntry = useCallback((key: string, ordered: OutputLine[]) => {
    const existing = outputCacheRef.current[key] ?? [];
    const nextSlice = buildOutputCacheSlice(existing, ordered, maxCachedEvents);
    if (!isSameOutputList(existing, nextSlice)) {
      const nextCache = limitOutputCacheSessions({ ...outputCacheRef.current, [key]: nextSlice }, maxCachedSessions);
      outputCacheRef.current = nextCache;
      setOutputCache(nextCache);
    }
    return nextSlice;
  }, [maxCachedEvents, maxCachedSessions]);

  const updateAcpOutputCacheEntry = useCallback((key: string, ordered: OutputLine[]) => {
    const existing = acpOutputCacheRef.current[key] ?? [];
    const nextSlice = buildAcpCacheSlice(existing, ordered, maxCachedEvents);
    if (!isSameOutputList(existing, nextSlice)) {
      const nextCache = limitOutputCacheSessions({ ...acpOutputCacheRef.current, [key]: nextSlice }, maxCachedSessions);
      acpOutputCacheRef.current = nextCache;
      setAcpOutputCache(nextCache);
    }
    return nextSlice;
  }, [maxCachedEvents, maxCachedSessions]);

  const replaceAcpOutputCacheEntry = useCallback((key: string, ordered: OutputLine[]) => {
    const nextSlice = replaceAcpCacheSlice(ordered, maxCachedEvents);
    const existing = acpOutputCacheRef.current[key] ?? [];
    if (!isSameOutputList(existing, nextSlice)) {
      const nextCache = limitOutputCacheSessions({ ...acpOutputCacheRef.current, [key]: nextSlice }, maxCachedSessions);
      acpOutputCacheRef.current = nextCache;
      setAcpOutputCache(nextCache);
    }
    return nextSlice;
  }, [maxCachedEvents, maxCachedSessions]);

  const consumeLiveOutputBatch = useCallback((lines: OutputLine[]) => {
    if (lines.length === 0) return;
    const currentActive = activeAgentRef.current;
    const currentSessionId = activeSessionIdRef.current;
    const { activeLines, activeAcpLines } = routeLiveOutputBatch({
      cursorRef: lastEventCursorRef,
      lines,
      activeAgent: currentActive,
      activeSessionId: currentSessionId,
      updateAgents: setAgents,
      onOutputGroup: updateOutputCacheEntry,
      onAcpGroup: updateAcpOutputCacheEntry,
    });
    
    const latestSessions = buildLatestLiveSessionMap(lines);
    if (Object.keys(latestSessions).length > 0) {
      setAgentSessions((prev) => {
        let next = prev;
        for (const [agentId, sessionId] of Object.entries(latestSessions)) {
          if (prev[agentId] === sessionId) continue;
          if (next === prev) next = { ...prev };
          next[agentId] = sessionId;
        }
        return next;
      });
    }

    const liveSessionSwitch = resolveLiveSessionSwitch(lines, currentActive, currentSessionId);
    if (liveSessionSwitch && liveSessionSwitch !== currentSessionId && isAgentActiveStatus(activeAgentStatusRef.current)) {
      setActiveSessionId(liveSessionSwitch);
    }

    if (activeLines.length > 0) {
      setOutputs((prev) => mergeOutputsWithLimit(prev, activeLines, LIVE_OUTPUT_RETENTION_LIMIT));
    }
    if (activeAcpLines.length > 0) {
      setAcpOutputs((prev) => mergeOutputsWithLimit(prev, activeAcpLines, LIVE_ACP_OUTPUT_RETENTION_LIMIT));
    }
  }, [updateAcpOutputCacheEntry, updateOutputCacheEntry, setAgents]);

  const loadAgentEvents = useCallback(async (id: string, sessionId?: string | null): Promise<boolean> => {
    if (!token) return false;
    const seq = ++loadSeq.current;
    const key = `${id}:${sessionId ?? "latest"}`;
    setEventMeta((prev) => ({
      ...prev,
      [key]: { ...prev[key], loading: true, loaded: prev[key]?.loaded ?? false },
    }));

    try {
      const events = await api.listAgentEvents(token, id, eventLimit, sessionId ?? undefined);
      if (seq !== loadSeq.current) return false;
      
      const { latestSessionId, resolvedSessionId, scopedEvents } = resolveSessionScopedEvents(events, sessionId ?? null);
      if (latestSessionId) {
        setAgentSessions((prev) => ({ ...prev, [id]: latestSessionId }));
        if (activeAgentRef.current === id && !activeSessionIdRef.current) {
          setActiveSessionId(latestSessionId);
        }
      }

      const ordered = [...scopedEvents].sort((a, b) => compareEventOrder(a, b));
      const resolvedKey = `${id}:${resolvedSessionId ?? "latest"}`;
      const nextSlice = updateOutputCacheEntry(key, ordered);
      if (resolvedKey !== key) updateOutputCacheEntry(resolvedKey, ordered);
      
      if (sessionId) updateAcpOutputCacheEntry(key, ordered);
      else replaceAcpOutputCacheEntry(key, ordered);
      if (resolvedKey !== key) updateAcpOutputCacheEntry(resolvedKey, ordered);

      const oldestEvent = nextSlice.length ? nextSlice[0] : null;
      const oldestId = typeof oldestEvent?.event_id === "number" ? oldestEvent.event_id : null;
      let hasNew = false;
      const maxCursor = getMaxEventCursor(ordered);
      if (maxCursor != null) {
        const prevCursor = lastEventCursorRef.current[key];
        lastEventCursorRef.current[key] = maxCursor;
        hasNew = prevCursor == null ? true : isCursorNewer(prevCursor, maxCursor);
      }

      setEventMeta((prev) => {
        const nextMeta = { oldestId, hasMore: ordered.length >= eventLimit, loading: false, loaded: true };
        const apply = (metaKey: string, currentState: typeof prev) => ({ ...currentState, [metaKey]: nextMeta });
        let nextState = apply(key, prev);
        if (resolvedKey !== key) nextState = apply(resolvedKey, nextState);
        return nextState;
      });
      return hasNew;
    } catch {
      setEventMeta((prev) => ({ ...prev, [key]: { ...prev[key], loading: false, loaded: true } }));
      return false;
    }
  }, [token, updateOutputCacheEntry, updateAcpOutputCacheEntry, replaceAcpOutputCacheEntry]);

  const loadOlderEvents = useCallback(async () => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const meta = eventMeta[key];
    if (!meta || meta.loading || !meta.hasMore || meta.oldestId == null) return;

    setEventMeta((prev) => ({ ...prev, [key]: { ...meta, loading: true } }));
    try {
      const older = await api.listAgentEvents(token, activeAgent, eventLimit, activeSessionId ?? undefined, meta.oldestId);
      const ordered = [...older].sort((a, b) => compareEventOrder(a, b));
      const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
      const nextOldestEvent = ordered.length ? ordered[0] : null;
      const nextOldestId = typeof nextOldestEvent?.event_id === "number" ? nextOldestEvent.event_id : meta.oldestId;
      const hasMore = ordered.length >= eventLimit;

      setOutputs((prev) => mergeOutputsPreserveOlderWithLimit(prev, ordered, LIVE_OUTPUT_RETENTION_LIMIT));
      setAcpOutputs((prev) => mergeOutputsPreserveOlderWithLimit(prev, acpOrdered, LIVE_ACP_OUTPUT_RETENTION_LIMIT));
      updateOutputCacheEntry(key, ordered);
      if (acpOrdered.length > 0) updateAcpOutputCacheEntry(key, acpOrdered);

      setEventMeta((prev) => ({ ...prev, [key]: { oldestId: nextOldestId, hasMore, loading: false, loaded: true } }));
    } catch {
      setEventMeta((prev) => ({ ...prev, [key]: { ...meta, loading: false } }));
    }
  }, [token, activeAgent, activeSessionId, eventMeta, updateOutputCacheEntry, updateAcpOutputCacheEntry]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const latestKey = `${activeAgent}:latest`;
    const viewKey = resolveOutputHistoryKey(activeAgent, activeSessionId, agentSessions);
    const previousKey = outputsKeyRef.current;
    const selection = selectCachedOutputs(outputCache, acpOutputCache, key, latestKey);

    if (selection.source === "none") {
      if (eventMeta[key]?.loading) return;
      setOutputs([]);
      setAcpOutputs([]);
      outputsKeyRef.current = viewKey;
      loadAgentEvents(activeAgent, activeSessionId);
      return;
    }

    const baseOutputs = selection.outputs ?? [];
    const baseAcpOutputs = selection.acpOutputs ?? [];
    const latestOutputs = activeSessionId && key !== latestKey ? (outputCache[latestKey] ?? []).filter(evt => evt.session_id === activeSessionId) : [];
    const latestAcpOutputs = activeSessionId && key !== latestKey ? (acpOutputCache[latestKey] ?? []).filter(evt => evt.session_id === activeSessionId) : [];
    
    const combinedOutputs = (selection.source === "session" && activeSessionId && key !== latestKey && baseOutputs.length > 0) ? mergeOutputs(baseOutputs, latestOutputs) : baseOutputs;
    const combinedAcpOutputs = (selection.source === "session" && activeSessionId && key !== latestKey && baseAcpOutputs.length > 0) ? mergeOutputs(baseAcpOutputs, latestAcpOutputs) : baseAcpOutputs;

    const shouldPreserveHistory = previousKey === viewKey;
    const nextOutputs = mergeOutputsPreserveHistory(outputsRef.current, combinedOutputs, shouldPreserveHistory);
    const nextAcpOutputs = mergeOutputsPreserveHistory(acpOutputsRef.current, combinedAcpOutputs, shouldPreserveHistory);

    if (!isSameOutputList(outputsRef.current, nextOutputs)) setOutputs(nextOutputs);
    if (!isSameOutputList(acpOutputsRef.current, nextAcpOutputs)) setAcpOutputs(nextAcpOutputs);

    outputsKeyRef.current = viewKey;
    if (!eventMeta[key]) {
      const oldestEvent = nextOutputs.length ? nextOutputs[0] : nextAcpOutputs.length ? nextAcpOutputs[0] : null;
      const oldestId = typeof oldestEvent?.event_id === "number" ? oldestEvent.event_id : null;
      setEventMeta((prev) => ({
        ...prev,
        [key]: { oldestId, hasMore: nextOutputs.length + nextAcpOutputs.length >= eventLimit, loading: false, loaded: true }
      }));
    }
  }, [token, activeAgent, activeSessionId, agentSessions, outputCache, acpOutputCache, loadAgentEvents, eventMeta]);

  return {
    outputs,
    setOutputs,
    acpOutputs,
    setAcpOutputs,
    activeSessionId,
    setActiveSessionId,
    agentSessions,
    setAgentSessions,
    outputCache,
    acpOutputCache,
    eventMeta,
    loadAgentEvents,
    loadOlderEvents,
    consumeLiveOutputBatch,
  };
}
