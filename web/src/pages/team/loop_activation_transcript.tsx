import { Alert, Button, Group, Stack, Text } from "@mantine/core";
import { useCallback, useMemo, useRef } from "react";
import { buildAcpView } from "../../acp";
import { api, type AgentEvent } from "../../api";
import { AcpConversation } from "../../components/acp_conversation";
import { buildConversationMessages } from "../../conversation";
import {
  omitIncompleteLeadingAcpMessageEvents,
  resolveAdaptiveAcpHistoryPageLimit,
  resolveInitialAcpHistoryDecision,
} from "./acp_history_prefetch";
import type { LoopMemberScope } from "./use_loop_configuration";
import { useLoopPage } from "./use_loop_page";

const PAGE_LIMIT = 100;
const noop = () => {};
const ansi = (input: string) => input;

export function LoopActivationTranscript({
  scope,
  activationId,
  sessionId,
}: {
  scope: LoopMemberScope;
  activationId: string;
  sessionId: string;
}) {
  const { token, userId, teamId, actorId } = scope;
  const key = JSON.stringify([token, userId, teamId, actorId, activationId, sessionId]);
  const read = useCallback(async (cursor: number | null, signal: AbortSignal) => {
    const page = async (before: number | null, limit: number) => {
      const events = await api.listAgentEvents(token, actorId, limit, sessionId, before, signal);
      if (events.some(event => event.agent_id !== actorId || event.session_id !== sessionId)) {
        throw new Error("The transcript response does not match the selected session.");
      }
      return {
        items: events,
        nextCursor: events.length >= limit ? Math.min(...events.map(event => event.event_id)) : null,
      };
    };
    let result = await page(cursor, PAGE_LIMIT);
    const decision = resolveInitialAcpHistoryDecision(result.items, sessionId, result.nextCursor != null, 1);
    // Recover at most one older page before rendering a partial leading response.
    // Further history reads require an explicit user action.
    if (cursor == null && result.nextCursor != null && decision.shouldPrefetchInitialHistory) {
      const older = await page(result.nextCursor, resolveAdaptiveAcpHistoryPageLimit(result.items, sessionId, PAGE_LIMIT));
      result = { items: [...older.items, ...result.items], nextCursor: older.nextCursor };
    }
    return { ...result, items: result.items.map(event => ({ ...event, id: event.event_id })) };
  }, [token, actorId, sessionId]);
  const history = useLoopPage(key, read);
  const events = useMemo<AgentEvent[]>(
    () => [...(history.state?.items ?? [])].sort((a, b) => a.event_id - b.event_id),
    [history.state?.items],
  );
  const { items, partial } = useMemo(() => {
    const visible = omitIncompleteLeadingAcpMessageEvents(events, sessionId);
    const view = buildAcpView(visible);
    return {
      items: buildConversationMessages(view.messages, view.toolCalls, view.plan, sessionId),
      partial: visible.length !== events.length,
    };
  }, [events, sessionId]);
  const container = useRef<HTMLDivElement>(null);

  return (
    <Stack gap="sm" className="h-[calc(100dvh-7rem)] min-h-0">
      <Text size="sm" c="dimmed">Read-only conversation recorded for this activation.</Text>
      <Group gap="xs">
        <Button size="xs" variant="subtle" loading={history.state?.loading} onClick={() => void history.refresh()}>
          Refresh transcript
        </Button>
        {history.state?.nextCursor != null && (
          <Button size="xs" variant="subtle" loading={history.state.loading} onClick={() => void history.loadMore()}>
            Load older transcript
          </Button>
        )}
      </Group>
      {history.state?.error && <Alert color="red">{history.state.error}</Alert>}
      {(!history.state || history.state.loading) && <Text size="sm">Loading transcript...</Text>}
      {partial && <Text size="sm" c="dimmed">An earlier response is incomplete in the loaded history.</Text>}
      {history.state && !history.state.loading && !history.state.error && !items.length && !partial && (
        <Text size="sm">No conversation recorded for this activation.</Text>
      )}
      <AcpConversation
        items={items}
        windowOffset={0}
        isFrozenView
        shouldAutoCollapse
        collapseCutoff={items.length}
        toolCallsDefaultCollapsed
        runStatus="exited"
        virtualTopSpacer={0}
        virtualBottomSpacer={0}
        stickToBottom={false}
        pendingCount={0}
        avgHeight={64}
        onScroll={noop}
        containerRef={container}
        ansi={ansi}
      />
    </Stack>
  );
}
