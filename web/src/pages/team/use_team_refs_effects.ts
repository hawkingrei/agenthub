import { useEffect } from "react";
import type { MutableRefObject } from "react";
import type { AgentEvent, TeamRunEventRecord } from "../../api";

type UseTeamRefsEffectsParams = {
  events: TeamRunEventRecord[];
  activeRunId: string | null;
  memberEvents: AgentEvent[];
  eventsRef: MutableRefObject<TeamRunEventRecord[]>;
  activeRunIdRef: MutableRefObject<string | null>;
  memberEventsRef: MutableRefObject<AgentEvent[]>;
};

export function useTeamRefsEffects({
  events,
  activeRunId,
  memberEvents,
  eventsRef,
  activeRunIdRef,
  memberEventsRef,
}: UseTeamRefsEffectsParams) {
  useEffect(() => {
    eventsRef.current = events;
  }, [events, eventsRef]);

  useEffect(() => {
    activeRunIdRef.current = activeRunId;
  }, [activeRunId, activeRunIdRef]);

  useEffect(() => {
    memberEventsRef.current = memberEvents;
  }, [memberEvents, memberEventsRef]);
}
