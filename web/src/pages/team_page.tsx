import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  TeamActorMessageRecord,
  TeamDefinitionRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamStepRecord,
} from "../api";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";

type TeamPageProps = {
  auth: AuthState;
  token: string;
  onLogout: () => void;
};

type TeamTab = "events" | "steps" | "messages";
type StepAction =
  | "start"
  | "complete"
  | "fail"
  | "input_required"
  | "resume";

const EVENT_PAGE_LIMIT = 100;
const DEFAULT_TEAM_SPEC = `{
  "spec_version": 1,
  "entrypoint": "planner",
  "members": [
    {
      "member_id": "planner"
    }
  ]
}`;

function sortRuns(runs: TeamRunRecord[]): TeamRunRecord[] {
  return [...runs].sort((a, b) => b.created_at - a.created_at);
}

function upsertRun(list: TeamRunRecord[], nextRun: TeamRunRecord): TeamRunRecord[] {
  const withoutCurrent = list.filter((run) => run.id !== nextRun.id);
  return sortRuns([nextRun, ...withoutCurrent]);
}

function upsertEventList(
  prev: TeamRunEventRecord[],
  next: TeamRunEventRecord[],
  mode: "replace" | "prepend"
): TeamRunEventRecord[] {
  const merged = mode === "replace" ? [...next] : [...next, ...prev];
  const byId = new Map<number, TeamRunEventRecord>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return [...byId.values()].sort((a, b) => a.event_id - b.event_id);
}

function parseErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    const msg = err.message ?? "request failed";
    if (!msg.trim().startsWith("{")) {
      return msg;
    }
    try {
      const parsed = JSON.parse(msg) as { error?: string };
      if (typeof parsed.error === "string" && parsed.error) {
        return parsed.error;
      }
      return msg;
    } catch {
      return msg;
    }
  }
  return String(err);
}

function parseRequiredJson(raw: string, field: string): unknown {
  const trimmed = raw.trim();
  if (!trimmed) {
    throw new Error(`${field} is required`);
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    throw new Error(`${field} must be valid JSON`);
  }
}

function parseOptionalJson(raw: string, field: string): unknown | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    throw new Error(`${field} must be valid JSON`);
  }
}

function parseOptionalInteger(raw: string, field: string): number | undefined {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    throw new Error(`${field} must be a non-negative integer`);
  }
  return parsed;
}

function parseCsvList(raw: string): string[] {
  return raw
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function formatTs(ts?: number | null): string {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

function toPrettyJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function TeamPage(props: TeamPageProps) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const [tab, setTab] = useState<TeamTab>("events");
  const [teams, setTeams] = useState<TeamDefinitionRecord[]>([]);
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null);

  const [newTeamName, setNewTeamName] = useState("");
  const [newTeamDescription, setNewTeamDescription] = useState("");
  const [newTeamSpec, setNewTeamSpec] = useState(DEFAULT_TEAM_SPEC);

  const [runContextId, setRunContextId] = useState("");
  const [runInput, setRunInput] = useState("{}");
  const [runLookupId, setRunLookupId] = useState("");

  const [runs, setRuns] = useState<TeamRunRecord[]>([]);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);

  const [events, setEvents] = useState<TeamRunEventRecord[]>([]);
  const [eventsHasMore, setEventsHasMore] = useState(false);
  const [eventsLoading, setEventsLoading] = useState(false);
  const [eventsAutoRefresh, setEventsAutoRefresh] = useState(true);

  const [steps, setSteps] = useState<TeamStepRecord[]>([]);
  const [stepKey, setStepKey] = useState("");
  const [stepMemberId, setStepMemberId] = useState("");
  const [stepDependsOn, setStepDependsOn] = useState("");
  const [stepInput, setStepInput] = useState("{}");

  const [selectedStepId, setSelectedStepId] = useState<string>("");
  const [stepAction, setStepAction] = useState<StepAction>("start");
  const [stepRemoteTaskId, setStepRemoteTaskId] = useState("");
  const [stepOutput, setStepOutput] = useState("{}");
  const [stepFailText, setStepFailText] = useState("");
  const [stepInputReason, setStepInputReason] = useState("");
  const [stepInputRequiredPayload, setStepInputRequiredPayload] = useState("{}");
  const [stepResumePayload, setStepResumePayload] = useState("{}");

  const [msgFromActorId, setMsgFromActorId] = useState("");
  const [msgToActorId, setMsgToActorId] = useState("");
  const [msgChannel, setMsgChannel] = useState("default");
  const [msgTransport, setMsgTransport] = useState<"local" | "remote">("local");
  const [msgRoute, setMsgRoute] = useState("");
  const [msgPayload, setMsgPayload] = useState("{}");
  const [msgIdempotencyKey, setMsgIdempotencyKey] = useState("");

  const [inboxActorId, setInboxActorId] = useState("");
  const [inboxLimit, setInboxLimit] = useState("100");
  const [inboxAfterId, setInboxAfterId] = useState("");
  const [inboxIncludeDelivered, setInboxIncludeDelivered] = useState(false);
  const [inbox, setInbox] = useState<TeamActorMessageRecord[]>([]);

  const selectedTeam = useMemo(
    () => teams.find((team) => team.id === selectedTeamId) ?? null,
    [teams, selectedTeamId]
  );

  const activeRun = useMemo(
    () => runs.find((run) => run.id === activeRunId) ?? null,
    [runs, activeRunId]
  );

  const visibleRuns = useMemo(() => {
    if (!selectedTeamId) return [];
    return runs.filter((run) => run.team_id === selectedTeamId);
  }, [runs, selectedTeamId]);

  const oldestEventId = events.length > 0 ? events[0].event_id : null;

  const refreshTeams = useCallback(async () => {
    setBusy("refresh-teams");
    setError(null);
    try {
      const list = await api.listTeams(props.token);
      setTeams(list);
      setSelectedTeamId((prev) => {
        if (prev && list.some((team) => team.id === prev)) {
          return prev;
        }
        return list[0]?.id ?? null;
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [props.token]);

  const refreshRun = useCallback(
    async (runId: string) => {
      const run = await api.getTeamRun(props.token, runId);
      setRuns((prev) => upsertRun(prev, run));
      return run;
    },
    [props.token]
  );

  const refreshSteps = useCallback(
    async (runId: string) => {
      const list = await api.listTeamRunSteps(props.token, runId);
      setSteps(list);
      setSelectedStepId((prev) => {
        if (prev && list.some((step) => step.id === prev)) {
          return prev;
        }
        return list[0]?.id ?? "";
      });
      return list;
    },
    [props.token]
  );

  const refreshEvents = useCallback(
    async (runId: string, mode: "replace" | "prepend" = "replace") => {
      setEventsLoading(true);
      try {
        const beforeId = mode === "prepend" ? oldestEventId ?? undefined : undefined;
        const list = await api.listTeamRunEvents(
          props.token,
          runId,
          EVENT_PAGE_LIMIT,
          beforeId
        );
        setEvents((prev) => upsertEventList(prev, list, mode));
        setEventsHasMore(list.length >= EVENT_PAGE_LIMIT);
      } finally {
        setEventsLoading(false);
      }
    },
    [oldestEventId, props.token]
  );

  const loadInbox = useCallback(async () => {
    if (!activeRunId) return;
    const actorId = inboxActorId.trim();
    if (!actorId) {
      throw new Error("Inbox actor_id is required");
    }
    const limit = parseOptionalInteger(inboxLimit, "Inbox limit") ?? 100;
    const afterId = parseOptionalInteger(inboxAfterId, "Inbox after_id");
    const list = await api.listTeamRunInbox(props.token, activeRunId, {
      actor_id: actorId,
      limit,
      after_id: afterId,
      include_delivered: inboxIncludeDelivered,
    });
    setInbox(list);
  }, [
    activeRunId,
    inboxActorId,
    inboxAfterId,
    inboxIncludeDelivered,
    inboxLimit,
    props.token,
  ]);

  useEffect(() => {
    void refreshTeams();
  }, [refreshTeams]);

  useEffect(() => {
    if (!selectedTeamId) {
      setActiveRunId(null);
      setRuns([]);
      setEvents([]);
      setSteps([]);
      setInbox([]);
      return;
    }
    setActiveRunId((prev) => {
      if (prev && runs.some((run) => run.id === prev && run.team_id === selectedTeamId)) {
        return prev;
      }
      return runs.find((run) => run.team_id === selectedTeamId)?.id ?? null;
    });
  }, [selectedTeamId, runs]);

  useEffect(() => {
    if (!activeRunId) {
      setEvents([]);
      setSteps([]);
      setInbox([]);
      return;
    }
    let canceled = false;
    const loadAll = async () => {
      try {
        setError(null);
        const run = await refreshRun(activeRunId);
        if (canceled) return;
        if (run.team_id !== selectedTeamId) {
          setSelectedTeamId(run.team_id);
        }
        await Promise.all([refreshSteps(activeRunId), refreshEvents(activeRunId)]);
      } catch (err) {
        if (!canceled) {
          setError(parseErrorMessage(err));
        }
      }
    };
    void loadAll();
    return () => {
      canceled = true;
    };
  }, [activeRunId, refreshEvents, refreshRun, refreshSteps, selectedTeamId]);

  useEffect(() => {
    if (!activeRunId || !eventsAutoRefresh) return;
    const timer = window.setInterval(() => {
      void refreshRun(activeRunId).catch(() => undefined);
      void refreshEvents(activeRunId).catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [activeRunId, eventsAutoRefresh, refreshEvents, refreshRun]);

  const onCreateTeam = async () => {
    const name = newTeamName.trim();
    if (!name) {
      setError("Team name is required");
      return;
    }
    setBusy("create-team");
    setError(null);
    try {
      const created = await api.createTeam(props.token, {
        name,
        description: newTeamDescription.trim() || undefined,
        spec: parseRequiredJson(newTeamSpec, "Team spec"),
      });
      setTeams((prev) => [...prev, created].sort((a, b) => a.name.localeCompare(b.name)));
      setSelectedTeamId(created.id);
      setNewTeamName("");
      setNewTeamDescription("");
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onCreateRun = async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    setBusy("create-run");
    setError(null);
    try {
      const created = await api.createTeamRun(props.token, selectedTeamId, {
        context_id: runContextId.trim() || undefined,
        input: parseOptionalJson(runInput, "Run input") ?? {},
      });
      setRuns((prev) => upsertRun(prev, created));
      setActiveRunId(created.id);
      setRunLookupId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onLoadRunById = async () => {
    const runId = runLookupId.trim();
    if (!runId) {
      setError("Run ID is required");
      return;
    }
    setBusy("load-run");
    setError(null);
    try {
      const run = await refreshRun(runId);
      setSelectedTeamId(run.team_id);
      setActiveRunId(run.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onCancelRun = async () => {
    if (!activeRunId) return;
    setBusy("cancel-run");
    setError(null);
    try {
      const canceled = await api.cancelTeamRun(props.token, activeRunId);
      setRuns((prev) => upsertRun(prev, canceled));
      await refreshEvents(activeRunId);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onSubmitStep = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    if (!stepKey.trim()) {
      setError("step_key is required");
      return;
    }
    if (!stepMemberId.trim()) {
      setError("member_id is required");
      return;
    }
    setBusy("submit-step");
    setError(null);
    try {
      const created = await api.submitTeamRunStep(props.token, activeRunId, {
        step_key: stepKey.trim(),
        member_id: stepMemberId.trim(),
        depends_on: parseCsvList(stepDependsOn),
        input: parseOptionalJson(stepInput, "Step input"),
      });
      await Promise.all([refreshRun(activeRunId), refreshSteps(activeRunId), refreshEvents(activeRunId)]);
      setSelectedStepId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onApplyStepAction = async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    if (!selectedStepId) {
      setError("Select a step first");
      return;
    }
    setBusy(`step-${stepAction}`);
    setError(null);
    try {
      if (stepAction === "start") {
        await api.startTeamRunStep(props.token, activeRunId, selectedStepId, {
          remote_task_id: stepRemoteTaskId.trim() || undefined,
        });
      } else if (stepAction === "complete") {
        await api.completeTeamRunStep(props.token, activeRunId, selectedStepId, {
          output: parseOptionalJson(stepOutput, "Step output"),
        });
      } else if (stepAction === "fail") {
        const errorText = stepFailText.trim();
        if (!errorText) {
          throw new Error("Fail reason is required");
        }
        await api.failTeamRunStep(props.token, activeRunId, selectedStepId, {
          error_text: errorText,
        });
      } else if (stepAction === "input_required") {
        await api.setTeamRunStepInputRequired(props.token, activeRunId, selectedStepId, {
          reason: stepInputReason.trim() || undefined,
          input: parseOptionalJson(stepInputRequiredPayload, "Input required payload"),
        });
      } else {
        await api.resumeTeamRunStep(props.token, activeRunId, selectedStepId, {
          input: parseOptionalJson(stepResumePayload, "Resume payload"),
        });
      }
      await Promise.all([refreshRun(activeRunId), refreshSteps(activeRunId), refreshEvents(activeRunId)]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onSendMessage = async () => {
    if (!activeRunId) {
      setError("Select a run first");
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
      await api.sendTeamRunMessage(props.token, activeRunId, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: msgChannel.trim() || undefined,
        transport: msgTransport,
        route: parseOptionalJson(msgRoute, "Message route"),
        payload: parseRequiredJson(msgPayload, "Message payload"),
        idempotency_key: msgIdempotencyKey.trim() || undefined,
      });
      await refreshEvents(activeRunId);
      if (inboxActorId.trim()) {
        await loadInbox();
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const onRefreshInbox = async () => {
    if (!activeRunId) {
      setError("Select a run first");
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
  };

  const onAckMessage = async (message: TeamActorMessageRecord) => {
    if (!activeRunId) return;
    const actorId = inboxActorId.trim() || message.to_actor_id;
    setBusy(`ack-${message.message_id}`);
    setError(null);
    try {
      await api.ackTeamRunMessage(props.token, activeRunId, message.message_id, actorId);
      await Promise.all([loadInbox(), refreshEvents(activeRunId)]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="app">
      <header>
        <h1>AgentHub Teams</h1>
        <div className="session">
          <a className="icon-button" href="/" title="Back" aria-label="Back">
            <i className="bi bi-arrow-left" aria-hidden="true" />
          </a>
          <span>{props.auth.username}</span>
          <button onClick={props.onLogout}>Logout</button>
        </div>
      </header>

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}

      <section className="teams-layout">
        <aside className="card teams-sidebar">
          <div className="toolbar">
            <h2>Teams</h2>
            <button onClick={() => void refreshTeams()} disabled={busy === "refresh-teams"}>
              Refresh
            </button>
          </div>

          <div className="teams-form">
            <h3>Create Team</h3>
            <input
              placeholder="team name"
              value={newTeamName}
              onChange={(event) => setNewTeamName(event.target.value)}
            />
            <input
              placeholder="description (optional)"
              value={newTeamDescription}
              onChange={(event) => setNewTeamDescription(event.target.value)}
            />
            <textarea
              className="mono"
              rows={10}
              value={newTeamSpec}
              onChange={(event) => setNewTeamSpec(event.target.value)}
            />
            <button onClick={onCreateTeam} disabled={busy === "create-team"}>
              Create Team
            </button>
          </div>

          <div className="teams-list">
            {teams.length === 0 && <p className="muted">No teams yet.</p>}
            {teams.map((team) => (
              <button
                key={team.id}
                className={team.id === selectedTeamId ? "team-item active" : "team-item"}
                onClick={() => {
                  setSelectedTeamId(team.id);
                  setRunLookupId("");
                }}
              >
                <span className="team-name">{team.name}</span>
                <span className="team-id mono">{team.id}</span>
              </button>
            ))}
          </div>
        </aside>

        <div className="teams-main">
          {!selectedTeam && (
            <div className="card">
              <h2>Team Workbench</h2>
              <p>Select a team from the left panel to manage runs, steps, and messages.</p>
            </div>
          )}

          {selectedTeam && (
            <>
              <div className="card">
                <div className="toolbar">
                  <h2>{selectedTeam.name}</h2>
                  <span className="mono">{selectedTeam.id}</span>
                </div>
                <div className="teams-run-create">
                  <h3>Create / Load Run</h3>
                  <div className="form-row">
                    <input
                      placeholder="context_id (optional)"
                      value={runContextId}
                      onChange={(event) => setRunContextId(event.target.value)}
                    />
                    <button onClick={onCreateRun} disabled={busy === "create-run"}>
                      Create Run
                    </button>
                  </div>
                  <textarea
                    className="mono"
                    rows={4}
                    value={runInput}
                    onChange={(event) => setRunInput(event.target.value)}
                  />
                  <div className="form-row">
                    <input
                      placeholder="existing run_id"
                      value={runLookupId}
                      onChange={(event) => setRunLookupId(event.target.value)}
                    />
                    <button onClick={onLoadRunById} disabled={busy === "load-run"}>
                      Load Run
                    </button>
                  </div>
                </div>
                <div className="teams-run-list">
                  <h3>Runs In This Browser Session</h3>
                  {visibleRuns.length === 0 && (
                    <p className="muted">No runs loaded yet. Create one or load by run_id.</p>
                  )}
                  {visibleRuns.map((run) => (
                    <button
                      key={run.id}
                      className={run.id === activeRunId ? "team-item active" : "team-item"}
                      onClick={() => setActiveRunId(run.id)}
                    >
                      <span className="team-name mono">{run.id}</span>
                      <span className="team-status">{run.status}</span>
                    </button>
                  ))}
                </div>
              </div>

              {activeRun && (
                <>
                  <div className="card">
                    <div className="toolbar">
                      <h3>Active Run</h3>
                      <div className="actions">
                        <button
                          onClick={() => {
                            if (!activeRunId) return;
                            void refreshRun(activeRunId).catch((err) =>
                              setError(parseErrorMessage(err))
                            );
                          }}
                        >
                          Refresh Run
                        </button>
                        <button
                          onClick={onCancelRun}
                          disabled={busy === "cancel-run" || activeRun.status === "canceled"}
                        >
                          Cancel Run
                        </button>
                      </div>
                    </div>
                    <div className="teams-run-meta">
                      <span>
                        <strong>ID:</strong> <code>{activeRun.id}</code>
                      </span>
                      <span>
                        <strong>Status:</strong> {activeRun.status}
                      </span>
                      <span>
                        <strong>Context:</strong> {activeRun.context_id}
                      </span>
                      <span>
                        <strong>Created:</strong> {formatTs(activeRun.created_at)}
                      </span>
                      <span>
                        <strong>Started:</strong> {formatTs(activeRun.started_at)}
                      </span>
                      <span>
                        <strong>Ended:</strong> {formatTs(activeRun.ended_at)}
                      </span>
                    </div>
                  </div>

                  <div className="tab-bar">
                    <button
                      className={tab === "events" ? "tab active" : "tab"}
                      onClick={() => setTab("events")}
                    >
                      Events
                    </button>
                    <button
                      className={tab === "steps" ? "tab active" : "tab"}
                      onClick={() => setTab("steps")}
                    >
                      Steps
                    </button>
                    <button
                      className={tab === "messages" ? "tab active" : "tab"}
                      onClick={() => setTab("messages")}
                    >
                      Messages
                    </button>
                  </div>

                  {tab === "events" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Run Events</h3>
                        <div className="actions">
                          <label className="checkbox">
                            <input
                              type="checkbox"
                              checked={eventsAutoRefresh}
                              onChange={(event) =>
                                setEventsAutoRefresh(event.target.checked)
                              }
                            />
                            Auto refresh
                          </label>
                          <button
                            onClick={() => void refreshEvents(activeRun.id)}
                            disabled={eventsLoading}
                          >
                            Refresh
                          </button>
                          <button
                            onClick={() => void refreshEvents(activeRun.id, "prepend")}
                            disabled={eventsLoading || !eventsHasMore || oldestEventId == null}
                          >
                            Load Older
                          </button>
                        </div>
                      </div>
                      {events.length === 0 && <p className="muted">No events.</p>}
                      <ul className="teams-event-list">
                        {events.map((event) => (
                          <li key={event.event_id}>
                            <div className="teams-event-head">
                              <span className="mono">#{event.event_id}</span>
                              <span>{event.event_type}</span>
                              <span>{formatTs(event.ts)}</span>
                            </div>
                            <pre className="mono">{toPrettyJson(event.payload)}</pre>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {tab === "steps" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Steps</h3>
                        <button onClick={() => void refreshSteps(activeRun.id)}>Refresh</button>
                      </div>

                      <div className="teams-step-grid">
                        <div className="teams-step-panel">
                          <h4>Submit Step</h4>
                          <input
                            placeholder="step_key"
                            value={stepKey}
                            onChange={(event) => setStepKey(event.target.value)}
                          />
                          <input
                            placeholder="member_id"
                            value={stepMemberId}
                            onChange={(event) => setStepMemberId(event.target.value)}
                          />
                          <input
                            placeholder="depends_on (comma separated)"
                            value={stepDependsOn}
                            onChange={(event) => setStepDependsOn(event.target.value)}
                          />
                          <textarea
                            className="mono"
                            rows={4}
                            value={stepInput}
                            onChange={(event) => setStepInput(event.target.value)}
                          />
                          <button onClick={onSubmitStep} disabled={busy === "submit-step"}>
                            Submit Step
                          </button>
                        </div>

                        <div className="teams-step-panel">
                          <h4>Step Action</h4>
                          <select
                            value={selectedStepId}
                            onChange={(event) => setSelectedStepId(event.target.value)}
                          >
                            <option value="">Select step</option>
                            {steps.map((step) => (
                              <option key={step.id} value={step.id}>
                                {step.step_key} ({step.status})
                              </option>
                            ))}
                          </select>
                          <select
                            value={stepAction}
                            onChange={(event) =>
                              setStepAction(event.target.value as StepAction)
                            }
                          >
                            <option value="start">start</option>
                            <option value="complete">complete</option>
                            <option value="fail">fail</option>
                            <option value="input_required">input_required</option>
                            <option value="resume">resume</option>
                          </select>

                          {stepAction === "start" && (
                            <input
                              placeholder="remote_task_id (optional)"
                              value={stepRemoteTaskId}
                              onChange={(event) =>
                                setStepRemoteTaskId(event.target.value)
                              }
                            />
                          )}

                          {stepAction === "complete" && (
                            <textarea
                              className="mono"
                              rows={4}
                              value={stepOutput}
                              onChange={(event) => setStepOutput(event.target.value)}
                            />
                          )}

                          {stepAction === "fail" && (
                            <input
                              placeholder="error_text"
                              value={stepFailText}
                              onChange={(event) => setStepFailText(event.target.value)}
                            />
                          )}

                          {stepAction === "input_required" && (
                            <>
                              <input
                                placeholder="reason (optional)"
                                value={stepInputReason}
                                onChange={(event) =>
                                  setStepInputReason(event.target.value)
                                }
                              />
                              <textarea
                                className="mono"
                                rows={4}
                                value={stepInputRequiredPayload}
                                onChange={(event) =>
                                  setStepInputRequiredPayload(event.target.value)
                                }
                              />
                            </>
                          )}

                          {stepAction === "resume" && (
                            <textarea
                              className="mono"
                              rows={4}
                              value={stepResumePayload}
                              onChange={(event) =>
                                setStepResumePayload(event.target.value)
                              }
                            />
                          )}

                          <button onClick={onApplyStepAction}>
                            Apply Step Action
                          </button>
                        </div>
                      </div>

                      <ul className="teams-step-list">
                        {steps.map((step) => (
                          <li key={step.id}>
                            <div className="teams-step-head">
                              <span className="mono">{step.id}</span>
                              <span>{step.step_key}</span>
                              <span>{step.status}</span>
                            </div>
                            <div className="teams-step-body mono">
                              <div>member_id: {step.member_id}</div>
                              <div>attempt: {step.attempt}</div>
                              <div>
                                depends_on: {step.depends_on.length ? step.depends_on.join(", ") : "-"}
                              </div>
                              <div>remote_task_id: {step.remote_task_id ?? "-"}</div>
                              {step.error_text && <div>error_text: {step.error_text}</div>}
                            </div>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {tab === "messages" && (
                    <div className="card">
                      <div className="toolbar">
                        <h3>Messages</h3>
                      </div>

                      <div className="teams-message-grid">
                        <div className="teams-message-panel">
                          <h4>Send Message</h4>
                          <input
                            placeholder="from_actor_id"
                            value={msgFromActorId}
                            onChange={(event) => setMsgFromActorId(event.target.value)}
                          />
                          <input
                            placeholder="to_actor_id"
                            value={msgToActorId}
                            onChange={(event) => setMsgToActorId(event.target.value)}
                          />
                          <input
                            placeholder="channel (default)"
                            value={msgChannel}
                            onChange={(event) => setMsgChannel(event.target.value)}
                          />
                          <select
                            value={msgTransport}
                            onChange={(event) =>
                              setMsgTransport(event.target.value as "local" | "remote")
                            }
                          >
                            <option value="local">local</option>
                            <option value="remote">remote</option>
                          </select>
                          <textarea
                            className="mono"
                            rows={3}
                            placeholder="route JSON (required for remote)"
                            value={msgRoute}
                            onChange={(event) => setMsgRoute(event.target.value)}
                          />
                          <textarea
                            className="mono"
                            rows={4}
                            placeholder="payload JSON"
                            value={msgPayload}
                            onChange={(event) => setMsgPayload(event.target.value)}
                          />
                          <input
                            placeholder="idempotency_key (optional)"
                            value={msgIdempotencyKey}
                            onChange={(event) =>
                              setMsgIdempotencyKey(event.target.value)
                            }
                          />
                          <button onClick={onSendMessage} disabled={busy === "send-message"}>
                            Send Message
                          </button>
                        </div>

                        <div className="teams-message-panel">
                          <h4>Inbox</h4>
                          <input
                            placeholder="actor_id"
                            value={inboxActorId}
                            onChange={(event) => setInboxActorId(event.target.value)}
                          />
                          <input
                            placeholder="limit"
                            value={inboxLimit}
                            onChange={(event) => setInboxLimit(event.target.value)}
                          />
                          <input
                            placeholder="after_id (optional)"
                            value={inboxAfterId}
                            onChange={(event) => setInboxAfterId(event.target.value)}
                          />
                          <label className="checkbox">
                            <input
                              type="checkbox"
                              checked={inboxIncludeDelivered}
                              onChange={(event) =>
                                setInboxIncludeDelivered(event.target.checked)
                              }
                            />
                            include_delivered
                          </label>
                          <button
                            onClick={onRefreshInbox}
                            disabled={busy === "refresh-inbox"}
                          >
                            Refresh Inbox
                          </button>
                        </div>
                      </div>

                      <ul className="teams-message-list">
                        {inbox.map((message) => (
                          <li key={message.message_id}>
                            <div className="teams-message-head">
                              <span className="mono">#{message.message_id}</span>
                              <span>
                                {message.from_actor_id} → {message.to_actor_id}
                              </span>
                              <span>{message.status}</span>
                            </div>
                            <pre className="mono">{toPrettyJson(message.payload)}</pre>
                            <div className="actions">
                              <button
                                onClick={() => void onAckMessage(message)}
                                disabled={
                                  message.status === "delivered" ||
                                  busy === `ack-${message.message_id}`
                                }
                              >
                                Ack
                              </button>
                            </div>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </>
              )}
            </>
          )}
        </div>
      </section>
    </div>
  );
}
