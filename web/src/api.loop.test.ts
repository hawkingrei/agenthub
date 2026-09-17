// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import type { LoopConfigurationUpdate } from "./loop_types";

describe("scoped loop API", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("encodes every path segment, keeps zero event cursors, and forwards cancellation", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ events: [], next_cursor: null })),
      );
    vi.stubGlobal("fetch", fetch);
    const controller = new AbortController();
    await api.listTeamMemberActivationEvents(
      "token",
      "team/a",
      "actor#b",
      "activation/c",
      0,
      controller.signal,
    );
    expect(fetch.mock.calls[0][0]).toBe(
      "/api/teams/team%2Fa/members/actor%23b/loop/activations/activation%2Fc/events?limit=25&after_event_id=0",
    );
    expect(fetch.mock.calls[0][1].signal).toBe(controller.signal);
    expect(fetch.mock.calls[0][1].headers.get("Authorization")).toBe(
      "Bearer token",
    );
  });

  it("never silently retries an activation write after a lost response", async () => {
    const fetch = vi.fn().mockRejectedValue(new TypeError("Failed to fetch"));
    vi.stubGlobal("fetch", fetch);
    await expect(
      api.activateTeamMemberLoop("token", "team", "actor", {
        source_key: "intent",
      }),
    ).rejects.toThrow("Failed to fetch");
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({
      source_key: "intent",
    });
  });

  it("keeps cancellation and opaque pagination scoped on every history read", async () => {
    const fetch = vi.fn().mockImplementation(async () => new Response("{}"));
    vi.stubGlobal("fetch", fetch);
    const signal = new AbortController().signal;
    const identity = ["token", "team/a", "actor#b"] as const;
    await api.getTeamMemberLoop(...identity, signal);
    await api.listTeamMemberActivations(...identity, "cursor/&", signal);
    await api.getTeamMemberActivation(...identity, "activation/c", signal);
    await api.listTeamMemberActivationSources(
      ...identity,
      "activation/c",
      "source/&",
      signal,
    );
    await api.listTeamMemberActivationTools(
      ...identity,
      "activation/c",
      0,
      signal,
    );
    await api.getTeamMemberLoopMetrics(...identity, signal);
    await api.listTeamMemberLoopSchedules(...identity, null, signal);
    const base = "/api/teams/team%2Fa/members/actor%23b/loop";
    expect(fetch.mock.calls.map((call) => call[0])).toEqual([
      base,
      `${base}/activations?limit=25&before_activation_id=cursor%2F%26`,
      `${base}/activations/activation%2Fc`,
      `${base}/activations/activation%2Fc/sources?limit=25&after_source_id=source%2F%26`,
      `${base}/activations/activation%2Fc/tools?limit=25&after_tool_id=0`,
      `${base}/metrics`,
      `${base}/schedules?limit=25`,
    ]);
    for (const [, options] of fetch.mock.calls) {
      expect(options.signal).toBe(signal);
      expect(options.headers.get("Authorization")).toBe("Bearer token");
    }
  });

  it("sends configuration revisions once and returns conflicts for reconciliation", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ error: "Revision changed" }), {
          status: 409,
        }),
      );
    vi.stubGlobal("fetch", fetch);
    const update: LoopConfigurationUpdate = {
      expected_revision: 4,
      state: "suspended",
      session_policy: "fresh",
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
    };
    await expect(
      api.configureTeamMemberLoop("token", "team/a", "actor#b", update),
    ).rejects.toThrow("Revision changed");
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0][0]).toBe(
      "/api/teams/team%2Fa/members/actor%23b/loop",
    );
    expect(fetch.mock.calls[0][1].method).toBe("PUT");
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual(update);
  });
});
