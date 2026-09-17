import type { Page } from "@playwright/test";
import type {
  LoopActivation,
  LoopConfiguration,
  LoopConfigurationUpdate,
  LoopRegistration,
  LoopSchedule,
  LoopSourceSummary,
} from "../../src/loop_types";
import { jsonResponse, mockTeamPageApis } from "./team_page_fixture";

export async function mockLoopWorkspace(page: Page) {
  const fixture = await mockTeamPageApis(page);
  const teamId = "team-loop-browser";
  const actorId = "agent-worker-1";
  fixture.agents.forEach((agent) => {
    agent.status = "stopped";
  });
  fixture.teams.push({
    id: teamId,
    name: "Loop Browser Team",
    spec: {
      execution_mode: "loop",
      coordinator_member_id: "agent-coordinator-1",
      members: [
        { member_id: "agent-coordinator-1", role: "coordinator" },
        {
          member_id: actorId,
          role: "worker",
          model: "gemini",
          description: "Offline loop worker",
        },
      ],
    },
    created_at: fixture.now,
    updated_at: fixture.now,
  });
  fixture.seedRuns(teamId, []);
  fixture.putTask({
    id: "retained-task",
    team_id: teamId,
    title: "Review retained evidence",
    status: "open",
    created_by_actor_id: `user:${fixture.auth.userId}`,
    assigned_member_id: actorId,
    context: {},
    created_at: fixture.now,
    updated_at: fixture.now,
  });
  const configuration: LoopConfiguration = {
    policy: {
      actor_id: actorId,
      team_id: teamId,
      state: "disabled",
      session_policy: "fresh",
      revision: 1,
      mailbox_run_id: null,
      generation: 0,
      no_progress_count: 0,
      created_at: fixture.now,
      updated_at: fixture.now,
      limits: {
        pending_per_actor: 32,
        pending_per_team: 256,
        sources_per_activation: 64,
        lease_seconds: 60,
        renewal_seconds: 15,
        startup_attempts: 5,
        retry_initial_seconds: 1,
        retry_max_seconds: 60,
        consecutive_no_progress: 3,
        window_seconds: 900,
        activations_per_actor: 12,
        activations_per_team: 120,
        standing_per_actor: 16,
        standing_per_team: 128,
      },
    },
    preflight: {
      ready: true,
      provider_id: "gemini",
      capabilities: [],
      blockers: [],
      warnings: [],
    },
  };
  const updates: LoopConfigurationUpdate[] = [];
  const activationKeys: string[] = [];
  const pages: {
    source: Array<string | null>;
    event: Array<string | null>;
    tool: Array<string | null>;
  } = { source: [], event: [], tool: [] };
  let loseNextActivationResponse = false;
  const base = `/api/teams/${teamId}/members/${actorId}/loop`;
  await page.route(`**${base}`, async (route, request) => {
    if (request.method() === "PUT") {
      const update = request.postDataJSON() as LoopConfigurationUpdate;
      updates.push(update);
      if (update.expected_revision !== configuration.policy!.revision) {
        await route.fulfill(jsonResponse({ error: "Revision changed" }, 409));
        return;
      }
      configuration.policy = {
        ...configuration.policy!,
        state: update.state,
        session_policy: update.session_policy,
        limits: update.limits,
        revision: update.expected_revision + 1,
      };
    }
    await route.fulfill(jsonResponse(configuration));
  });
  await page.route(`**${base}/activate`, async (route, request) => {
    const key = (request.postDataJSON() as { source_key: string }).source_key;
    const duplicate = activationKeys.includes(key);
    activationKeys.push(key);
    if (loseNextActivationResponse) {
      loseNextActivationResponse = false;
      await route.abort("failed");
      return;
    }
    await route.fulfill(
      jsonResponse({
        activation_id: "queued",
        trigger_id: "trigger",
        duplicate,
      }),
    );
  });
  const references: LoopSourceSummary["references"] = {
    task_id: null,
    mailbox_message_id: null,
    conversation_message_id: null,
    thread_id: null,
    scheduling_actor_id: null,
    scheduling_activation_id: null,
    scheduling_user_id: null,
    app_id: null,
  };
  const activation: LoopActivation = {
    id: "finished",
    actor_id: actorId,
    team_id: teamId,
    state: "finished",
    due_at: fixture.now,
    next_admission_at: fixture.now,
    policy_revision: 1,
    generation: 1,
    attempt_count: 1,
    mailbox_run_id: null,
    session_id: null,
    created_at: fixture.now,
    updated_at: fixture.now,
    finished_at: fixture.now,
    launch: null,
    outcome: {
      kind: "completion_proposed",
      wait_reason: null,
      task_note_id: null,
      continuation: null,
    },
  };
  await page.route(`**${base}/activations?*`, async (route, request) => {
    const older = new URL(request.url()).searchParams.has(
      "before_activation_id",
    );
    const activations: LoopActivation[] = older
      ? [
          {
            ...activation,
            id: "older",
            outcome: {
              kind: "waiting",
              wait_reason: "external_event",
              task_note_id: null,
              continuation: {
                due_at: fixture.now + 60,
                task_id: "retained-task",
              },
            },
          },
        ]
      : [
          activation,
          { ...activation, id: "queued", state: "pending", outcome: null },
        ];
    await route.fulfill(
      jsonResponse({ activations, next_cursor: older ? null : "older" }),
    );
  });
  const schedules: LoopSchedule[] = [
    { kind: "due", due_at: fixture.now + 60 },
    { kind: "recurring", first_at: fixture.now + 90, interval_seconds: 120 },
    {
      kind: "task_status",
      task_id: "retained-task",
      statuses: ["in_review"],
      repeat: false,
    },
    {
      kind: "thread_reply",
      root_message_id: 1,
      after_message_id: 1,
      repeat: false,
    },
  ];
  const registrations: LoopRegistration[] = schedules.map(
    (schedule, index) => ({
      id: `schedule-${index}`,
      input: {
        actor_id: actorId,
        team_id: teamId,
        source_key: `schedule-${index}`,
        schedule,
        work_task_id: null,
        references,
      },
      state: "active",
      next_due_at: fixture.now + 60,
      observed_cursor: 0,
      pending_cursor: null,
      pending_due_at: null,
      next_check_at: fixture.now,
      created_at: fixture.now,
      updated_at: fixture.now,
    }),
  );
  await page.route(`**${base}/schedules?*`, async (route, request) => {
    const more = new URL(request.url()).searchParams.has(
      "after_registration_id",
    );
    await route.fulfill(
      jsonResponse({
        registrations: more
          ? registrations.slice(2)
          : registrations.slice(0, 2),
        next_cursor: more ? null : "schedule-1",
      }),
    );
  });
  await page.route(
    `**${base}/activations/finished/sources?*`,
    async (route, request) => {
      const cursor = new URL(request.url()).searchParams.get("after_source_id");
      pages.source.push(cursor);
      await route.fulfill(
        jsonResponse({
          sources: [
            {
              id: cursor ? "source-2" : "source-1",
              kind: cursor ? "assignment" : "message",
              references,
              due_at: null,
              created_at: fixture.now,
              revoked: !cursor,
            },
          ],
          next_cursor: cursor ? null : "source-1",
        }),
      );
    },
  );
  await page.route(
    `**${base}/activations/finished/events?*`,
    async (route, request) => {
      const cursor = new URL(request.url()).searchParams.get("after_event_id");
      pages.event.push(cursor);
      await route.fulfill(
        jsonResponse({
          events: [
            {
              id: cursor ? 1 : 0,
              activation_id: "finished",
              kind: cursor ? "cleanup_verified" : "running",
              generation: 1,
              trigger_id: null,
              reason: null,
              exit_reason: cursor ? "provider_exited" : null,
              created_at: fixture.now,
            },
          ],
          next_cursor: cursor ? null : 0,
        }),
      );
    },
  );
  await page.route(
    `**${base}/activations/finished/tools?*`,
    async (route, request) => {
      const cursor = new URL(request.url()).searchParams.get("after_tool_id");
      pages.tool.push(cursor);
      await route.fulfill(
        jsonResponse({
          tools: [
            {
              id: cursor ? 1 : 0,
              activation_id: "finished",
              generation: 1,
              surface: "mcp_tool",
              tool_name: cursor ? "read_result" : "app_write",
              target_ref: null,
              operation_id: "operation",
              attempt_number: 1,
              status: cursor ? "succeeded" : "outcome_unknown",
              started_at: fixture.now,
              completed_at: cursor ? fixture.now + 1 : null,
              duration_ms: cursor ? 1000 : null,
            },
          ],
          next_cursor: cursor ? null : 0,
        }),
      );
    },
  );
  return {
    fixture,
    teamId,
    actorId,
    configuration,
    updates,
    activationKeys,
    pages,
    loseNextActivationResponse: () => {
      loseNextActivationResponse = true;
    },
  };
}
