import { compareEventOrder } from "./seq_order";

export type AcpToolCall = {
  id: string;
  title: string;
  kind?: string;
  status?: string;
  content?: string;
  raw_input?: unknown;
  raw_output?: unknown;
  terminal_output?: string;
  session_id?: string | null;
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type AcpMessage = {
  kind: "agent_message" | "agent_thought" | "user_message";
  text: string;
  session_id?: string | null;
  message_id?: string | null;
  chunk_index?: number | null;
  seq?: string;
  event_id?: number;
  chunk: boolean;
  ts?: number;
};

export type AcpPlanEntry = {
  content: string;
  priority?: string;
  status?: string;
  meta?: unknown;
};

export type AcpPlan = {
  entries: AcpPlanEntry[];
  meta?: unknown;
};

export type AcpPlanView = AcpPlan & {
  session_id?: string | null;
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type AcpCommand = {
  name: string;
  description: string;
  input?: unknown;
  meta?: unknown;
};

export type AcpRawEvent = {
  ts: number;
  type: string;
  payload: unknown;
};

export type AcpRunStatus = {
  status: string;
  session_id?: string;
};

export type AcpEventLine = {
  ts: number;
  seq?: string;
  event_id?: number;
  stream: string;
  message: string;
  session_id?: string | null;
};

export type AcpView = {
  hasAcp: boolean;
  toolCalls: AcpToolCall[];
  messages: AcpMessage[];
  rawEvents: AcpRawEvent[];
  plan: AcpPlanView | null;
  commands: AcpCommand[];
  currentMode: string | null;
  runStatus: AcpRunStatus | null;
  thinkingStartTs: number | null;
};

export function buildAcpView(events: AcpEventLine[]): AcpView {
  const toolCalls: AcpToolCall[] = [];
  const toolCallMap = new Map<string, AcpToolCall>();
  const messages: AcpMessage[] = [];
  const messageIndex = new Map<string, number>();
  const messageChunks = new Map<string, Map<number, string>>();
  const rawEvents: AcpRawEvent[] = [];
  let plan: AcpPlanView | null = null;
  let commands: AcpCommand[] = [];
  let currentMode: string | null = null;
  let runStatus: AcpRunStatus | null = null;
  let thinkingStartTs: number | null = null;
  let inThinking = false;

  for (const event of events) {
    if (event.stream !== "acp") continue;
    const parsed = parseAcpEvent(event.message);
    if (!parsed) continue;
    rawEvents.push({
      ts: event.ts,
      type: String(parsed.type ?? "unknown"),
      payload: parsed,
    });
    if (
      parsed.type === "agent_message" ||
      parsed.type === "agent_thought" ||
      parsed.type === "user_message"
    ) {
      const text = String(parsed.text ?? "");
      const last = messages[messages.length - 1];
      const is_chunk = parsed.chunk === true;
      const messageId =
        typeof parsed.message_id === "string" ? parsed.message_id : null;
      const chunkIndex = parseChunkIndex(parsed.chunk_index);
      if (
        messageId &&
        messages.some(
          (msg) =>
            msg.kind === "user_message" &&
            msg.message_id === messageId &&
            msg.session_id === (event.session_id ?? null)
        )
      ) {
        continue;
      }
      if (
        parsed.type === "user_message" &&
        last &&
        last.kind === "user_message" &&
        last.session_id === (event.session_id ?? null) &&
        last.text === text
      ) {
        continue;
      }
      if (is_chunk && messageId && chunkIndex != null) {
        const key = `${parsed.type}:${event.session_id ?? "none"}:${messageId}`;
        const existingIndex = messageIndex.get(key);
        if (existingIndex != null) {
          const existing = messages[existingIndex];
          const chunks = messageChunks.get(key) ?? new Map<number, string>();
          chunks.set(chunkIndex, text);
          messageChunks.set(key, chunks);
          existing.text = joinChunks(chunks);
          existing.seq = event.seq ?? existing.seq;
          existing.event_id = event.event_id ?? existing.event_id;
          existing.ts = event.ts ?? existing.ts;
          existing.chunk = true;
          if (existing.message_id == null) {
            existing.message_id = messageId;
          }
          existing.chunk_index = chunkIndex;
        } else {
          const chunks = new Map<number, string>();
          chunks.set(chunkIndex, text);
          messageChunks.set(key, chunks);
          const next = {
            kind: parsed.type,
            text: joinChunks(chunks),
            session_id: event.session_id ?? null,
            message_id: messageId,
            chunk_index: chunkIndex,
            seq: event.seq,
            event_id: event.event_id,
            chunk: true,
            ts: event.ts,
          } satisfies AcpMessage;
          messages.push(next);
          messageIndex.set(key, messages.length - 1);
        }
      } else {
        const shouldMergeChunk =
          is_chunk &&
          last &&
          last.chunk &&
          last.kind === parsed.type &&
          last.session_id === (event.session_id ?? null) &&
          (messageId == null ||
            last.message_id == null ||
            last.message_id === messageId);
        if (shouldMergeChunk) {
          last.text = `${last.text}${text}`;
          last.seq = event.seq ?? last.seq;
          last.event_id = event.event_id ?? last.event_id;
          last.ts = event.ts ?? last.ts;
          if (messageId && last.message_id == null) {
            last.message_id = messageId;
          }
        } else {
          messages.push({
            kind: parsed.type,
            text,
            session_id: event.session_id ?? null,
            message_id: messageId,
            chunk_index: chunkIndex,
            seq: event.seq,
            event_id: event.event_id,
            chunk: is_chunk,
            ts: event.ts,
          });
        }
      }
      if (parsed.type === "agent_thought") {
        if (!inThinking) {
          thinkingStartTs = event.ts;
          inThinking = true;
        }
      } else if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
      continue;
    }
    if (parsed.type === "tool_call") {
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
      const call: AcpToolCall = {
        id: String(parsed.id ?? parsed.call_id ?? ""),
        title: String(parsed.title ?? "Tool Call"),
        kind: parsed.kind ? String(parsed.kind) : undefined,
        status: parsed.status ? String(parsed.status) : "in_progress",
        content: parsed.content ? formatAcpContent(parsed.content) : undefined,
        raw_input: parsed.raw_input,
        session_id: event.session_id ?? null,
        seq: event.seq,
        event_id: event.event_id,
        ts: event.ts,
      };
      if (!call.id) continue;
      toolCallMap.set(call.id, call);
      toolCalls.push(call);
      continue;
    }
    if (parsed.type === "tool_call_update") {
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
      const id = String(parsed.id ?? parsed.call_id ?? "");
      if (!id) continue;
      let call = toolCallMap.get(id);
      if (!call) {
        call = { id, title: "Tool Call" };
        toolCallMap.set(id, call);
        toolCalls.push(call);
      }
      if (parsed.title) call.title = String(parsed.title);
      if (parsed.status) call.status = String(parsed.status);
      if (parsed.kind) call.kind = String(parsed.kind);
      if (parsed.raw_input) call.raw_input = parsed.raw_input;
      if (parsed.raw_output) call.raw_output = parsed.raw_output;
      if (parsed.content) {
        const nextContent = formatAcpContent(parsed.content);
        if (nextContent) {
          call.content = mergeToolCallContent(call.content, nextContent);
        }
      }
      if (call.session_id == null) call.session_id = event.session_id ?? null;
      if (call.ts == null) call.ts = event.ts;
      const updated = compareEventOrder(
        { event_id: event.event_id ?? null, ts: event.ts },
        { event_id: call.event_id ?? null, ts: call.ts }
      );
      if (updated > 0) {
        call.seq = event.seq;
        call.event_id = event.event_id;
        call.ts = event.ts;
      }
      if (parsed.meta?.terminal_output?.data) {
        call.terminal_output =
          (call.terminal_output ?? "") +
          String(parsed.meta.terminal_output.data);
      }
    }
    if (parsed.type === "plan" && parsed.plan) {
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
      plan = {
        ...(parsed.plan as AcpPlan),
        session_id: event.session_id ?? null,
        seq: event.seq,
        event_id: event.event_id,
        ts: event.ts,
      };
    }
    if (parsed.type === "available_commands") {
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
      const list = parsed.commands ?? parsed.available_commands ?? [];
      if (Array.isArray(list)) {
        commands = list as AcpCommand[];
      }
    }
    if (parsed.type === "current_mode") {
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
      if (parsed.current_mode_id) {
        currentMode = String(parsed.current_mode_id);
      }
    }
    if (parsed.type === "run_status") {
      runStatus = {
        status: String(parsed.status ?? ""),
        session_id: parsed.session_id ? String(parsed.session_id) : undefined,
      };
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
    }
  }

  const limitedRawEvents =
    rawEvents.length > 200 ? rawEvents.slice(rawEvents.length - 200) : rawEvents;
  return {
    hasAcp:
      toolCalls.length > 0 ||
      messages.length > 0 ||
      limitedRawEvents.length > 0 ||
      (plan?.entries?.length ?? 0) > 0 ||
      commands.length > 0,
    toolCalls,
    messages,
    rawEvents: limitedRawEvents,
    plan,
    commands,
    currentMode,
    runStatus,
    thinkingStartTs,
  };
}

function parseAcpEvent(line: string): Record<string, unknown> | null {
  const trimmed = line.trim();
  if (!trimmed.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(trimmed);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return null;
    }
    const payload = parsed as Record<string, unknown>;
    if (typeof payload.type === "string") return payload;
    return null;
  } catch {
    return null;
  }
}

function formatAcpContent(content: unknown): string {
  if (Array.isArray(content)) {
    return content
      .map((item) => formatAcpContent(item))
      .filter((item) => item.length > 0)
      .join("\n");
  }
  if (typeof content === "string") return content;
  if (content && typeof content === "object") {
    const obj = content as Record<string, unknown>;
    if (typeof obj.text === "string") return obj.text;
    if (obj.type === "content" && obj.content) {
      return formatAcpContent(obj.content);
    }
    if (obj.content) return formatAcpContent(obj.content);
    return JSON.stringify(content, null, 2);
  }
  return String(content ?? "");
}

function mergeToolCallContent(prev: string | undefined, next: string): string {
  if (!prev) return next;
  if (!next) return prev;
  if (next.startsWith(prev)) return next;
  if (prev.startsWith(next)) return prev;
  return next;
}

function parseChunkIndex(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function joinChunks(chunks: Map<number, string>): string {
  return [...chunks.entries()]
    .sort((left, right) => left[0] - right[0])
    .map((entry) => entry[1])
    .join("");
}
