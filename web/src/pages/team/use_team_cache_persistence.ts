import { useEffect, useRef } from "react";
import {
  loadTeamConversationRuntimeCache,
  loadTeamMailboxInboxRuntimeCache,
  loadTeamMemberAcpRuntimeCache,
  saveTeamConversationRuntimeCache,
  saveTeamMailboxInboxRuntimeCache,
  saveTeamMemberAcpRuntimeCache,
} from "./runtime_cache_storage";
import {
  shouldPersistRuntimeCacheFingerprint,
  shouldSkipRuntimeCacheSaveAfterHydrate,
} from "./runtime_cache_hydration";
import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
} from "../../api";

const TEAM_RUNTIME_CACHE_PERSIST_DEBOUNCE_MS = 250;

type UseTeamCachePersistenceOptions = {
  effectiveSelectedTeamId: string | null;
  selectedConversationId: string | null;
  taskMessages: TeamConversationMessageRecord[];
  conversationMailboxMessages: TeamActorMessageRecord[];
  setTaskMessages: (messages: TeamConversationMessageRecord[]) => void;
  setConversationMailboxMessages: (messages: TeamActorMessageRecord[]) => void;
  selectedAgentWorkspaceEventAgentId: string;
  selectedAgentWorkspaceSessionId: string | null;
  memberEvents: AgentEvent[];
  setMemberEvents: (events: AgentEvent[]) => void;
  setMemberEventsHasMore: (hasMore: boolean) => void;
  activeRunIdForSelectedTeam: string | null;
  inboxActorId: string;
  inbox: TeamActorMessageRecord[];
  setInbox: (inbox: TeamActorMessageRecord[]) => void;
};

export function useTeamCachePersistence({
  effectiveSelectedTeamId,
  selectedConversationId,
  taskMessages,
  conversationMailboxMessages,
  setTaskMessages,
  setConversationMailboxMessages,
  selectedAgentWorkspaceEventAgentId,
  selectedAgentWorkspaceSessionId,
  memberEvents,
  setMemberEvents,
  setMemberEventsHasMore,
  activeRunIdForSelectedTeam,
  inboxActorId,
  inbox,
  setInbox,
}: UseTeamCachePersistenceOptions) {
  const pendingConversationCacheHydrationKeyRef = useRef<string | null>(null);
  const pendingMemberAcpCacheHydrationKeyRef = useRef<string | null>(null);
  const pendingMailboxCacheHydrationKeyRef = useRef<string | null>(null);
  const conversationCachePersistTimerRef = useRef<number | null>(null);
  const memberAcpCachePersistTimerRef = useRef<number | null>(null);
  const mailboxCachePersistTimerRef = useRef<number | null>(null);
  const lastConversationCacheFingerprintRef = useRef<string | null>(null);
  const lastMemberAcpCacheFingerprintRef = useRef<string | null>(null);
  const lastMailboxCacheFingerprintRef = useRef<string | null>(null);

  useEffect(
    () => () => {
      if (typeof window !== "undefined") {
        if (conversationCachePersistTimerRef.current != null) {
          window.clearTimeout(conversationCachePersistTimerRef.current);
        }
        if (memberAcpCachePersistTimerRef.current != null) {
          window.clearTimeout(memberAcpCachePersistTimerRef.current);
        }
        if (mailboxCachePersistTimerRef.current != null) {
          window.clearTimeout(mailboxCachePersistTimerRef.current);
        }
      }
    },
    []
  );

  // Conversation Cache
  useEffect(() => {
    const teamId = effectiveSelectedTeamId?.trim() ?? "";
    const conversationId = selectedConversationId?.trim() ?? "";
    if (!teamId || !conversationId) {
      pendingConversationCacheHydrationKeyRef.current = null;
      lastConversationCacheFingerprintRef.current = null;
      return;
    }
    pendingConversationCacheHydrationKeyRef.current = `${teamId}:${conversationId}`;
    const cached = loadTeamConversationRuntimeCache(teamId, conversationId);
    lastConversationCacheFingerprintRef.current = JSON.stringify({
      messages: cached.messages,
      mailboxMessages: cached.mailboxMessages,
    });
    setTaskMessages(cached.messages);
    setConversationMailboxMessages(cached.mailboxMessages);
  }, [effectiveSelectedTeamId, selectedConversationId, setTaskMessages, setConversationMailboxMessages]);

  useEffect(() => {
    const teamId = effectiveSelectedTeamId?.trim() ?? "";
    const conversationId = selectedConversationId?.trim() ?? "";
    if (!teamId || !conversationId) {
      return;
    }
    const cacheKey = `${teamId}:${conversationId}`;
    if (
      shouldSkipRuntimeCacheSaveAfterHydrate(
        pendingConversationCacheHydrationKeyRef.current,
        cacheKey
      )
    ) {
      pendingConversationCacheHydrationKeyRef.current = null;
      return;
    }
    const nextFingerprint = JSON.stringify({
      messages: taskMessages,
      mailboxMessages: conversationMailboxMessages,
    });
    if (
      !shouldPersistRuntimeCacheFingerprint(
        lastConversationCacheFingerprintRef.current,
        nextFingerprint
      )
    ) {
      return;
    }
    if (typeof window === "undefined") {
      saveTeamConversationRuntimeCache(
        teamId,
        conversationId,
        taskMessages,
        conversationMailboxMessages
      );
      lastConversationCacheFingerprintRef.current = nextFingerprint;
      return;
    }
    if (conversationCachePersistTimerRef.current != null) {
      window.clearTimeout(conversationCachePersistTimerRef.current);
    }
    conversationCachePersistTimerRef.current = window.setTimeout(() => {
      saveTeamConversationRuntimeCache(
        teamId,
        conversationId,
        taskMessages,
        conversationMailboxMessages
      );
      lastConversationCacheFingerprintRef.current = nextFingerprint;
      conversationCachePersistTimerRef.current = null;
    }, TEAM_RUNTIME_CACHE_PERSIST_DEBOUNCE_MS);
  }, [
    conversationMailboxMessages,
    effectiveSelectedTeamId,
    selectedConversationId,
    taskMessages,
  ]);

  // Member ACP Cache
  useEffect(() => {
    const agentId = selectedAgentWorkspaceEventAgentId.trim();
    const sessionId = selectedAgentWorkspaceSessionId?.trim() ?? "";
    if (!agentId || !sessionId) {
      pendingMemberAcpCacheHydrationKeyRef.current = null;
      lastMemberAcpCacheFingerprintRef.current = null;
      return;
    }
    pendingMemberAcpCacheHydrationKeyRef.current = `${agentId}:${sessionId}`;
    const cached = loadTeamMemberAcpRuntimeCache(agentId, sessionId);
    lastMemberAcpCacheFingerprintRef.current = JSON.stringify(cached);
    setMemberEvents(cached);
    setMemberEventsHasMore(cached.length > 0);
  }, [selectedAgentWorkspaceEventAgentId, selectedAgentWorkspaceSessionId, setMemberEvents, setMemberEventsHasMore]);

  useEffect(() => {
    const agentId = selectedAgentWorkspaceEventAgentId.trim();
    const sessionId = selectedAgentWorkspaceSessionId?.trim() ?? "";
    if (!agentId || !sessionId) {
      return;
    }
    const cacheKey = `${agentId}:${sessionId}`;
    if (
      shouldSkipRuntimeCacheSaveAfterHydrate(
        pendingMemberAcpCacheHydrationKeyRef.current,
        cacheKey
      )
    ) {
      pendingMemberAcpCacheHydrationKeyRef.current = null;
      return;
    }
    const nextFingerprint = JSON.stringify(memberEvents);
    if (
      !shouldPersistRuntimeCacheFingerprint(
        lastMemberAcpCacheFingerprintRef.current,
        nextFingerprint
      )
    ) {
      return;
    }
    if (typeof window === "undefined") {
      saveTeamMemberAcpRuntimeCache(agentId, sessionId, memberEvents);
      lastMemberAcpCacheFingerprintRef.current = nextFingerprint;
      return;
    }
    if (memberAcpCachePersistTimerRef.current != null) {
      window.clearTimeout(memberAcpCachePersistTimerRef.current);
    }
    memberAcpCachePersistTimerRef.current = window.setTimeout(() => {
      saveTeamMemberAcpRuntimeCache(agentId, sessionId, memberEvents);
      lastMemberAcpCacheFingerprintRef.current = nextFingerprint;
      memberAcpCachePersistTimerRef.current = null;
    }, TEAM_RUNTIME_CACHE_PERSIST_DEBOUNCE_MS);
  }, [memberEvents, selectedAgentWorkspaceEventAgentId, selectedAgentWorkspaceSessionId]);

  // Mailbox Cache
  useEffect(() => {
    const runId = activeRunIdForSelectedTeam?.trim() ?? "";
    const actorId = inboxActorId.trim();
    if (!runId || !actorId) {
      pendingMailboxCacheHydrationKeyRef.current = null;
      lastMailboxCacheFingerprintRef.current = null;
      return;
    }
    pendingMailboxCacheHydrationKeyRef.current = `${runId}:${actorId}`;
    const cached = loadTeamMailboxInboxRuntimeCache(runId, actorId);
    lastMailboxCacheFingerprintRef.current = JSON.stringify(cached);
    setInbox(cached);
  }, [activeRunIdForSelectedTeam, inboxActorId, setInbox]);

  useEffect(() => {
    const runId = activeRunIdForSelectedTeam?.trim() ?? "";
    const actorId = inboxActorId.trim();
    if (!runId || !actorId) {
      return;
    }
    const cacheKey = `${runId}:${actorId}`;
    if (
      shouldSkipRuntimeCacheSaveAfterHydrate(
        pendingMailboxCacheHydrationKeyRef.current,
        cacheKey
      )
    ) {
      pendingMailboxCacheHydrationKeyRef.current = null;
      return;
    }
    const nextFingerprint = JSON.stringify(inbox);
    if (
      !shouldPersistRuntimeCacheFingerprint(
        lastMailboxCacheFingerprintRef.current,
        nextFingerprint
      )
    ) {
      return;
    }
    if (typeof window === "undefined") {
      saveTeamMailboxInboxRuntimeCache(runId, actorId, inbox);
      lastMailboxCacheFingerprintRef.current = nextFingerprint;
      return;
    }
    if (mailboxCachePersistTimerRef.current != null) {
      window.clearTimeout(mailboxCachePersistTimerRef.current);
    }
    mailboxCachePersistTimerRef.current = window.setTimeout(() => {
      saveTeamMailboxInboxRuntimeCache(runId, actorId, inbox);
      lastMailboxCacheFingerprintRef.current = nextFingerprint;
      mailboxCachePersistTimerRef.current = null;
    }, TEAM_RUNTIME_CACHE_PERSIST_DEBOUNCE_MS);
  }, [activeRunIdForSelectedTeam, inboxActorId, inbox]);
}
