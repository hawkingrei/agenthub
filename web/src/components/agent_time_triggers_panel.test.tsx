// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AgentTimeTriggersPanel } from "./agent_time_triggers_panel";
import { api, type AgentTimeTriggerRecord } from "../api";

vi.mock("../api", () => ({ api: { listAgentTimeTriggers: vi.fn() } }));
const list = vi.mocked(api.listAgentTimeTriggers);
let container: HTMLDivElement;
let root: Root;
const record: AgentTimeTriggerRecord = {
  id: "reminder-1", agent_id: "a", kind: "time", created_by_actor_id: "a",
  message_text: "Check task state", fire_at: 123, status: "fired",
  created_at: 100, updated_at: 123, fired_at: 123, last_error: null,
};

function createTriggerRequest() {
  let resolve!: (records: AgentTimeTriggerRecord[]) => void;
  const promise = new Promise<AgentTimeTriggerRecord[]>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.useFakeTimers();
  list.mockReset();
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
});
afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
  vi.useRealTimers();
});

it("distinguishes submission from execution and displays retry provenance", async () => {
  const request = createTriggerRequest();
  list.mockReturnValueOnce(request.promise);
  await act(async () => root.render(<AgentTimeTriggersPanel agentId="a" authToken="token" />));
  expect(container.textContent).toContain("Loading...");
  await act(async () => {
    request.resolve([
      record,
      {
        ...record,
        id: "retry",
        status: "scheduled",
        fired_at: null,
        last_error: "agent not running",
        attempt: 2,
        next_attempt_at: Math.floor(Date.now() / 1000) + 30,
        source: { reference: "task:123" },
      },
    ]);
    await request.promise;
  });
  expect(container.textContent).toContain("submitted");
  expect(container.textContent).toContain("execution is not confirmed");
  expect(container.textContent).not.toContain("fired");
  expect(container.textContent).toContain("Retry in <1 min · attempt 2");
  expect(container.textContent).toContain("Source: task:123");
});

it("refreshes serially and ignores an old identity's response", async () => {
  const oldRequest = createTriggerRequest();
  const currentRequest = createTriggerRequest();
  const refreshRequest = createTriggerRequest();
  list
    .mockReturnValueOnce(oldRequest.promise)
    .mockReturnValueOnce(currentRequest.promise)
    .mockReturnValueOnce(refreshRequest.promise);
  await act(async () => root.render(<AgentTimeTriggersPanel agentId="a" authToken="token" />));
  await act(async () => vi.advanceTimersByTimeAsync(20_000));
  expect(list).toHaveBeenCalledTimes(1);
  await act(async () => root.render(<AgentTimeTriggersPanel agentId="b" authToken="token" />));
  expect(list).toHaveBeenNthCalledWith(2, "token", "b", 100);
  expect(container.textContent).toContain("Loading...");
  await act(async () => {
    currentRequest.resolve([
      { ...record, agent_id: "b", message_text: "Current agent reminder" },
    ]);
    await currentRequest.promise;
  });
  expect(container.textContent).toContain("Current agent reminder");
  await act(async () => {
    oldRequest.resolve([record]);
    await oldRequest.promise;
  });
  expect(container.textContent).toContain("Current agent reminder");
  expect(container.textContent).not.toContain("Check task state");
  await act(async () => vi.advanceTimersByTimeAsync(9_999));
  expect(list).toHaveBeenCalledTimes(2);
  await act(async () => vi.advanceTimersByTimeAsync(1));
  expect(list).toHaveBeenNthCalledWith(3, "token", "b", 100);
  await act(async () => vi.advanceTimersByTimeAsync(20_000));
  expect(list).toHaveBeenCalledTimes(3);
});
