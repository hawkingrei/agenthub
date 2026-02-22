import { useCallback, useMemo, type Dispatch, type SetStateAction } from "react";
import { api, type TeamActorMessageRecord, type TeamRunSnapshotRecord } from "../../api";
import {
  parseErrorMessage,
  parseOptionalJson,
  parseRequiredJson,
} from "./create_helpers";
import { buildMailboxChatPayload } from "./mailbox_helpers";
import type { TeamTab } from "./state";

type UseTeamMailboxActionsOptions = {
  token: string;
  tab: TeamTab;
  activeRunIdForSelectedTeam: string | null;
  chatFromActorId: string;
  chatToActorId: string;
  chatDraft: string;
  msgFromActorId: string;
  msgToActorId: string;
  msgChannel: string;
  msgTransport: "local" | "remote";
  msgRoute: string;
  msgPayload: string;
  msgIdempotencyKey: string;
  inboxActorId: string;
  setBusy: Dispatch<SetStateAction<string | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setChatDraft: (next: string) => void;
  loadInbox: (actorIdOverride?: string) => Promise<void>;
  refreshSnapshot: (runId: string) => Promise<TeamRunSnapshotRecord>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<void>;
};

type TeamMailboxApiClient = {
  sendTeamRunMessage: (
    runId: string,
    payload: {
      from_actor_id: string;
      to_actor_id: string;
      channel?: string;
      transport?: "local" | "remote";
      route?: unknown;
      payload: unknown;
      idempotency_key?: string;
    }
  ) => Promise<TeamActorMessageRecord>;
  ackTeamRunMessage: (
    runId: string,
    messageId: number,
    actorId: string
  ) => Promise<TeamActorMessageRecord>;
};

function buildTeamMailboxApiClient(token: string): TeamMailboxApiClient {
  return {
    sendTeamRunMessage: (runId, payload) => api.sendTeamRunMessage(token, runId, payload),
    ackTeamRunMessage: (runId, messageId, actorId) =>
      api.ackTeamRunMessage(token, runId, messageId, actorId),
  };
}

export function useTeamMailboxActions(options: UseTeamMailboxActionsOptions) {
  const {
    token,
    tab,
    activeRunIdForSelectedTeam,
    chatFromActorId,
    chatToActorId,
    chatDraft,
    msgFromActorId,
    msgToActorId,
    msgChannel,
    msgTransport,
    msgRoute,
    msgPayload,
    msgIdempotencyKey,
    inboxActorId,
    setBusy,
    setError,
    setChatDraft,
    loadInbox,
    refreshSnapshot,
    refreshEvents,
  } = options;

  const teamMailboxApi = useMemo(() => buildTeamMailboxApiClient(token), [token]);

  const onSendChatMessage = useCallback(async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const fromActorId = chatFromActorId.trim();
    const toActorId = chatToActorId.trim();
    const text = chatDraft.trim();
    if (!fromActorId || !toActorId) {
      setError("Select a valid member conversation first");
      return;
    }
    if (!text) {
      setError("Chat message is required");
      return;
    }

    setBusy("send-chat");
    setError(null);
    try {
      await teamMailboxApi.sendTeamRunMessage(activeRunIdForSelectedTeam, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: "default",
        transport: "local",
        payload: buildMailboxChatPayload(text),
      });
      setChatDraft("");
      await refreshSnapshot(activeRunIdForSelectedTeam);
      await loadInbox(toActorId);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunIdForSelectedTeam,
    chatDraft,
    chatFromActorId,
    chatToActorId,
    loadInbox,
    refreshSnapshot,
    setBusy,
    setChatDraft,
    setError,
    teamMailboxApi,
  ]);

  const onSendMessage = useCallback(async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }

    const fromActorId = msgFromActorId.trim();
    const toActorId = msgToActorId.trim();
    if (!fromActorId || !toActorId) {
      setError("from_actor_id and to_actor_id are required");
      return;
    }

    setBusy("send-message");
    setError(null);
    try {
      await teamMailboxApi.sendTeamRunMessage(activeRunIdForSelectedTeam, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: msgChannel.trim() || undefined,
        transport: msgTransport,
        route: parseOptionalJson(msgRoute, "Message route"),
        payload: parseRequiredJson(msgPayload, "Message payload"),
        idempotency_key: msgIdempotencyKey.trim() || undefined,
      });

      if (tab === "mailbox") {
        await refreshSnapshot(activeRunIdForSelectedTeam);
        if (inboxActorId.trim()) {
          await loadInbox();
        }
      } else {
        await Promise.all([
          refreshEvents(activeRunIdForSelectedTeam),
          refreshSnapshot(activeRunIdForSelectedTeam),
        ]);
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunIdForSelectedTeam,
    inboxActorId,
    loadInbox,
    msgChannel,
    msgFromActorId,
    msgIdempotencyKey,
    msgPayload,
    msgRoute,
    msgToActorId,
    msgTransport,
    refreshEvents,
    refreshSnapshot,
    setBusy,
    setError,
    tab,
    teamMailboxApi,
  ]);

  const onRefreshInbox = useCallback(async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }

    setBusy("refresh-inbox");
    setError(null);
    try {
      await loadInbox();
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [activeRunIdForSelectedTeam, loadInbox, setBusy, setError]);

  const onAckMessage = useCallback(
    async (message: TeamActorMessageRecord) => {
      if (!activeRunIdForSelectedTeam) return;
      const actorId = inboxActorId.trim() || message.to_actor_id;
      setBusy(`ack-${message.message_id}`);
      setError(null);
      try {
        await teamMailboxApi.ackTeamRunMessage(
          activeRunIdForSelectedTeam,
          message.message_id,
          actorId
        );
        if (tab === "mailbox") {
          await Promise.all([loadInbox(actorId), refreshSnapshot(activeRunIdForSelectedTeam)]);
        } else {
          await Promise.all([
            loadInbox(),
            refreshEvents(activeRunIdForSelectedTeam),
            refreshSnapshot(activeRunIdForSelectedTeam),
          ]);
        }
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setBusy(null);
      }
    },
    [
      activeRunIdForSelectedTeam,
      inboxActorId,
      loadInbox,
      refreshEvents,
      refreshSnapshot,
      setBusy,
      setError,
      tab,
      teamMailboxApi,
    ]
  );

  return {
    onSendChatMessage,
    onSendMessage,
    onRefreshInbox,
    onAckMessage,
  };
}
