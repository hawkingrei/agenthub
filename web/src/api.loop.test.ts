// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";

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
});
