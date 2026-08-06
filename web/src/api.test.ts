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
    const fetchMock = vi.fn().mockImplementation(() =>
      Promise.resolve(new Response(JSON.stringify({ status: "ok" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }))
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

  it("uploads Team images through the scoped image endpoint", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "upload-1",
          owner_scope: "teams/team-1",
          backend: "s3",
          object_key: "images/teams/team-1/upload-1.png",
          original_filename: "diagram.png",
          content_type: "image/png",
          size_bytes: 4,
          sha256: "sha",
          public_url: "https://cdn.example.test/upload-1.png",
          created_by_actor_id: "human",
          publish_state: "published",
          created_at: 1,
          published_at: 1,
          cleanup_after: null,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }
      )
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.uploadTeamImage("token-1", "team-1", {
      file_name: "diagram.png",
      content_type: "image/png",
      bytes_base64: "AQIDBA==",
      expected_size_bytes: 4,
      expected_sha256: "sha",
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/teams/team-1/images");
    expect(init.method).toBe("POST");
    expect(init.body).toBe(
      JSON.stringify({
        file_name: "diagram.png",
        content_type: "image/png",
        bytes_base64: "AQIDBA==",
        expected_size_bytes: 4,
        expected_sha256: "sha",
      })
    );
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

  it("creates a task from a channel message through the narrow channel route", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ task: { id: "task-1" } }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    await api.createTeamTaskFromChannelMessage(
      "token-1",
      "team/1",
      "review lane",
      42,
      {
        priority: "high",
        context: { source: "test" },
      }
    );

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe(
      "/api/teams/team%2F1/channels/review%20lane/messages/42/tasks"
    );
    expect(init.method).toBe("POST");
    expect(init.body).toBe(
      JSON.stringify({ priority: "high", context: { source: "test" } })
    );
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

  it("uses the admin linker endpoints for Slock linker operations", async () => {
    const linker = {
      linker_id: "slock-primary",
      connector_id: "slock",
      display_name: "Slock",
      status: "configured",
      api_origin: "https://api.slock.ai",
      client_id: "agenthub",
      return_url: "https://agenthub.example.com/api/linkers/slock/callback",
      scopes: ["identity", "openid", "profile"],
      client_secret_configured: true,
      token_configured: false,
      token_type: null,
      granted_scopes: [],
      expires_at: null,
      principal: null,
      updated_at: 1,
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify([linker]), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(linker), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            linker_id: "slock-primary",
            state: "state-1",
            expires_at: 100,
            return_url: "https://agenthub.example.com/api/linkers/slock/callback",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ ...linker, status: "connected" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      );
    vi.stubGlobal("fetch", fetchMock);

    const payload = {
      api_origin: "https://api.slock.ai",
      client_id: "agenthub",
      client_secret: "secret-1",
      return_url: "https://agenthub.example.com/api/linkers/slock/callback",
      scopes: ["identity", "openid", "profile"],
    };
    await api.listLinkers("token-1");
    await api.upsertSlockLinker("token-1", payload);
    await api.createSlockLinkAttempt("token-1");
    await api.exchangeSlockCode("token-1", {
      callback_url:
        "https://agenthub.example.com/api/linkers/slock/callback?code=callback-code",
    });

    expect(fetchMock).toHaveBeenCalledTimes(4);
    const [listUrl, listInit] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(listUrl).toBe("/api/admin/linkers");
    expect(listInit.method).toBeUndefined();

    const [upsertUrl, upsertInit] = fetchMock.mock.calls[1] as [
      string,
      RequestInit,
    ];
    expect(upsertUrl).toBe("/api/admin/linkers/slock");
    expect(upsertInit.method).toBe("PUT");
    expect(upsertInit.body).toBe(JSON.stringify(payload));

    const [attemptUrl, attemptInit] = fetchMock.mock.calls[2] as [
      string,
      RequestInit,
    ];
    expect(attemptUrl).toBe("/api/admin/linkers/slock/link_attempts");
    expect(attemptInit.method).toBe("POST");

    const [exchangeUrl, exchangeInit] = fetchMock.mock.calls[3] as [
      string,
      RequestInit,
    ];
    expect(exchangeUrl).toBe("/api/admin/linkers/slock/exchange");
    expect(exchangeInit.method).toBe("POST");
    expect(exchangeInit.body).toBe(
      JSON.stringify({
        callback_url:
          "https://agenthub.example.com/api/linkers/slock/callback?code=callback-code",
      })
    );
  });

  it("routes representative API facade calls to stable endpoints", async () => {
    const fetchMock = vi.fn().mockImplementation(() =>
      Promise.resolve(
        new Response(JSON.stringify({ status: "ok" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
      )
    );
    vi.stubGlobal("fetch", fetchMock);

    const token = "token-1";
    const agentId = "agent/one";
    const teamId = "team/one";
    const runId = "run/one";
    const taskId = "task/one";
    const stepId = "step/one";
    const nodeId = "node/one";
    const permissionId = "permission/one";
    const requests: Array<{
      call: () => Promise<unknown>;
      url: string;
      method?: string;
      body?: unknown;
    }> = [
      {
        call: () =>
          api.registerStart("root", "Root User", "root", "pw", "laptop", "invite-token"),
        url: "/api/auth/register/start",
        method: "POST",
        body: {
          username: "root",
          display_name: "Root User",
          role: "root",
          password: "pw",
          device_name: "laptop",
          team_invite_token: "invite-token",
        },
      },
      {
        call: () => api.registerFinish("challenge-1", { id: "credential-1" }),
        url: "/api/auth/register/finish",
        method: "POST",
        body: { challenge_id: "challenge-1", credential: { id: "credential-1" } },
      },
      {
        call: () => api.loginStart("root", "pw"),
        url: "/api/auth/login/start",
        method: "POST",
        body: { username: "root", password: "pw" },
      },
      {
        call: () => api.loginFinish("challenge-1", { id: "credential-1" }),
        url: "/api/auth/login/finish",
        method: "POST",
        body: { challenge_id: "challenge-1", credential: { id: "credential-1" } },
      },
      {
        call: () => api.loginRegisterFinish("challenge-1", { id: "credential-1" }),
        url: "/api/auth/login/register_finish",
        method: "POST",
        body: { challenge_id: "challenge-1", credential: { id: "credential-1" } },
      },
      {
        call: () =>
          api.joinStart({
            token: "join-token",
            pin: "123456",
            username: "worker",
            display_name: "Worker",
            password: "pw",
            device_name: "laptop",
          }),
        url: "/api/join/start",
        method: "POST",
        body: {
          token: "join-token",
          pin: "123456",
          username: "worker",
          display_name: "Worker",
          password: "pw",
          device_name: "laptop",
        },
      },
      {
        call: () => api.joinFinish("challenge-1", { id: "credential-1" }),
        url: "/api/join/finish",
        method: "POST",
        body: { challenge_id: "challenge-1", credential: { id: "credential-1" } },
      },
      {
        call: () => api.authStatus(),
        url: "/api/auth/status",
      },
      {
        call: () => api.getRuntimeDefaults(token),
        url: "/api/settings/defaults",
      },
      {
        call: () => api.listSafePaths(token),
        url: "/api/admin/safe_paths",
      },
      {
        call: () => api.addSafePath(token, "/workspace"),
        url: "/api/admin/safe_paths",
        method: "POST",
        body: { path: "/workspace" },
      },
      {
        call: () => api.deleteSafePath(token, "/workspace"),
        url: "/api/admin/safe_paths",
        method: "DELETE",
        body: { path: "/workspace" },
      },
      {
        call: () => api.revokeDevice(token, "device/one"),
        url: "/api/admin/devices/device%2Fone/revoke",
        method: "POST",
      },
      {
        call: () => api.listDevices(token),
        url: "/api/admin/devices",
      },
      {
        call: () => api.listAudits(token, 25),
        url: "/api/admin/audits?limit=25",
      },
      {
        call: () => api.joinStartAdmin(token),
        url: "/api/admin/join/start",
        method: "POST",
      },
      {
        call: () => api.getAdminSettings(token),
        url: "/api/admin/settings",
      },
      {
        call: () => api.setPasskeyEnabled(token, true),
        url: "/api/admin/settings/passkey",
        method: "POST",
        body: { enabled: true },
      },
      {
        call: () => api.getTeamPromptDefaults(token),
        url: "/api/teams/prompt_defaults",
      },
      {
        call: () => api.listTeams(token),
        url: "/api/teams",
      },
      {
        call: () => api.createTeam(token, { name: "Team One", spec: { members: [] } }),
        url: "/api/teams",
        method: "POST",
        body: { name: "Team One", spec: { members: [] } },
      },
      {
        call: () => api.listTeamspaceMembers(token, teamId),
        url: "/api/teams/team%2Fone/members",
      },
      {
        call: () => api.revokeTeamspaceMember(token, teamId, "user/one"),
        url: "/api/teams/team%2Fone/members/user%2Fone",
        method: "DELETE",
      },
      {
        call: () => api.createTeamspaceInvite(token, teamId, { role: "contributor" }),
        url: "/api/teams/team%2Fone/invites",
        method: "POST",
        body: { role: "contributor" },
      },
      {
        call: () => api.acceptTeamspaceInvite(token, "invite-token"),
        url: "/api/teams/invites/accept",
        method: "POST",
        body: { token: "invite-token" },
      },
      {
        call: () => api.getTeam(token, teamId),
        url: "/api/teams/team%2Fone",
      },
      {
        call: () =>
          api.updateTeamSpec(token, teamId, {
            spec: { members: [] },
            expected_updated_at: 1,
          }),
        url: "/api/teams/team%2Fone/spec",
        method: "PUT",
        body: { spec: { members: [] }, expected_updated_at: 1 },
      },
      {
        call: () => api.getTeamRuntime(token, teamId),
        url: "/api/teams/team%2Fone/runtime",
      },
      {
        call: () => api.startTeam(token, teamId),
        url: "/api/teams/team%2Fone/start",
        method: "POST",
      },
      {
        call: () => api.stopTeam(token, teamId),
        url: "/api/teams/team%2Fone/stop",
        method: "POST",
      },
      {
        call: () => api.forceTeamMemberNewSession(token, teamId, "member/one"),
        url: "/api/teams/team%2Fone/members/member%2Fone/force_new_session",
        method: "POST",
      },
      {
        call: () => api.deleteTeam(token, teamId),
        url: "/api/teams/team%2Fone",
        method: "DELETE",
      },
      {
        call: () => api.getTeamSharedThread(token, teamId),
        url: "/api/teams/team%2Fone/shared_thread",
      },
      {
        call: () => api.ensureTeamSharedThread(token, teamId),
        url: "/api/teams/team%2Fone/shared_thread",
        method: "POST",
      },
      {
        call: () => api.getTeamTask(token, teamId, taskId),
        url: "/api/teams/team%2Fone/tasks/task%2Fone",
      },
      {
        call: () =>
          api.updateTeamTask(token, teamId, taskId, {
            status: "completed",
            assigned_member_id: null,
          }),
        url: "/api/teams/team%2Fone/tasks/task%2Fone",
        method: "PATCH",
        body: { status: "completed", assigned_member_id: null },
      },
      {
        call: () =>
          api.handoffTeamTask(token, teamId, taskId, {
            assigned_member_id: "member-2",
            reason: "Reassigned by the owner",
          }),
        url: "/api/teams/team%2Fone/tasks/task%2Fone/handoff",
        method: "POST",
        body: {
          assigned_member_id: "member-2",
          reason: "Reassigned by the owner",
        },
      },
      {
        call: () =>
          api.sendTeamTaskMessage(token, teamId, taskId, {
            payload: { type: "chat_message", text: "hello" },
          }),
        url: "/api/teams/team%2Fone/tasks/task%2Fone/messages",
        method: "POST",
        body: { payload: { type: "chat_message", text: "hello" } },
      },
      {
        call: () =>
          api.listTeamTaskMessages(token, teamId, taskId, {
            limit: 10,
            before_id: 20,
          }),
        url: "/api/teams/team%2Fone/tasks/task%2Fone/messages?limit=10&before_id=20",
      },
      {
        call: () => api.compileTeamTaskRunPreview(token, teamId, taskId, {}),
        url: "/api/teams/team%2Fone/tasks/task%2Fone/compile_run_preview",
        method: "POST",
        body: {},
      },
      {
        call: () =>
          api.listTeamRuns(token, teamId, {
            limit: 10,
            status: "working",
            before_created_at: 20,
          }),
        url: "/api/teams/team%2Fone/runs?limit=10&status=working&before_created_at=20",
      },
      {
        call: () => api.createTeamRun(token, teamId, { input: { text: "run" } }),
        url: "/api/teams/team%2Fone/runs",
        method: "POST",
        body: { input: { text: "run" } },
      },
      {
        call: () => api.getTeamRun(token, runId),
        url: "/api/teams/runs/run%2Fone",
      },
      {
        call: () => api.getTeamRunSnapshot(token, runId, { event_limit: 5 }),
        url: "/api/teams/runs/run%2Fone/snapshot?event_limit=5",
      },
      {
        call: () => api.cancelTeamRun(token, runId),
        url: "/api/teams/runs/run%2Fone/cancel",
        method: "POST",
      },
      {
        call: () => api.resumeTeamRun(token, runId),
        url: "/api/teams/runs/run%2Fone/resume",
        method: "POST",
      },
      {
        call: () => api.restartTeamRun(token, runId),
        url: "/api/teams/runs/run%2Fone/restart",
        method: "POST",
      },
      {
        call: () => api.listTeamRunEvents(token, runId, 10, 20),
        url: "/api/teams/runs/run%2Fone/events?limit=10&before_id=20",
      },
      {
        call: () => api.listTeamRunSteps(token, runId),
        url: "/api/teams/runs/run%2Fone/steps",
      },
      {
        call: () =>
          api.submitTeamRunStep(token, runId, {
            step_key: "implement",
            member_id: "worker",
          }),
        url: "/api/teams/runs/run%2Fone/steps",
        method: "POST",
        body: { step_key: "implement", member_id: "worker" },
      },
      {
        call: () => api.startTeamRunStep(token, runId, stepId, {}),
        url: "/api/teams/runs/run%2Fone/steps/step%2Fone/start",
        method: "POST",
        body: {},
      },
      {
        call: () => api.completeTeamRunStep(token, runId, stepId, { output: "ok" }),
        url: "/api/teams/runs/run%2Fone/steps/step%2Fone/complete",
        method: "POST",
        body: { output: "ok" },
      },
      {
        call: () => api.failTeamRunStep(token, runId, stepId, { error_text: "failed" }),
        url: "/api/teams/runs/run%2Fone/steps/step%2Fone/fail",
        method: "POST",
        body: { error_text: "failed" },
      },
      {
        call: () => api.setTeamRunStepInputRequired(token, runId, stepId, {}),
        url: "/api/teams/runs/run%2Fone/steps/step%2Fone/input_required",
        method: "POST",
        body: {},
      },
      {
        call: () => api.resumeTeamRunStep(token, runId, stepId, { input: "next" }),
        url: "/api/teams/runs/run%2Fone/steps/step%2Fone/resume",
        method: "POST",
        body: { input: "next" },
      },
      {
        call: () =>
          api.sendTeamRunMessage(token, runId, {
            from_actor_id: "leader",
            to_actor_id: "worker",
            payload: { text: "hello" },
          }),
        url: "/api/teams/runs/run%2Fone/messages/send",
        method: "POST",
        body: {
          from_actor_id: "leader",
          to_actor_id: "worker",
          payload: { text: "hello" },
        },
      },
      {
        call: () =>
          api.listTeamRunInbox(token, runId, {
            actor_id: "worker",
            limit: 10,
            after_id: 20,
            include_delivered: true,
          }),
        url: "/api/teams/runs/run%2Fone/messages/inbox?actor_id=worker&limit=10&after_id=20&include_delivered=true",
      },
      {
        call: () => api.ackTeamRunMessage(token, runId, 7, "worker"),
        url: "/api/teams/runs/run%2Fone/messages/7/ack",
        method: "POST",
        body: { actor_id: "worker" },
      },
      {
        call: () => api.listAgents(token),
        url: "/api/agents",
      },
      {
        call: () => api.listAgentNodes(token),
        url: "/api/agent_nodes",
      },
      {
        call: () => api.getAgentNodeJoinBootstrap(token),
        url: "/api/agent_nodes/bootstrap",
      },
      {
        call: () => api.createAgentNode(token, {
          id: nodeId,
          name: "Node One",
          grpc_target: "http://127.0.0.1:50051",
        }),
        url: "/api/agent_nodes",
        method: "POST",
        body: {
          id: nodeId,
          name: "Node One",
          grpc_target: "http://127.0.0.1:50051",
        },
      },
      {
        call: () =>
          api.updateAgentNode(token, nodeId, {
            name: "Node One",
            grpc_target: "http://127.0.0.1:50051",
          }),
        url: "/api/agent_nodes/node%2Fone",
        method: "PATCH",
        body: {
          name: "Node One",
          grpc_target: "http://127.0.0.1:50051",
        },
      },
      {
        call: () => api.deleteAgentNode(token, nodeId),
        url: "/api/agent_nodes/node%2Fone",
        method: "DELETE",
      },
      {
        call: () => api.getAgent(token, agentId),
        url: "/api/agents/agent%2Fone",
      },
      {
        call: () => api.getAgentDiscoveryCard(token, agentId),
        url: "/api/agents/agent%2Fone/.well-known/agent-card",
      },
      {
        call: () => api.sendInput(token, agentId, "hello", "message-1", "session-1"),
        url: "/api/agents/agent%2Fone/input",
        method: "POST",
        body: { input: "hello", message_id: "message-1", session_id: "session-1" },
      },
      {
        call: () =>
          api.createAgent(token, {
            name: "Agent One",
            workdir: "/tmp",
            command: "codex",
            args: [],
            worktree_mode: "use_existing",
            code_mode: true,
          }),
        url: "/api/agents",
        method: "POST",
        body: {
          name: "Agent One",
          workdir: "/tmp",
          command: "codex",
          args: [],
          worktree_mode: "use_existing",
          code_mode: true,
        },
      },
      {
        call: () => api.listAgentEvents(token, agentId, 10, "session-1", 20),
        url: "/api/agents/agent%2Fone/events?limit=10&session_id=session-1&before_id=20",
      },
      {
        call: () => api.getAgentEvent(token, agentId, 7),
        url: "/api/agents/agent%2Fone/events/7",
      },
      {
        call: () => api.setAgentCodeMode(token, agentId, true),
        url: "/api/agents/agent%2Fone/code_mode",
        method: "POST",
        body: { code_mode: true },
      },
      {
        call: () => api.setAgentCodexAcpDefaultMode(token, agentId, "read-only"),
        url: "/api/agents/agent%2Fone/codex_acp_default_mode",
        method: "POST",
        body: { mode_id: "read-only" },
      },
      {
        call: () =>
          api.setAgentRuntimeProfile(token, agentId, {
            runtime_model: "gpt-5",
            thinking_level: "high",
          }),
        url: "/api/agents/agent%2Fone/runtime_profile",
        method: "POST",
        body: { runtime_model: "gpt-5", thinking_level: "high" },
      },
      {
        call: () =>
          api.setAgentLoop(token, agentId, {
            enabled: true,
            idle_seconds: 30,
            prompt: "continue",
          }),
        url: "/api/agents/agent%2Fone/agent_loop",
        method: "POST",
        body: { enabled: true, idle_seconds: 30, prompt: "continue" },
      },
      {
        call: () => api.clearAcpSession(token, agentId, "codex"),
        url: "/api/agents/agent%2Fone/acp/session/clear",
        method: "POST",
        body: { provider: "codex" },
      },
      {
        call: () => api.setAcpMode(token, agentId, "plan"),
        url: "/api/agents/agent%2Fone/acp/mode",
        method: "POST",
        body: { mode_id: "plan" },
      },
      {
        call: () => api.setAcpModel(token, agentId, "gpt-5"),
        url: "/api/agents/agent%2Fone/acp/model",
        method: "POST",
        body: { model_id: "gpt-5" },
      },
      {
        call: () => api.setAcpConfig(token, agentId, "approval", "on-request"),
        url: "/api/agents/agent%2Fone/acp/config",
        method: "POST",
        body: { config_id: "approval", value: "on-request" },
      },
      {
        call: () => api.cancelAcp(token, agentId),
        url: "/api/agents/agent%2Fone/acp/cancel",
        method: "POST",
      },
      {
        call: () => api.startAgent(token, agentId),
        url: "/api/agents/agent%2Fone/start",
        method: "POST",
      },
      {
        call: () => api.stopAgent(token, agentId),
        url: "/api/agents/agent%2Fone/stop",
        method: "POST",
      },
      {
        call: () => api.deleteAgent(token, agentId),
        url: "/api/agents/agent%2Fone",
        method: "DELETE",
      },
      {
        call: () => api.listAgentTimeTriggers(token, agentId, 10),
        url: "/api/agents/agent%2Fone/triggers?limit=10",
      },
      {
        call: () => api.listAcpPermissions(token, agentId, "pending"),
        url: "/api/agents/agent%2Fone/permissions?status=pending",
      },
      {
        call: () => api.respondAcpPermission(token, agentId, permissionId, {
          option_id: "allow",
        }),
        url: "/api/agents/agent%2Fone/permissions/permission%2Fone/respond",
        method: "POST",
        body: { option_id: "allow" },
      },
      {
        call: () => api.getVapidPublicKey(),
        url: "/api/push/vapid_public",
      },
      {
        call: () => api.getVapidInfo(token),
        url: "/api/push/vapid_info",
      },
      {
        call: () => api.rotateVapid(token),
        url: "/api/push/vapid_rotate",
        method: "POST",
      },
      {
        call: () => api.subscribePush(token, { endpoint: "https://push.example" }),
        url: "/api/push/subscribe",
        method: "POST",
        body: { endpoint: "https://push.example" },
      },
    ];

    for (const request of requests) {
      await request.call();
    }

    expect(fetchMock).toHaveBeenCalledTimes(requests.length);
    requests.forEach((request, index) => {
      const [url, init] = fetchMock.mock.calls[index] as [string, RequestInit];
      expect(url).toBe(request.url);
      expect(init.method).toBe(request.method);
      if (request.body !== undefined) {
        expect(init.body).toBe(JSON.stringify(request.body));
      }
    });
  });
});
