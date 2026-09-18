// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
import type {
  LoopConfiguration,
  LoopConfigurationUpdate,
} from "../../loop_types";
import {
  useLoopConfiguration,
  type LoopMemberScope,
} from "./use_loop_configuration";

(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function configuration(actor = "worker", revision = 1): LoopConfiguration {
  return {
    policy: {
      actor_id: actor,
      team_id: "team",
      state: "disabled",
      session_policy: "fresh",
      revision,
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

describe("loop configuration controller", () => {
  let root: Root;
  let container: HTMLDivElement;
  let value: ReturnType<typeof useLoopConfiguration>;
  const scope: LoopMemberScope = {
    token: "token",
    userId: "owner",
    teamId: "team",
    actorId: "worker",
  };
  function Harness(props: LoopMemberScope) {
    value = useLoopConfiguration(props);
    return (
      <span>{value.state?.configuration?.policy?.actor_id ?? "loading"}</span>
    );
  }
  async function render(props = scope) {
    await act(async () => {
      root.render(<Harness {...props} />);
    });
  }
  function update(): LoopConfigurationUpdate {
    const policy = value.state!.configuration!.policy!;
    return {
      expected_revision: policy.revision,
      state: "enabled",
      session_policy: policy.session_policy,
      limits: policy.limits,
    };
  }

  beforeEach(() => {
    const storage = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
    });
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(api, "getTeamMemberLoop").mockImplementation(
      async (_token, _team, actor) => configuration(actor),
    );
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("hides previous scope data and drops a late read after changing members", async () => {
    const old = deferred<LoopConfiguration>();
    vi.mocked(api.getTeamMemberLoop).mockReturnValueOnce(old.promise);
    await render();
    await render({ ...scope, actorId: "planner" });
    expect(value.state?.configuration?.policy?.actor_id).toBe("planner");
    await act(async () => old.resolve(configuration()));
    expect(container.textContent).toBe("planner");
  });

  it("refreshes a revision conflict without resubmitting the requested change", async () => {
    vi.spyOn(api, "configureTeamMemberLoop").mockRejectedValue(
      Object.assign(new Error("revision mismatch"), { status: 409 }),
    );
    await render();
    const payload = update();
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(
      configuration("worker", 3),
    );
    await act(async () => value.configure(payload));
    expect(api.configureTeamMemberLoop).toHaveBeenCalledExactlyOnceWith(
      "token",
      "team",
      "worker",
      payload,
    );
    expect(value.state?.configuration?.policy?.revision).toBe(3);
    expect(value.state?.actionError).toContain("Review the refreshed settings");
    expect(value.state?.busy).toBeNull();
  });

  it("reconciles an uncertain configuration write without replaying it", async () => {
    const configure = vi
      .spyOn(api, "configureTeamMemberLoop")
      .mockRejectedValue(new TypeError("Lost response"));
    await render();
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue({
      ...configuration("worker", 2),
      policy: { ...configuration("worker", 2).policy!, state: "enabled" },
    });
    await act(async () => value.configure(update()));
    expect(configure).toHaveBeenCalledTimes(1);
    expect(value.state?.configuration?.policy?.state).toBe("enabled");
    expect(value.state?.configuration?.policy?.revision).toBe(2);
    expect(value.state?.actionError).toContain("Lost response");
    expect(value.state?.stale).toBe(false);
  });

  it("does not let an older refresh overwrite an acknowledged configuration change", async () => {
    await render();
    const old = deferred<LoopConfiguration>();
    vi.mocked(api.getTeamMemberLoop).mockReturnValueOnce(old.promise);
    let read!: Promise<void>;
    act(() => {
      read = value.refresh();
    });
    vi.spyOn(api, "configureTeamMemberLoop").mockResolvedValue(
      configuration("worker", 2),
    );
    await act(async () => value.configure(update()));
    await act(async () => {
      old.resolve(configuration());
      await read;
    });
    expect(value.state?.configuration?.policy?.revision).toBe(2);
  });

  it("keeps a lost-response activation identity across unmount and retries once", async () => {
    const activate = vi
      .spyOn(api, "activateTeamMemberLoop")
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockResolvedValue({
        trigger_id: "trigger",
        activation_id: "activation",
        duplicate: true,
      });
    await render();
    await act(async () => value.activate());
    const identity = activate.mock.calls[0][3].source_key;
    expect(value.state?.retryPending).toBe(true);
    act(() => root.unmount());
    root = createRoot(container);
    await render();
    expect(value.state?.retryPending).toBe(true);
    await act(async () => value.activate());
    expect(activate.mock.calls[1][3].source_key).toBe(identity);
    expect(value.state?.retryPending).toBe(false);
    expect(value.state?.receipt?.duplicate).toBe(true);
    await act(async () => value.activate());
    expect(activate.mock.calls[2][3].source_key).not.toBe(identity);
  });

  it("scopes retry identities by user, Team, and member", async () => {
    const activate = vi
      .spyOn(api, "activateTeamMemberLoop")
      .mockRejectedValue(new Error("lost response"));
    for (const next of [
      scope,
      { ...scope, userId: "other" },
      { ...scope, teamId: "other" },
      { ...scope, actorId: "other" },
      scope,
    ]) {
      await render(next);
      await act(async () => value.activate());
    }
    const keys = activate.mock.calls.map((call) => call[3].source_key);
    expect(new Set(keys.slice(0, 4)).size).toBe(4);
    expect(keys[4]).toBe(keys[0]);
  });

  it("ignores an old mutation after leaving and returning to the same member", async () => {
    const old = deferred<LoopConfiguration>();
    vi.spyOn(api, "configureTeamMemberLoop").mockReturnValueOnce(old.promise);
    await render();
    let mutation!: Promise<void>;
    act(() => {
      mutation = value.configure(update());
    });
    await render({ ...scope, actorId: "planner" });
    vi.mocked(api.getTeamMemberLoop).mockResolvedValue(
      configuration("worker", 7),
    );
    await render();
    await act(async () => {
      old.resolve(configuration("worker", 2));
      await mutation;
    });
    expect(value.state?.configuration?.policy?.revision).toBe(7);
  });

  it("allows one mutation at a time and preserves prior facts on read failure", async () => {
    const response = deferred<LoopConfiguration>();
    const configure = vi
      .spyOn(api, "configureTeamMemberLoop")
      .mockReturnValue(response.promise);
    const activate = vi.spyOn(api, "activateTeamMemberLoop");
    await render();
    let pending!: Promise<void>;
    act(() => {
      pending = value.configure(update());
      void value.configure(update());
      void value.activate();
    });
    expect(configure).toHaveBeenCalledTimes(1);
    expect(activate).not.toHaveBeenCalled();
    await act(async () => {
      response.resolve(configuration("worker", 2));
      await pending;
    });
    vi.mocked(api.getTeamMemberLoop).mockRejectedValue(new Error("offline"));
    await act(async () => value.refresh());
    expect(value.state?.configuration?.policy?.revision).toBe(2);
    expect(value.state?.stale).toBe(true);
    expect(value.state?.error).toBe("offline");
  });
});
