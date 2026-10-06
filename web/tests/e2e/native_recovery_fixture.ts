import type { Page } from "@playwright/test";
import { mockLoopWorkspace } from "./team_loop_fixture";
import { jsonResponse } from "./team_page_fixture";
import { buildTeamMemberWorkspacePath } from "../../src/pages/team/team_route_helpers";

export async function mockNativeRecovery(page: Page, role = "root") {
  const state = await mockLoopWorkspace(page, role);
  const agent = state.fixture.agents.find((agent) => agent.id === state.actorId)!;
  agent.command = "rara";
  agent.status = "running";
  const localSessionId = `session-${state.teamId}-${state.actorId}`;
  const target = { runtime_id: "current-runtime", session_id: "native-conversation", recovery_id: "recovery-token" };
  const resolutions: unknown[] = [];
  const inputs: unknown[] = [];
  let resolved = false;
  let reads = 0;
  await page.route(`**/api/agents/${state.actorId}/events?*`, route => route.fulfill(jsonResponse([{
    event_id: 1,
    agent_id: state.actorId,
    session_id: localSessionId,
    seq: "event-1",
    ts: state.fixture.now,
    stream: "acp",
    message: JSON.stringify({ type: "agent_message", text: "Interrupted work is retained.", chunk: false,
      meta: { provider_runtime: { provider: "rara", runtime_id: target.runtime_id, native_session_id: target.session_id } } }),
  }])));
  await page.route(`**/api/agents/${state.actorId}/runtime/recovery*`, async (route, request) => {
    if (request.method() === "POST") {
      resolutions.push(request.postDataJSON());
      resolved = true;
      await route.fulfill(jsonResponse({ status: "ok" }));
    } else {
      reads += 1;
      await route.fulfill(jsonResponse({
        local_session_id: localSessionId,
        runtime_id: target.runtime_id,
        session_id: target.session_id,
        recovery: {
          waiting_turn_id: null,
          blocked: resolved ? null : { recovery_id: target.recovery_id, turn_id: "old-turn", reason: "process_lost" },
          decisions: [],
          last_resolution: resolved ? { recovery_id: target.recovery_id, note: "Effects checked" } : null,
        },
      }));
    }
  });
  await page.route(`**/api/agents/${state.actorId}/input`, async (route, request) => {
    inputs.push(request.postDataJSON());
    await route.fulfill(jsonResponse({ status: "ok" }));
  });
  return {
    path: buildTeamMemberWorkspacePath(state.teamId, state.actorId, "agent_acp"),
    localSessionId, target, resolutions, inputs,
    reads: () => reads,
  };
}
