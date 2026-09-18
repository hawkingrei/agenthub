// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type TeamTaskRecord } from "../../api";
import type { LoopActivation, LoopConfiguration } from "../../loop_types";
import { TeamLoopMemberPanel } from "./team_loop_member_panel";

(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function configuration(
  state: "disabled" | "enabled" | "suspended" = "disabled",
): LoopConfiguration {
  return {
    policy: {
      actor_id: "worker",
      team_id: "team",
      state,
      session_policy: "fresh",
      revision: 3,
      mailbox_run_id: null,
      generation: 0,
      no_progress_count: 0,
      created_at: 1,
      updated_at: 1,
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
      provider_id: "claude",
      capabilities: [],
      blockers: [],
      warnings: [],
    },
  };
}

const team = {
  id: "team",
  name: "Team",
  spec: { execution_mode: "loop" },
  created_at: 1,
  updated_at: 9,
};

describe("offline loop member panel", () => {
  const scrollIntoView = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "scrollIntoView",
  );
  let root: Root;
  let container: HTMLDivElement;
  const onEdit = vi.fn();
  const onOpenTask = vi.fn();
  const onTeamUpdated = vi.fn();
  function button(label: string) {
    const result = [...container.querySelectorAll("button")].find(
      (item) => item.textContent === label,
    );
    expect(result, label).toBeDefined();
    return result!;
  }
  async function render(
    overrides: Partial<React.ComponentProps<typeof TeamLoopMemberPanel>> = {},
  ) {
    await act(async () => {
      root.render(
        <MantineProvider env="test">
          <TeamLoopMemberPanel
            auth={{
              token: "token",
              userId: "owner",
              username: "Owner",
              role: "admin",
            }}
            team={team}
            actorId="worker"
            label="Worker"
            processStatus="stopped"
            tasks={[]}
            tasksLoading={false}
            onEdit={onEdit}
            onOpenTask={onOpenTask}
            onTeamUpdated={onTeamUpdated}
            onOpenDiagnostics={vi.fn()}
            {...overrides}
          />
        </MantineProvider>,
      );
    });
  }
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    const storage = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
    });
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(api, "getTeamMemberLoop").mockResolvedValue(configuration());
    vi.spyOn(api, "listTeamspaceMembers").mockResolvedValue([
      {
        team_id: "team",
        user_id: "owner",
        role: "owner",
        created_at: 1,
        updated_at: 1,
      },
    ]);
    vi.spyOn(api, "listTeamMemberActivations").mockResolvedValue({
      activations: [],
      next_cursor: null,
    });
    vi.spyOn(api, "listTeamMemberLoopSchedules").mockResolvedValue({
      registrations: [],
      next_cursor: null,
    });
  });
  afterEach(() => {
    act(() => root.unmount());
    if (scrollIntoView)
      Object.defineProperty(
        HTMLElement.prototype,
        "scrollIntoView",
        scrollIntoView,
      );
    else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
    container.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("keeps execution, stopped process, and task review distinct without a run", async () => {
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(
      configuration("enabled"),
    );
    await render({
      tasks: [
        {
          id: "task",
          title: "Review evidence",
          status: "in_review",
          assigned_member_id: "worker",
        } as TeamTaskRecord,
      ],
    });
    expect(container.textContent).toContain("Execution: enabled");
    expect(container.textContent).toContain("Process: stopped");
    expect(container.textContent).toContain("in review");
    expect(container.textContent).not.toContain("No Active Execution Run");
    act(() => button("Edit profile").click());
    expect(onEdit).toHaveBeenCalledTimes(1);
    act(() => button("Review evidence").click());
    expect(onOpenTask).toHaveBeenCalledWith("task");
    const configure = vi
      .spyOn(api, "configureTeamMemberLoop")
      .mockResolvedValue(configuration("suspended"));
    await act(async () => button("Suspend execution").click());
    expect(configure).toHaveBeenCalledExactlyOnceWith(
      "token",
      "team",
      "worker",
      {
        expected_revision: 3,
        state: "suspended",
        session_policy: "fresh",
        limits: configuration().policy!.limits,
      },
    );
    expect(container.textContent).toContain("in review");
  });

  it("permits an authorized owner without an explicit roster row", async () => {
    vi.mocked(api.listTeamspaceMembers).mockResolvedValue([
      {
        team_id: "team",
        user_id: "other",
        role: "observer",
        created_at: 1,
        updated_at: 1,
      },
    ]);
    await render();
    expect(button("Enable execution").disabled).toBe(false);
  });

  it("shows the current offline profile and preserves its accessible close action", async () => {
    const onClose = vi.fn();
    const member = {
      member_id: "worker",
      role: "worker",
      model: "gemini",
      description: "Handles browser route validation",
    };
    await render({ team: { ...team, spec: { members: [member] } }, onClose });
    expect(container.textContent).toContain("Agent Profile");
    expect(container.querySelector("h2")?.textContent).toBe("Worker worker");
    expect(container.textContent).toContain("Role: worker. Model: gemini.");
    expect(container.textContent).toContain(member.description);
    const close = container.querySelector<HTMLButtonElement>(
      '[aria-label="Close agent profile"]',
    );
    expect(close).not.toBeNull();
    act(() => close!.click());
    expect(onClose).toHaveBeenCalledOnce();
    await render({
      team: {
        ...team,
        spec: {
          members: [{ ...member, description: "Updated while stopped" }],
        },
      },
      label: "worker",
    });
    expect(container.querySelector("h2")?.textContent).toBe("worker");
    expect(container.textContent).toContain("Updated while stopped");
    expect(container.textContent).not.toContain(member.description);
  });

  it("queues a request while suspended without resuming execution", async () => {
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(
      configuration("suspended"),
    );
    const activate = vi.spyOn(api, "activateTeamMemberLoop").mockResolvedValue({
      trigger_id: "trigger",
      activation_id: "activation",
      duplicate: false,
    });
    const configure = vi.spyOn(api, "configureTeamMemberLoop");
    await render();
    await act(async () => button("Activate member").click());
    expect(activate).toHaveBeenCalledTimes(1);
    expect(configure).not.toHaveBeenCalled();
    expect(container.textContent).toContain("Execution: suspended");
    expect(container.textContent).toContain("Activation request accepted");
    expect(button("Resume execution").disabled).toBe(false);
  });

  it("shows preflight blockers and prevents enablement", async () => {
    const blocked = configuration();
    blocked.preflight = {
      ...blocked.preflight,
      ready: false,
      blockers: ["workspace_unavailable"],
    };
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(blocked);
    await render();
    expect(container.textContent).toContain("Choose an available workspace.");
    expect(button("Enable execution").disabled).toBe(true);
    expect(button("Activate member").disabled).toBe(true);
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(configuration());
    await act(async () => button("Check again").click());
    expect(button("Enable execution").disabled).toBe(false);
  });

  it("saves edited budgets against the observed revision and discards unsaved edits", async () => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
    const initial = configuration("suspended");
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(initial);
    const configure = vi
      .spyOn(api, "configureTeamMemberLoop")
      .mockResolvedValue(initial);
    await render();
    await act(async () => button("Execution settings").click());
    function input(label: string) {
      const element = [...container.querySelectorAll("label")].find(
        (item) => item.textContent === label,
      );
      expect(element).toBeDefined();
      return document.getElementById(element!.htmlFor) as HTMLInputElement;
    }
    async function change(label: string, value: string) {
      await act(async () => {
        Object.getOwnPropertyDescriptor(
          HTMLInputElement.prototype,
          "value",
        )!.set!.call(input(label), value);
        input(label).dispatchEvent(new Event("input", { bubbles: true }));
      });
    }
    await change("Activations per window", "");
    expect(button("Save execution settings").disabled).toBe(true);
    await change("Activations per window", "5");
    await change("Window in seconds", "120");
    await act(async () => input("Session policy").click());
    const resume = [
      ...document.querySelectorAll<HTMLElement>('[role="option"]'),
    ].find((item) => item.textContent?.includes("Resume when supported"));
    expect(resume).toBeDefined();
    await act(async () => resume!.click());
    await act(async () => button("Save execution settings").click());
    expect(configure).toHaveBeenCalledExactlyOnceWith(
      "token",
      "team",
      "worker",
      {
        expected_revision: 3,
        state: "suspended",
        session_policy: "resume",
        limits: {
          ...initial.policy!.limits,
          activations_per_actor: 5,
          window_seconds: 120,
        },
      },
    );
    await change("Activations per window", "2");
    await act(async () => button("Close settings").click());
    await act(async () => button("Execution settings").click());
    expect(input("Activations per window").value).toBe("12");
    expect(configure).toHaveBeenCalledTimes(1);
  });

  it("reconciles a lost opt-in response without repeating the write", async () => {
    const update = vi
      .spyOn(api, "updateTeamSpec")
      .mockRejectedValue(new Error("Response lost"));
    const read = vi.spyOn(api, "getTeam").mockResolvedValue(team);
    await render({ team: { ...team, spec: {} } });
    await act(async () => button("Use durable execution").click());
    expect(update).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledExactlyOnceWith("token", "team");
    expect(onTeamUpdated).toHaveBeenCalledWith(team);
    expect(container.textContent).toContain("Response lost");
  });

  it("keeps controls disabled when membership cannot be read", async () => {
    vi.mocked(api.listTeamspaceMembers).mockRejectedValue(
      new Error("Unavailable"),
    );
    await render();
    expect(button("Enable execution").disabled).toBe(true);
    expect(button("Edit profile").disabled).toBe(true);
  });

  it("keeps history visible to an observer without offering writable controls", async () => {
    await render({
      auth: {
        token: "token",
        userId: "observer",
        username: "Observer",
        role: "viewer",
      },
    });
    expect(container.textContent).toContain("Activation history");
    expect(button("Enable execution").disabled).toBe(true);
    expect(button("Edit profile").disabled).toBe(true);
  });

  it("inspects retained history with a zero event cursor and separate task review", async () => {
    const activation: LoopActivation = {
      id: "activation",
      actor_id: "worker",
      team_id: "team",
      state: "finished",
      due_at: 1,
      next_admission_at: 1,
      policy_revision: 3,
      generation: 1,
      attempt_count: 1,
      mailbox_run_id: "mailbox",
      session_id: "session",
      created_at: 1,
      updated_at: 2,
      finished_at: 2,
      launch: null,
      outcome: {
        kind: "completion_proposed",
        wait_reason: null,
        task_note_id: null,
        continuation: null,
      },
    };
    vi.mocked(api.listTeamMemberActivations).mockResolvedValue({
      activations: [activation],
      next_cursor: null,
    });
    vi.spyOn(api, "listTeamMemberActivationSources").mockResolvedValue({
      sources: [],
      next_cursor: null,
    });
    const events = vi
      .spyOn(api, "listTeamMemberActivationEvents")
      .mockResolvedValueOnce({
        events: [
          {
            id: 0,
            activation_id: "activation",
            kind: "running",
            generation: 1,
            trigger_id: null,
            reason: null,
            exit_reason: null,
            created_at: 1,
          },
        ],
        next_cursor: 0,
      })
      .mockResolvedValue({
        events: [
          {
            id: 1,
            activation_id: "activation",
            kind: "cleanup_verified",
            generation: 1,
            trigger_id: null,
            reason: null,
            exit_reason: null,
            created_at: 2,
          },
        ],
        next_cursor: null,
      });
    vi.spyOn(api, "listTeamMemberActivationTools").mockResolvedValue({
      tools: [],
      next_cursor: null,
    });
    await render();
    expect(container.textContent).toContain(
      "Completion proposed; task review remains separate",
    );
    await act(async () => button("Inspect activation").click());
    expect(container.textContent).toContain("Lifecycle events");
    await act(async () => button("More lifecycle events").click());
    expect(events).toHaveBeenLastCalledWith(
      "token",
      "team",
      "worker",
      "activation",
      0,
      expect.any(AbortSignal),
    );
    expect(container.textContent).toContain("cleanup verified");
    expect(container.textContent).toContain("No tool observations recorded.");
  });

  it("opts in explicitly and preserves the rest of the Team specification", async () => {
    const manual = {
      ...team,
      spec: { members: [{ member_id: "worker" }], custom: "retained" },
    };
    const updated = {
      ...manual,
      updated_at: 10,
      spec: { ...manual.spec, execution_mode: "loop" },
    };
    const update = vi.spyOn(api, "updateTeamSpec").mockResolvedValue(updated);
    await render({ team: manual });
    expect(update).not.toHaveBeenCalled();
    await act(async () => button("Use durable execution").click());
    expect(update).toHaveBeenCalledExactlyOnceWith("token", "team", {
      expected_updated_at: 9,
      spec: updated.spec,
    });
    expect(onTeamUpdated).toHaveBeenCalledWith(updated);
  });
});
