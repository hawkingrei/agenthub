import {
  Alert,
  Badge,
  Button,
  Group,
  NumberInput,
  Paper,
  Select,
  Stack,
  Text,
} from "@mantine/core";
import { useCallback, useEffect, useState } from "react";
import {
  api,
  stringifyApiError,
  type TeamDefinitionRecord,
  type TeamTaskRecord,
} from "../../api";
import type {
  LoopConfigurationUpdate,
  LoopPolicy,
  LoopSessionPolicy,
} from "../../loop_types";
import type { AuthState } from "../../types";
import { buildTeamMemberDraftFromSpec } from "./create_helpers";
import { LoopActivationHistory } from "./loop_activation_history";
import { loopLabel, loopPreflightMessages } from "./loop_labels";
import { useLoopConfiguration } from "./use_loop_configuration";
import { useResumeRefresh } from "./use_resume_refresh";

type Props = {
  auth: AuthState;
  team: TeamDefinitionRecord;
  actorId: string;
  label: string;
  processStatus: string | null;
  tasks: TeamTaskRecord[];
  tasksLoading: boolean;
  onEdit: () => void;
  onTeamUpdated: (team: TeamDefinitionRecord) => void;
  onOpenTask: (taskId: string) => void;
  onOpenDiagnostics: () => void;
  onClose?: () => void;
};

export function TeamLoopMemberPanel({
  auth,
  team,
  actorId,
  label,
  processStatus,
  tasks,
  tasksLoading,
  onEdit,
  onTeamUpdated,
  onOpenTask,
  onOpenDiagnostics,
  onClose,
}: Props) {
  const scope = {
    token: auth.token,
    userId: auth.userId,
    teamId: team.id,
    actorId,
  };
  const control = useLoopConfiguration(scope);
  const spec =
    team.spec && typeof team.spec === "object" && !Array.isArray(team.spec)
      ? (team.spec as Record<string, unknown>)
      : {};
  const loopMode = spec.execution_mode === "loop";
  const profile = buildTeamMemberDraftFromSpec(team.spec, actorId);
  const accessScope = JSON.stringify([
    auth.token,
    auth.userId,
    auth.role,
    team.id,
  ]);
  const [access, setAccess] = useState<{
    scope: string;
    allowed: boolean;
  } | null>(null);
  const [modeBusy, setModeBusy] = useState(false);
  const [modeError, setModeError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const policy = control.state?.configuration?.policy;
  const canOperate = access?.scope === accessScope && access.allowed;
  const configuration = control.state?.configuration;
  const busy = Boolean(control.state?.busy || modeBusy);
  const writable = canOperate && !busy && !control.state?.stale;
  const assignedTasks = tasks.filter(
    (task) => task.assigned_member_id === actorId,
  );

  useEffect(() => {
    let active = true;
    void api
      .listTeamspaceMembers(auth.token, team.id)
      .then((members) => {
        if (active)
          setAccess({
            scope: accessScope,
            allowed:
              ["root", "admin", "operator"].includes(auth.role) &&
              (!members.some((member) => member.user_id === auth.userId) ||
                members.some(
                  (member) =>
                    member.user_id === auth.userId && member.role === "owner",
                )),
          });
      })
      .catch(() => {
        if (active) setAccess({ scope: accessScope, allowed: false });
      });
    return () => {
      active = false;
    };
  }, [accessScope, auth.token, auth.userId, auth.role, team.id]);

  useResumeRefresh({
    enabled: !editing,
    intervalMs: 5000,
    pauseWhenHidden: true,
    refresh: control.refresh,
  });

  const changeMode = async () => {
    if (!canOperate || modeBusy || loopMode) return;
    setModeBusy(true);
    setModeError(null);
    try {
      const updated = await api.updateTeamSpec(auth.token, team.id, {
        expected_updated_at: team.updated_at,
        spec: { ...spec, execution_mode: "loop" },
      });
      onTeamUpdated(updated);
      await control.refresh();
    } catch (error) {
      setModeError(stringifyApiError(error));
      // A lost response may still have committed the mode change.
      try {
        onTeamUpdated(await api.getTeam(auth.token, team.id));
      } catch {
        /* Retain the original error. */
      }
    } finally {
      setModeBusy(false);
    }
  };
  const setPolicy = (state: LoopPolicy["state"]) => {
    if (!policy || !writable) return;
    void control.configure({
      expected_revision: policy.revision,
      state,
      session_policy: policy.session_policy,
      limits: policy.limits,
    });
  };

  return (
    <Stack
      gap="md"
      className="min-w-0 overflow-auto p-1"
      aria-label={`${label} durable work`}
    >
      <Paper withBorder p="md">
        <Stack gap="sm">
          <Group justify="space-between" align="start">
            <div>
              <Text size="sm" c="dimmed">
                Agent Profile
              </Text>
              <Text component="h2" fw={600} size="lg">
                {label === actorId ? label : `${label} ${actorId}`}
              </Text>
              <Text size="sm" c="dimmed">
                Member configuration and work
              </Text>
            </div>
            {onClose && (
              <Button
                size="xs"
                variant="subtle"
                aria-label="Close agent profile"
                onClick={onClose}
              >
                Close profile
              </Button>
            )}
          </Group>
          {profile && (
            <>
              <Text size="sm">
                Role: {profile.role}. Model: {profile.model || "Default"}.
              </Text>
              {profile.description && (
                <Text size="sm" className="whitespace-pre-wrap break-words">
                  {profile.description}
                </Text>
              )}
            </>
          )}
          <Group gap="xs">
            <Badge
              variant="light"
              color={policy?.state === "enabled" ? "green" : "gray"}
            >
              Execution: {policy?.state ?? (loopMode ? "loading" : "manual")}
            </Badge>
            <Badge variant="outline" color="gray">
              Process: {processStatus || "unknown"}
            </Badge>
          </Group>
          <Text size="sm">
            Configuration, tasks, messages, and activation history remain
            available while the process is stopped.
          </Text>
          <Group gap="xs">
            <Button
              size="xs"
              variant="light"
              disabled={!canOperate || busy}
              onClick={onEdit}
            >
              Edit profile
            </Button>
            <Button size="xs" variant="subtle" onClick={onOpenDiagnostics}>
              Process diagnostics
            </Button>
          </Group>
          {!canOperate && access && (
            <Text size="xs" c="dimmed">
              Execution changes require a Team owner with runtime access.
            </Text>
          )}
          {modeError && <Alert color="red">{modeError}</Alert>}
          {control.state?.error && (
            <Alert color="red">{control.state.error}</Alert>
          )}
          {control.state?.actionError && (
            <Alert color="red">{control.state.actionError}</Alert>
          )}
          {!loopMode ? (
            <Stack gap="xs">
              <Text size="sm">
                Choose durable execution to let queued work start this Team's
                members. Each member starts disabled and requires explicit
                enablement.
              </Text>
              <Button
                size="sm"
                variant="light"
                disabled={!canOperate}
                loading={modeBusy}
                onClick={() => void changeMode()}
              >
                Use durable execution
              </Button>
            </Stack>
          ) : (
            <>
              {configuration?.preflight.ready && (
                <Text size="sm" c="teal">
                  Configuration ready
                </Text>
              )}
              {configuration &&
                (!configuration.preflight.ready ||
                  loopPreflightMessages(configuration).length > 0) && (
                  <Alert
                    color={configuration.preflight.ready ? "green" : "yellow"}
                    title={
                      configuration.preflight.ready
                        ? "Configuration ready"
                        : "Configuration needs attention"
                    }
                  >
                    {loopPreflightMessages(configuration).map((message) => (
                      <Text key={message} size="sm">
                        {message}
                      </Text>
                    ))}
                    {!configuration.preflight.ready && (
                      <Button
                        size="xs"
                        variant="subtle"
                        loading={control.state?.loading}
                        onClick={() => void control.refresh()}
                      >
                        Check again
                      </Button>
                    )}
                  </Alert>
                )}
              {policy && (
                <>
                  <Text size="sm">
                    Session policy:{" "}
                    {policy.session_policy === "fresh"
                      ? "Fresh session for each activation"
                      : "Resume provider context when supported"}
                    .
                  </Text>
                  <Group gap="xs">
                    {policy.state === "enabled" ? (
                      <Button
                        size="sm"
                        variant="light"
                        disabled={!writable}
                        onClick={() => setPolicy("suspended")}
                      >
                        Suspend execution
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        disabled={!writable || !configuration?.preflight.ready}
                        onClick={() => setPolicy("enabled")}
                      >
                        {policy.state === "suspended"
                          ? "Resume execution"
                          : "Enable execution"}
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="light"
                      disabled={!writable || policy.state === "disabled"}
                      loading={control.state?.busy === "activate"}
                      onClick={() => void control.activate()}
                    >
                      {control.state?.retryPending
                        ? "Retry activation request"
                        : "Activate member"}
                    </Button>
                    <Button
                      size="xs"
                      variant="subtle"
                      disabled={!writable}
                      onClick={() => setEditing((open) => !open)}
                    >
                      {editing ? "Close settings" : "Execution settings"}
                    </Button>
                  </Group>
                  <Text size="xs" c="dimmed">
                    Suspension pauses new execution and keeps queued work. A
                    running activation can finish.
                  </Text>
                  {control.state?.retryPending && !control.state.busy && (
                    <Text size="sm">
                      The request has no confirmed receipt. Retry uses the same
                      request identity.
                    </Text>
                  )}
                  {control.state?.receipt && (
                    <Text size="sm" role="status">
                      Activation request accepted
                      {control.state.receipt.duplicate
                        ? " (existing request)"
                        : ""}
                      . Queued work follows the current execution settings.
                    </Text>
                  )}
                  {editing && (
                    <LoopPolicyEditor
                      key={policy.revision}
                      policy={policy}
                      disabled={!writable}
                      onSave={control.configure}
                    />
                  )}
                </>
              )}
            </>
          )}
        </Stack>
      </Paper>
      <Paper withBorder p="md">
        <Stack gap="xs">
          <Text fw={600}>Assigned tasks</Text>
          <Text size="sm" c="dimmed">
            Task progress is tracked separately from process and activation
            status.
          </Text>
          {tasksLoading ? (
            <Text size="sm">Loading tasks...</Text>
          ) : assignedTasks.length === 0 ? (
            <Text size="sm">
              No assigned tasks in the loaded workspace records.
            </Text>
          ) : (
            assignedTasks.map((task) => (
              <Group key={task.id} justify="space-between">
                <Button
                  variant="subtle"
                  size="sm"
                  onClick={() => onOpenTask(task.id)}
                >
                  {task.title}
                </Button>
                <Badge variant="light">{loopLabel(task.status)}</Badge>
              </Group>
            ))
          )}
        </Stack>
      </Paper>
      {loopMode && (
        <LoopActivationHistory
          scope={scope}
          refreshKey={control.state?.receipt?.activation_id ?? null}
        />
      )}
    </Stack>
  );
}

function LoopPolicyEditor({
  policy,
  disabled,
  onSave,
}: {
  policy: LoopPolicy;
  disabled: boolean;
  onSave: (update: LoopConfigurationUpdate) => Promise<void>;
}) {
  const [session, setSession] = useState<LoopSessionPolicy>(
    policy.session_policy,
  );
  const [budget, setBudget] = useState<string | number>(
    policy.limits.activations_per_actor,
  );
  const [windowSeconds, setWindowSeconds] = useState<string | number>(
    policy.limits.window_seconds,
  );
  const valid =
    typeof budget === "number" &&
    Number.isInteger(budget) &&
    budget >= 1 &&
    budget <= policy.limits.activations_per_team &&
    typeof windowSeconds === "number" &&
    Number.isInteger(windowSeconds) &&
    windowSeconds >= 1 &&
    windowSeconds <= 86400;
  const save = useCallback(() => {
    if (disabled || !valid) return;
    void onSave({
      expected_revision: policy.revision,
      state: policy.state,
      session_policy: session,
      limits: {
        ...policy.limits,
        activations_per_actor: budget,
        window_seconds: windowSeconds,
      },
    });
  }, [disabled, valid, onSave, policy, session, budget, windowSeconds]);
  return (
    <Stack gap="xs">
      <Select
        label="Session policy"
        value={session}
        data={[
          { value: "fresh", label: "Fresh (default)" },
          { value: "resume", label: "Resume when supported" },
        ]}
        disabled={disabled}
        allowDeselect={false}
        onChange={(value) => {
          if (value === "fresh" || value === "resume") setSession(value);
        }}
      />
      <NumberInput
        label="Activations per window"
        value={budget}
        min={1}
        max={policy.limits.activations_per_team}
        allowDecimal={false}
        disabled={disabled}
        onChange={setBudget}
      />
      <NumberInput
        label="Window in seconds"
        value={windowSeconds}
        min={1}
        max={86400}
        allowDecimal={false}
        disabled={disabled}
        onChange={setWindowSeconds}
      />
      <Button size="sm" disabled={disabled || !valid} onClick={save}>
        Save execution settings
      </Button>
    </Stack>
  );
}
