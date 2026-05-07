import { useCallback } from "react";
import type { TeamMemberSnapshot, TeamRunRecord } from "../../api";
import type { TeamTab } from "./state";

type UseTeamPanelActionsParams = {
  activeRunId: string | null;
  activeRun: TeamRunRecord | null;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  setError: (value: string | null) => void;
  parseErrorMessage: (error: unknown) => string;
  loadMemberEvents: (
    mode?: "replace" | "prepend",
    sessionIdOverride?: string | null
  ) => Promise<void>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<void>;
  refreshSnapshot: (runId: string) => Promise<unknown>;
  setSelectedMemberId: (memberId: string) => void;
  setTab: (tab: TeamTab) => void;
};

export function useTeamPanelActions({
  activeRunId,
  activeRun,
  selectedMemberSnapshot,
  setError,
  parseErrorMessage,
  loadMemberEvents,
  refreshEvents,
  refreshSnapshot,
  setSelectedMemberId,
  setTab,
}: UseTeamPanelActionsParams) {
  const onRefreshMemberConsole = useCallback(async () => {
    if (selectedMemberSnapshot) {
      await loadMemberEvents("replace");
      return;
    }
    if (activeRunId) {
      await refreshEvents(activeRunId);
    }
  }, [activeRunId, loadMemberEvents, refreshEvents, selectedMemberSnapshot]);

  const onLoadOlderMemberConsole = useCallback(async () => {
    if (!selectedMemberSnapshot) {
      return;
    }
    await loadMemberEvents("prepend");
  }, [loadMemberEvents, selectedMemberSnapshot]);

  const onRefreshOverviewSnapshot = useCallback(async () => {
    if (!activeRunId) return;
    setError(null);
    try {
      await refreshSnapshot(activeRunId);
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRunId, parseErrorMessage, refreshSnapshot, setError]);

  const onOpenMailboxForMember = useCallback(
    (memberId: string) => {
      setSelectedMemberId(memberId);
      setTab("mailbox");
    },
    [setSelectedMemberId, setTab]
  );

  const onRefreshEventsPanel = useCallback(async () => {
    if (!activeRun) return;
    setError(null);
    try {
      await refreshEvents(activeRun.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRun, parseErrorMessage, refreshEvents, setError]);

  const onLoadOlderEventsPanel = useCallback(async () => {
    if (!activeRun) return;
    setError(null);
    try {
      await refreshEvents(activeRun.id, "prepend");
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [activeRun, parseErrorMessage, refreshEvents, setError]);

  return {
    onRefreshMemberConsole,
    onLoadOlderMemberConsole,
    onRefreshOverviewSnapshot,
    onOpenMailboxForMember,
    onRefreshEventsPanel,
    onLoadOlderEventsPanel,
  };
}
