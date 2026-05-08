// @vitest-environment jsdom
import { act, type Dispatch, type SetStateAction } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentRecord } from "./api";
import type { OutputLine } from "./output_cache";
import { useAppOutputCache } from "./use_app_output_cache";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function buildOutputLine(overrides: Partial<OutputLine> = {}): OutputLine {
  return {
    agent_id: "agent-1",
    session_id: "session-1",
    event_id: 1,
    seq: "1",
    ts: 1,
    stream: "acp",
    message: JSON.stringify({ type: "permission_request", permission_id: "perm-1" }),
    ...overrides,
  };
}

function HookHarness({
  onReady,
  onAcpPermissionSignal,
}: {
  onReady: (consumeLiveOutputBatch: (lines: OutputLine[]) => void) => void;
  onAcpPermissionSignal: (agentIds: string[]) => void;
}) {
  const setAgents = vi.fn() as unknown as Dispatch<SetStateAction<AgentRecord[]>>;
  const { consumeLiveOutputBatch } = useAppOutputCache(
    { token: "token-1", userId: "user-1", username: "user-1", role: "admin" },
    "agent-1",
    "running",
    setAgents,
    onAcpPermissionSignal
  );
  onReady(consumeLiveOutputBatch);
  return null;
}

describe("useAppOutputCache", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.restoreAllMocks();
  });

  it("forwards ACP permission live signals from consumed SSE output", () => {
    const onAcpPermissionSignal = vi.fn();
    let consume: ((lines: OutputLine[]) => void) | null = null;

    act(() => {
      root.render(
        <HookHarness
          onReady={(callback) => {
            consume = callback;
          }}
          onAcpPermissionSignal={onAcpPermissionSignal}
        />
      );
    });

    act(() => {
      consume?.([buildOutputLine()]);
    });

    expect(onAcpPermissionSignal).toHaveBeenCalledWith(["agent-1"]);
  });
});
