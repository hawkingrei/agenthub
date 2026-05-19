// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  __testOnlyApiInternals,
  api,
  buildTeamRunContextSseUrl,
  buildTeamRuntimeSseUrl,
} from "./api";

describe("api request headers", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("does not send content-type for empty POST requests", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ session_id: "session-1" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.startAgent("token-1", "agent-1");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);
    expect(headers.get("Authorization")).toBe("Bearer token-1");
    expect(headers.has("Content-Type")).toBe(false);
  });

  it("keeps json content-type when a request body is present", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "team-1" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.createTeam("token-1", {
      name: "Team One",
      spec: { members: [] },
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);
    expect(headers.get("Content-Type")).toBe("application/json");
  });

  it("preserves caller-provided content-type headers", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ status: "ok" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.respondAcpPermission("token-1", "agent-1", "perm-1", {
      option_id: "approved",
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    const headers = new Headers(init.headers);
    expect(headers.get("Content-Type")).toBe("application/json");
  });

  it("retries idempotent network reads with backoff before surfacing an error", async () => {
    vi.useFakeTimers();
    const fetchMock = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockRejectedValueOnce(new TypeError("Failed to fetch"))
      .mockResolvedValue(
        new Response(JSON.stringify([{ id: "team-1", name: "Team One" }]), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      );
    vi.stubGlobal("fetch", fetchMock);

    const request = api.listTeams("token-1");
    await vi.runAllTimersAsync();

    await expect(request).resolves.toEqual([{ id: "team-1", name: "Team One" }]);
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  it("does not retry mutating requests without explicit opt-in", async () => {
    const fetchMock = vi.fn().mockRejectedValue(new TypeError("Failed to fetch"));
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      api.createTeam("token-1", {
      name: "Team One",
      spec: { members: [] },
      })
    ).rejects.toThrow("Failed to fetch");
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("does not retry HTTP status errors surfaced from responses", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ error: "forbidden" }), {
        status: 403,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(api.listTeams("token-1")).rejects.toMatchObject({
      message: "forbidden",
      status: 403,
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("strips networkRetry before forwarding request init to fetch", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await __testOnlyApiInternals.apiFetch<{ ok: boolean }>("/api/test", "token-1", {
      method: "GET",
      networkRetry: "always",
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit & { networkRetry?: string }];
    expect("networkRetry" in init).toBe(false);
  });

  it("posts thread replies to the channel-thread reply endpoint", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          thread: {
            team_id: "team-1",
            channel_id: "review",
            task_id: "task-1",
            conversation_id: "conversation-1",
            thread_id: "42",
            root_message_id: 42,
          },
          message: {
            message_id: 77,
            conversation_id: "conversation-1",
            task_id: "task-1",
            from_actor_id: "coordinator",
            to_actor_id: null,
            route: "team_thread_reply",
            payload: { type: "chat_message", text: "Thread reply" },
            created_at: 1713480000,
          },
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }
      )
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.replyTeamThread("token-1", "team-1", "review", 42, {
      text: "Thread reply",
      mention_actor_ids: ["worker-1"],
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/teams/team-1/channels/review/threads/42/replies");
    expect(init.method).toBe("POST");
    expect(init.body).toBe(
      JSON.stringify({ text: "Thread reply", mention_actor_ids: ["worker-1"] })
    );
    const headers = new Headers(init.headers);
    expect(headers.get("Authorization")).toBe("Bearer token-1");
    expect(headers.get("Content-Type")).toBe("application/json");
  });

  it("includes priority when listing team tasks with a priority filter", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify([]), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.listTeamTasks("token-1", "team-1", 25, {
      include_shared_thread: true,
      priority: "critical",
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toContain("/api/teams/team-1/tasks?");
    expect(url).toContain("limit=25");
    expect(url).toContain("include_shared_thread=true");
    expect(url).toContain("priority=critical");
  });

  it("builds the team run-context SSE URL with encoded dynamic segments", () => {
    expect(
      buildTeamRunContextSseUrl(
        "https://agenthub.example",
        "team/one",
        "run with spaces",
        "token+value/1"
      )
    ).toBe(
      "https://agenthub.example/sse/teams/team%2Fone/runs/run%20with%20spaces/context?token=token%2Bvalue%2F1"
    );
  });

  it("builds the team runtime SSE URL with encoded dynamic segments", () => {
    expect(
      buildTeamRuntimeSseUrl(
        "https://agenthub.example",
        "team/one",
        "token+value/1"
      )
    ).toBe(
      "https://agenthub.example/sse/teams/team%2Fone/runtime?token=token%2Bvalue%2F1"
    );
  });

  it("lists and mutates team channels through the public channel endpoints", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify([
            {
              team_id: "team-1",
              channel_id: "review",
              task_id: "task-1",
              conversation_id: "conversation-1",
              description: "Review lane",
              created_by_actor_id: "user:user-1",
              created_at: 1713480000,
              updated_at: 1713480000,
            },
          ]),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            team_id: "team-1",
            channel_id: "review",
            task_id: "task-1",
            conversation_id: "conversation-1",
            description: "Review lane",
            created_by_actor_id: "user:user-1",
            created_at: 1713480000,
            updated_at: 1713480000,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            team_id: "team-1",
            channel_id: "review",
            task_id: "task-1",
            conversation_id: "conversation-1",
            description: "Review lane",
            created_by_actor_id: "user:user-1",
            created_at: 1713480000,
            updated_at: 1713480000,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }
        )
      );
    vi.stubGlobal("fetch", fetchMock);

    await api.listTeamChannels("token-1", "team-1");
    await api.createTeamChannel("token-1", "team-1", {
      channel_id: "review",
      description: "Review lane",
    });
    await api.deleteTeamChannel("token-1", "team-1", "review");

    expect(fetchMock).toHaveBeenCalledTimes(3);
    const [listUrl, listInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(listUrl).toBe("/api/teams/team-1/channels");
    expect(listInit.method).toBeUndefined();

    const [createUrl, createInit] = fetchMock.mock.calls[1] as [string, RequestInit];
    expect(createUrl).toBe("/api/teams/team-1/channels");
    expect(createInit.method).toBe("POST");
    expect(createInit.body).toBe(
      JSON.stringify({ channel_id: "review", description: "Review lane" })
    );

    const [deleteUrl, deleteInit] = fetchMock.mock.calls[2] as [string, RequestInit];
    expect(deleteUrl).toBe("/api/teams/team-1/channels/review");
    expect(deleteInit.method).toBe("DELETE");
  });
});
