import { Alert, Badge, Button, Group, Paper, Stack, Text } from "@mantine/core";
import { useCallback, useState } from "react";
import { api } from "../../api";
import type { LoopMemberScope } from "./use_loop_configuration";
import { useLoopPage } from "./use_loop_page";
import { useResumeRefresh } from "./use_resume_refresh";
import {
  loopLabel,
  loopOutcomeLabel,
  loopScheduleLabel,
  loopTime,
} from "./loop_labels";

export function LoopActivationHistory({
  scope,
  refreshKey,
}: {
  scope: LoopMemberScope;
  refreshKey: string | null;
}) {
  const { token, userId, teamId, actorId } = scope;
  const key = JSON.stringify([token, userId, teamId, actorId]);
  const read = useCallback(
    async (cursor: string | null, signal: AbortSignal) => {
      const page = await api.listTeamMemberActivations(
        token,
        teamId,
        actorId,
        cursor,
        signal,
      );
      return { items: page.activations, nextCursor: page.next_cursor };
    },
    [token, teamId, actorId],
  );
  const history = useLoopPage(`${key}:${refreshKey ?? ""}`, read);
  const readSchedules = useCallback(
    async (cursor: string | null, signal: AbortSignal) => {
      const page = await api.listTeamMemberLoopSchedules(
        token,
        teamId,
        actorId,
        cursor,
        signal,
      );
      return { items: page.registrations, nextCursor: page.next_cursor };
    },
    [token, teamId, actorId],
  );
  const schedules = useLoopPage(key, readSchedules);
  const [selected, setSelected] = useState<{
    scope: string;
    id: string;
  } | null>(null);
  const selectedId = selected?.scope === key ? selected.id : null;
  const refreshHistory = history.refresh;
  const refreshSchedules = schedules.refresh;
  const refresh = useCallback(async () => {
    await Promise.all([refreshHistory(), refreshSchedules()]);
  }, [refreshHistory, refreshSchedules]);
  const viewingOlder =
    history.state?.viewingOlder || schedules.state?.viewingOlder;
  useResumeRefresh({
    enabled: !viewingOlder && !selectedId,
    intervalMs: 3000,
    pauseWhenHidden: true,
    refresh,
  });
  const activeSchedules =
    schedules.state?.items.filter((item) => item.state === "active") ?? [];
  const pending =
    history.state?.items.filter((item) => item.state === "pending") ?? [];

  return (
    <Stack gap="md">
      <Paper withBorder p="md">
        <Stack gap="xs">
          <Text fw={600}>Recorded wakes and waits</Text>
          <Text size="sm" c="dimmed">
            Execution settings and limits determine when queued work can start.
          </Text>
          {pending.map((item) => (
            <Text key={item.id} size="sm">
              Queued work: eligible from{" "}
              {loopTime(Math.max(item.due_at, item.next_admission_at))}
            </Text>
          ))}
          {activeSchedules.map((item) => (
            <Text key={item.id} size="sm" className="break-words">
              {loopScheduleLabel(item)}
            </Text>
          ))}
          {(!history.state ||
            !schedules.state ||
            history.state.loading ||
            schedules.state.loading) && (
            <Text size="sm">Loading wake records...</Text>
          )}
          {history.state &&
            schedules.state &&
            !history.state.loading &&
            !schedules.state.loading &&
            !history.state.error &&
            !schedules.state.error &&
            !pending.length &&
            !activeSchedules.length && (
              <Text size="sm">No upcoming wake in the loaded records.</Text>
            )}
          {(history.state?.nextCursor != null ||
            schedules.state?.nextCursor != null) && (
            <Text size="xs" c="dimmed">
              More records are available; this list may omit earlier pending
              work.
            </Text>
          )}
          {schedules.state?.error && (
            <Alert color="red">{schedules.state.error}</Alert>
          )}
          {schedules.state?.nextCursor != null && (
            <Button
              variant="subtle"
              size="xs"
              loading={schedules.state.loading}
              onClick={() => void schedules.loadMore()}
            >
              Load more wake records
            </Button>
          )}
        </Stack>
      </Paper>
      <Paper withBorder p="md">
        <Stack gap="sm">
          <Group justify="space-between">
            <Text fw={600}>Activation history</Text>
            <Button
              variant="subtle"
              size="xs"
              loading={history.state?.loading}
              onClick={() => void refresh()}
            >
              Refresh history
            </Button>
          </Group>
          <Text size="sm" c="dimmed">
            Outcomes remain available after the process exits. Task progress is
            recorded separately.
          </Text>
          {history.state?.error && (
            <Alert color="red">{history.state.error}</Alert>
          )}
          {history.state &&
            !history.state.loading &&
            !history.state.items.length &&
            !history.state.error && <Text size="sm">No activations yet.</Text>}
          {history.state?.items.map((item) => (
            <Paper key={item.id} withBorder p="sm">
              <Group justify="space-between" align="start">
                <Stack gap={4}>
                  <Group gap="xs">
                    <Badge variant="light">{item.state}</Badge>
                    <Text size="sm">{loopTime(item.created_at)}</Text>
                  </Group>
                  <Text size="sm">{loopOutcomeLabel(item)}</Text>
                  {item.outcome?.continuation && (
                    <Text size="xs" c="dimmed">
                      Recorded continuation request:{" "}
                      {loopTime(item.outcome.continuation.due_at)}. See current
                      wakes for execution eligibility.
                    </Text>
                  )}
                </Stack>
                <Button
                  variant="subtle"
                  size="xs"
                  onClick={() => setSelected({ scope: key, id: item.id })}
                >
                  Inspect activation
                </Button>
              </Group>
              {selectedId === item.id && (
                <LoopActivationDetail
                  key={`${key}:${item.id}`}
                  scope={scope}
                  activationId={item.id}
                  onClose={() => setSelected(null)}
                />
              )}
            </Paper>
          ))}
          {viewingOlder && (
            <Text size="xs" c="dimmed">
              Automatic refresh pauses while viewing older records. Refresh
              history to return to the latest page.
            </Text>
          )}
          {history.state?.nextCursor != null && (
            <Button
              variant="light"
              loading={history.state.loading}
              onClick={() => void history.loadMore()}
            >
              Load older activations
            </Button>
          )}
        </Stack>
      </Paper>
    </Stack>
  );
}

function LoopActivationDetail({
  scope,
  activationId,
  onClose,
}: {
  scope: LoopMemberScope;
  activationId: string;
  onClose: () => void;
}) {
  const { token, teamId, actorId } = scope;
  const key = JSON.stringify([token, teamId, actorId, activationId]);
  const readSources = useCallback(
    async (cursor: string | null, signal: AbortSignal) => {
      const page = await api.listTeamMemberActivationSources(
        token,
        teamId,
        actorId,
        activationId,
        cursor,
        signal,
      );
      return { items: page.sources, nextCursor: page.next_cursor };
    },
    [token, teamId, actorId, activationId],
  );
  const readEvents = useCallback(
    async (cursor: number | null, signal: AbortSignal) => {
      const page = await api.listTeamMemberActivationEvents(
        token,
        teamId,
        actorId,
        activationId,
        cursor,
        signal,
      );
      return { items: page.events, nextCursor: page.next_cursor };
    },
    [token, teamId, actorId, activationId],
  );
  const readTools = useCallback(
    async (cursor: number | null, signal: AbortSignal) => {
      const page = await api.listTeamMemberActivationTools(
        token,
        teamId,
        actorId,
        activationId,
        cursor,
        signal,
      );
      return { items: page.tools, nextCursor: page.next_cursor };
    },
    [token, teamId, actorId, activationId],
  );
  const sources = useLoopPage(key, readSources);
  const events = useLoopPage(key, readEvents);
  const tools = useLoopPage(key, readTools);
  return (
    <Stack gap="xs" mt="sm">
      <Group justify="space-between">
        <Text fw={500} size="sm">
          Activation details
        </Text>
        <Button size="xs" variant="subtle" onClick={onClose}>
          Close details
        </Button>
      </Group>
      <Text size="xs" c="dimmed">
        Trigger sources
      </Text>
      {sources.state?.items.map((source) => (
        <Stack key={source.id} gap={2}>
          <Text size="sm">
            {loopLabel(source.kind)} · {loopTime(source.created_at)}
            {source.revoked ? " · revoked" : ""}
          </Text>
          {source.references.app_id && source.references.app_event && (
            <Text size="xs" c="dimmed" className="break-words">
              App {source.references.app_id} ·{" "}
              {source.references.app_event.event_class}
              {" · Event "}
              {source.references.app_event.event_id}
              {" · Cursor "}
              {source.references.app_event.cursor}
              {" · Version "}
              {source.references.app_event.version}
            </Text>
          )}
        </Stack>
      ))}
      {sources.state?.error && <Alert color="red">{sources.state.error}</Alert>}
      {sources.state?.nextCursor != null && (
        <Button
          size="xs"
          variant="subtle"
          loading={sources.state.loading}
          onClick={() => void sources.loadMore()}
        >
          More trigger sources
        </Button>
      )}
      <Text size="xs" c="dimmed">
        Lifecycle events
      </Text>
      {events.state?.items.map((event) => (
        <Text key={event.id} size="sm">
          {loopLabel(event.kind)} · {loopTime(event.created_at)}
          {event.reason ? ` · ${loopLabel(event.reason)}` : ""}
          {event.exit_reason ? ` · ${loopLabel(event.exit_reason)}` : ""}
        </Text>
      ))}
      {events.state?.error && <Alert color="red">{events.state.error}</Alert>}
      {events.state?.nextCursor != null && (
        <Button
          size="xs"
          variant="subtle"
          loading={events.state.loading}
          onClick={() => void events.loadMore()}
        >
          More lifecycle events
        </Button>
      )}
      <Text size="xs" c="dimmed">
        Tool observations
      </Text>
      {tools.state?.items.map((tool) => (
        <Text key={tool.id} size="sm">
          {tool.tool_name}: {loopLabel(tool.status)}
          {tool.duration_ms == null ? "" : ` · ${tool.duration_ms} ms`}
        </Text>
      ))}
      {tools.state &&
        !tools.state.loading &&
        !tools.state.items.length &&
        !tools.state.error && (
          <Text size="sm">No tool observations recorded.</Text>
        )}
      {tools.state?.error && <Alert color="red">{tools.state.error}</Alert>}
      {tools.state?.nextCursor != null && (
        <Button
          size="xs"
          variant="subtle"
          loading={tools.state.loading}
          onClick={() => void tools.loadMore()}
        >
          More tool observations
        </Button>
      )}
    </Stack>
  );
}
