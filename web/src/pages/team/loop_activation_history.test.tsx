// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
import type {
  LoopActivation,
  LoopRegistration,
  LoopSchedule,
} from "../../loop_types";
import { LoopActivationHistory } from "./loop_activation_history";

(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;
const scope = {
  token: "token",
  userId: "owner",
  teamId: "team",
  actorId: "worker",
};
const references = {
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
  id: "activation",
  actor_id: "worker",
  team_id: "team",
  state: "finished",
  due_at: 10,
  next_admission_at: 20,
  policy_revision: 3,
  generation: 1,
  attempt_count: 1,
  mailbox_run_id: null,
  session_id: null,
  created_at: 1,
  updated_at: 2,
  finished_at: 2,
  outcome: {
    kind: "waiting",
    wait_reason: "external_event",
    task_note_id: null,
    continuation: { due_at: 30, task_id: null },
  },
  launch: null,
};
function registration(
  id: string,
  schedule: LoopSchedule,
  state: LoopRegistration["state"] = "active",
): LoopRegistration {
  return {
    id,
    input: {
      actor_id: "worker",
      team_id: "team",
      source_key: id,
      schedule,
      work_task_id: null,
      references,
    },
    state,
    next_due_at: schedule.kind === "due" ? schedule.due_at : null,
    observed_cursor: 0,
    pending_cursor: null,
    pending_due_at: null,
    next_check_at: 1,
    created_at: 1,
    updated_at: 1,
  };
}

describe("retained activation and wake history", () => {
  let root: Root;
  let container: HTMLDivElement;
  async function render() {
    await act(async () => {
      root.render(
        <MantineProvider env="test">
          <LoopActivationHistory scope={scope} refreshKey={null} />
        </MantineProvider>,
      );
    });
  }
  async function click(label: string) {
    const button = [...container.querySelectorAll("button")].find(
      (item) => item.textContent === label,
    );
    expect(button, label).toBeDefined();
    await act(async () => button!.click());
  }
  it("shows App conditions and retained event identity on revoked dependency sources", async () => {
    vi.spyOn(api, "listTeamMemberLoopSchedules").mockResolvedValue({
      registrations: [
        registration("app-watch", {
          kind: "app_event",
          app_id: "app-release",
          event_class: "build.finished",
          after_cursor: 7,
          repeat: true,
        }),
      ],
      next_cursor: null,
    });
    vi.spyOn(api, "listTeamMemberActivationSources").mockResolvedValue({
      sources: [
        {
          id: "event-source",
          kind: "dependency",
          references: {
            ...references,
            app_id: "app-release",
            app_event: {
              event_id: "release-7",
              event_class: "build.finished",
              cursor: 7,
              version: 2,
            },
          },
          due_at: null,
          created_at: 1,
          revoked: true,
        },
      ],
      next_cursor: null,
    });
    await render();
    expect(container.textContent).toContain(
      "Waiting for App event: build.finished (app-release)",
    );
    expect(container.textContent).not.toContain("Scheduled wake: Not recorded");
    await click("Inspect activation");
    expect(container.textContent).toContain("revoked");
    expect(container.textContent).toContain(
      "App app-release · build.finished · Event release-7 · Cursor 7 · Version 2",
    );
  });
  beforeEach(() => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(api, "listTeamMemberActivations").mockResolvedValue({
      activations: [activation],
      next_cursor: null,
    });
    vi.spyOn(api, "listTeamMemberLoopSchedules").mockResolvedValue({
      registrations: [],
      next_cursor: null,
    });
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("keeps bounded wakes explicit and resets older pages on refresh", async () => {
    const history = vi
      .mocked(api.listTeamMemberActivations)
      .mockResolvedValueOnce({
        activations: [
          activation,
          { ...activation, id: "pending", state: "pending", outcome: null },
        ],
        next_cursor: "older",
      })
      .mockResolvedValueOnce({
        activations: [
          {
            ...activation,
            id: "older",
            outcome: {
              ...activation.outcome!,
              kind: "progress",
              wait_reason: null,
            },
          },
        ],
        next_cursor: null,
      });
    const schedules = vi
      .mocked(api.listTeamMemberLoopSchedules)
      .mockResolvedValueOnce({
        registrations: [
          registration("due", { kind: "due", due_at: 45 }),
          registration("recurring", {
            kind: "recurring",
            first_at: 50,
            interval_seconds: 60,
          }),
          registration("revoked", { kind: "due", due_at: 98765 }, "revoked"),
        ],
        next_cursor: "more-wakes",
      })
      .mockResolvedValueOnce({
        registrations: [
          registration("task", {
            kind: "task_status",
            task_id: "task",
            statuses: ["in_review", "completed"],
            repeat: false,
          }),
          registration("reply", {
            kind: "thread_reply",
            root_message_id: 1,
            after_message_id: 2,
            repeat: false,
          }),
        ],
        next_cursor: null,
      });
    await render();
    expect(container.textContent).toContain(
      `Queued work: eligible from ${new Date(20000).toLocaleString()}`,
    );
    expect(container.textContent).toContain("Waiting for external event");
    expect(container.textContent).toContain("Recorded continuation request");
    expect(container.textContent).toContain("Recurring wake: Not recorded");
    expect(container.textContent).not.toContain(
      new Date(98765000).toLocaleString(),
    );
    expect(container.textContent).toContain(
      "this list may omit earlier pending work",
    );
    await click("Load more wake records");
    expect(schedules).toHaveBeenLastCalledWith(
      "token",
      "team",
      "worker",
      "more-wakes",
      expect.any(AbortSignal),
    );
    expect(container.textContent).toContain(
      "Task condition: in review, completed",
    );
    expect(container.textContent).toContain("Waiting for a new thread reply");
    await click("Load older activations");
    expect(history).toHaveBeenLastCalledWith(
      "token",
      "team",
      "worker",
      "older",
      expect.any(AbortSignal),
    );
    expect(container.textContent).toContain(
      "Automatic refresh pauses while viewing older records",
    );
    await click("Refresh history");
    expect(history).toHaveBeenLastCalledWith(
      "token",
      "team",
      "worker",
      null,
      expect.any(AbortSignal),
    );
    expect(container.textContent).not.toContain("Automatic refresh pauses");
    expect(container.textContent).not.toContain("Queued work: eligible from");
  });

  it("retains factual tool results and revoked sources when a later page fails", async () => {
    const sources = vi
      .spyOn(api, "listTeamMemberActivationSources")
      .mockResolvedValueOnce({
        sources: [
          {
            id: "source",
            kind: "member_request",
            references,
            due_at: null,
            created_at: 1,
            revoked: true,
          },
        ],
        next_cursor: "source",
      })
      .mockRejectedValue(new Error("Source page unavailable"));
    vi.spyOn(api, "listTeamMemberActivationEvents").mockRejectedValue(
      new Error("Events unavailable"),
    );
    const tools = vi
      .spyOn(api, "listTeamMemberActivationTools")
      .mockResolvedValueOnce({
        tools: [
          {
            id: 0,
            activation_id: "activation",
            generation: 1,
            surface: "mcp_tool",
            tool_name: "app_write",
            target_ref: null,
            operation_id: "operation",
            attempt_number: 1,
            status: "outcome_unknown",
            started_at: 1,
            completed_at: null,
            duration_ms: null,
          },
        ],
        next_cursor: 0,
      })
      .mockResolvedValueOnce({
        tools: [
          {
            id: 1,
            activation_id: "activation",
            generation: 1,
            surface: "mcp_tool",
            tool_name: "app_read",
            target_ref: null,
            operation_id: "read-operation",
            attempt_number: 1,
            status: "succeeded",
            started_at: 2,
            completed_at: 3,
            duration_ms: 1000,
          },
        ],
        next_cursor: 1,
      })
      .mockRejectedValue(new Error("Tool page unavailable"));
    await render();
    await click("Inspect activation");
    expect(container.textContent).toContain("member request");
    expect(container.textContent).toContain("revoked");
    expect(container.textContent).toContain("Events unavailable");
    expect(container.textContent).toContain("app_write: outcome unknown");
    await click("More trigger sources");
    expect(sources).toHaveBeenLastCalledWith(
      "token",
      "team",
      "worker",
      "activation",
      "source",
      expect.any(AbortSignal),
    );
    expect(container.textContent).toContain("Source page unavailable");
    expect(container.textContent).toContain("member request");
    await click("More tool observations");
    expect(tools).toHaveBeenLastCalledWith(
      "token",
      "team",
      "worker",
      "activation",
      0,
      expect.any(AbortSignal),
    );
    expect(container.textContent).toContain("app_read: succeeded · 1000 ms");
    await click("More tool observations");
    expect(container.textContent).toContain("Tool page unavailable");
    expect(container.textContent).toContain("app_write: outcome unknown");
    expect(container.textContent).not.toContain(
      "No tool observations recorded.",
    );
    await click("Close details");
    expect(container.textContent).not.toContain("Activation details");
  });

  it("does not turn failed history reads into empty-work claims", async () => {
    vi.mocked(api.listTeamMemberActivations).mockRejectedValue(
      new Error("History unavailable"),
    );
    vi.mocked(api.listTeamMemberLoopSchedules).mockRejectedValue(
      new Error("Wakes unavailable"),
    );
    await render();
    expect(container.textContent).toContain("History unavailable");
    expect(container.textContent).toContain("Wakes unavailable");
    expect(container.textContent).not.toContain("No upcoming wake");
    expect(container.textContent).not.toContain("No activations yet");
  });
});
