import type { AgentRecord } from "./api";
import { type CursorRef, updateLastEventCursor } from "./event_polling";
import type { OutputLine } from "./output_cache";
import { compareEventOrder } from "./seq_order";

export type LiveOutputBatchAnalysis = {
  outputGroups: Record<string, OutputLine[]>;
  acpGroups: Record<string, OutputLine[]>;
  activeLines: OutputLine[];
  activeAcpLines: OutputLine[];
  nextStatuses: Record<string, AgentRecord["status"]>;
};

function parseRunStatus(message: string): string | null {
  const trimmed = message.trim();
  if (!trimmed.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(trimmed) as { type?: string; status?: string };
    if (parsed?.type === "run_status" && typeof parsed.status === "string") {
      return parsed.status;
    }
  } catch {
    return null;
  }
  return null;
}

function statusToAgentStatus(status: string): AgentRecord["status"] {
  if (status === "running") return "running";
  if (status === "idle") return "idle";
  if (status === "failed") return "failed";
  if (status === "completed" || status === "cancelled") return "stopped";
  return "stopped";
}

export function updateLiveOutputBatchCursors(
  ref: CursorRef,
  lines: OutputLine[]
): void {
  for (const line of lines) {
    updateLastEventCursor(ref, `${line.agent_id}:${line.session_id}`, line);
  }
}

export function analyzeLiveOutputBatch(
  lines: OutputLine[],
  activeAgent: string | null,
  activeSessionId: string | null
): LiveOutputBatchAnalysis {
  const outputGroups: Record<string, OutputLine[]> = {};
  const acpGroups: Record<string, OutputLine[]> = {};
  const activeLines: OutputLine[] = [];
  const activeAcpLines: OutputLine[] = [];
  const nextStatuses: Record<string, AgentRecord["status"]> = {};

  for (const line of lines) {
    const key = `${line.agent_id}:${line.session_id}`;
    (outputGroups[key] ??= []).push(line);

    if (line.stream === "acp") {
      const status = parseRunStatus(line.message);
      if (status) {
        nextStatuses[line.agent_id] = statusToAgentStatus(status);
      }
      (acpGroups[key] ??= []).push(line);
    }

    if (line.agent_id !== activeAgent) {
      continue;
    }
    if (activeSessionId && line.session_id !== activeSessionId) {
      continue;
    }
    activeLines.push(line);
    if (line.stream === "acp") {
      activeAcpLines.push(line);
    }
  }

  return {
    outputGroups,
    acpGroups,
    activeLines,
    activeAcpLines,
    nextStatuses,
  };
}

export function buildLatestLiveSessionMap(
  lines: OutputLine[]
): Record<string, string> {
  const latestByAgent = new Map<string, OutputLine>();
  for (const line of lines) {
    const previous = latestByAgent.get(line.agent_id);
    if (!previous || compareEventOrder(line, previous) > 0) {
      latestByAgent.set(line.agent_id, line);
    }
  }
  return Object.fromEntries(
    Array.from(latestByAgent.entries()).map(([agentId, line]) => [
      agentId,
      line.session_id,
    ])
  );
}

export function resolveLiveSessionSwitch(
  lines: OutputLine[],
  activeAgent: string | null,
  activeSessionId: string | null
): string | null {
  if (!activeAgent) return null;
  let latestReplacement: OutputLine | null = null;
  for (const line of lines) {
    if (line.agent_id !== activeAgent) continue;
    if (activeSessionId && line.session_id === activeSessionId) continue;
    if (!latestReplacement || compareEventOrder(line, latestReplacement) > 0) {
      latestReplacement = line;
    }
  }
  return latestReplacement?.session_id ?? null;
}

export function dispatchLiveOutputBatch(params: {
  cursorRef: CursorRef;
  lines: OutputLine[];
  activeAgent: string | null;
  activeSessionId: string | null;
  onStatuses: (nextStatuses: Record<string, AgentRecord["status"]>) => void;
  onOutputGroup: (key: string, grouped: OutputLine[]) => void;
  onAcpGroup: (key: string, grouped: OutputLine[]) => void;
}): Pick<LiveOutputBatchAnalysis, "activeLines" | "activeAcpLines"> {
  const {
    cursorRef,
    lines,
    activeAgent,
    activeSessionId,
    onStatuses,
    onOutputGroup,
    onAcpGroup,
  } = params;
  updateLiveOutputBatchCursors(cursorRef, lines);
  const analyzed = analyzeLiveOutputBatch(lines, activeAgent, activeSessionId);
  if (Object.keys(analyzed.nextStatuses).length > 0) {
    onStatuses(analyzed.nextStatuses);
  }
  for (const [key, grouped] of Object.entries(analyzed.outputGroups)) {
    onOutputGroup(key, grouped);
  }
  for (const [key, grouped] of Object.entries(analyzed.acpGroups)) {
    onAcpGroup(key, grouped);
  }
  return {
    activeLines: analyzed.activeLines,
    activeAcpLines: analyzed.activeAcpLines,
  };
}

export function routeLiveOutputBatch(params: {
  cursorRef: CursorRef;
  lines: OutputLine[];
  activeAgent: string | null;
  activeSessionId: string | null;
  updateAgents: (updater: (prev: AgentRecord[]) => AgentRecord[]) => void;
  onOutputGroup: (key: string, grouped: OutputLine[]) => void;
  onAcpGroup: (key: string, grouped: OutputLine[]) => void;
}): Pick<LiveOutputBatchAnalysis, "activeLines" | "activeAcpLines"> {
  const {
    cursorRef,
    lines,
    activeAgent,
    activeSessionId,
    updateAgents,
    onOutputGroup,
    onAcpGroup,
  } = params;
  return dispatchLiveOutputBatch({
    cursorRef,
    lines,
    activeAgent,
    activeSessionId,
    onStatuses: (nextStatuses) => {
      updateAgents((prev) =>
        prev.map((agent) => {
          const nextStatus = nextStatuses[agent.id];
          return nextStatus ? { ...agent, status: nextStatus } : agent;
        })
      );
    },
    onOutputGroup,
    onAcpGroup,
  });
}

function isValidOutputPayload(
  payload: unknown
): payload is {
  event_id: number;
  agent_id: string;
  session_id: string;
  seq: string;
  ts: number;
  stream: OutputLine["stream"];
  message: string;
} {
  if (!payload || typeof payload !== "object") return false;
  const candidate = payload as {
    event_id?: unknown;
    agent_id?: unknown;
    session_id?: unknown;
    seq?: unknown;
    ts?: unknown;
    stream?: unknown;
    message?: unknown;
  };
  if (typeof candidate.event_id !== "number") return false;
  if (typeof candidate.agent_id !== "string" || !candidate.agent_id) return false;
  if (typeof candidate.session_id !== "string" || !candidate.session_id) return false;
  if (typeof candidate.seq !== "string" || !candidate.seq) return false;
  if (typeof candidate.ts !== "number") return false;
  if (typeof candidate.message !== "string") return false;
  if (
    candidate.stream !== "stdout" &&
    candidate.stream !== "stderr" &&
    candidate.stream !== "system" &&
    candidate.stream !== "acp"
  ) {
    return false;
  }
  return true;
}

export function normalizeSseOutputLines(message: unknown): OutputLine[] {
  if (!message || typeof message !== "object") return [];
  const parsed = message as { type?: unknown; payload?: unknown };
  if (parsed.type === "output" || parsed.type === "acp") {
    return isValidOutputPayload(parsed.payload) ? [parsed.payload] : [];
  }
  if (parsed.type !== "batch" || !Array.isArray(parsed.payload)) {
    return [];
  }
  return parsed.payload.filter(isValidOutputPayload);
}
