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
  terminal_activities?: AcpTerminalActivity[];
  media?: AcpMedia[];
  meta?: unknown;
  session_id?: string | null;
  seq?: string;
  event_id?: number;
  ts?: number;
};

export type AcpMedia = {
  type: "image";
  data?: string;
  mime_type: string;
  uri?: string;
  name?: string;
};

export type AcpTerminalActivity = {
  kind: "waiting" | "waited" | "interacted";
  command?: string;
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
  media?: AcpMedia[];
  delivery?: string;
  meta?: unknown;
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

export type AcpConfigSelectOption = {
  valueId: string;
  label: string;
};

export type AcpConfigOption = {
  id: string;
  label: string;
  category?: string | null;
  currentValueId?: string | null;
  selectOptions: AcpConfigSelectOption[];
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
  configOptions: AcpConfigOption[];
  plan: AcpPlanView | null;
  commands: AcpCommand[];
  currentMode: string | null;
  runStatus: AcpRunStatus | null;
  thinkingStartTs: number | null;
};

export const EMPTY_ACP_VIEW: AcpView = {
  hasAcp: false,
  toolCalls: [],
  messages: [],
  rawEvents: [],
  configOptions: [],
  plan: null,
  commands: [],
  currentMode: null,
  runStatus: null,
  thinkingStartTs: null,
};

export function buildAcpView(events: AcpEventLine[]): AcpView {
  const toolCalls: AcpToolCall[] = [];
  const toolCallMap = new Map<string, AcpToolCall>();
  const messages: AcpMessage[] = [];
  const messageIndex = new Map<string, number>();
  const messageChunks = new Map<string, Map<number, string>>();
  const rawEvents: AcpRawEvent[] = [];
  let configOptions: AcpConfigOption[] = [];
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
    if (parsed.type === "config_option_update") {
      const parsedConfigOptions = parseAcpConfigOptions(parsed);
      if (parsedConfigOptions !== null) {
        configOptions = parsedConfigOptions;
      }
      if (parsed.current_mode_id) {
        currentMode = String(parsed.current_mode_id);
      }
    }
    if (
      parsed.type === "agent_message" ||
      parsed.type === "agent_thought" ||
      parsed.type === "user_message"
    ) {
      const text = String(parsed.text ?? "");
      const last = messages[messages.length - 1];
      const is_chunk = parsed.chunk === true;
      const messageMedia = extractAcpMedia(parsed.attachments);
      const messageMeta = parsed.meta;
      const delivery = parseMessageDelivery(messageMeta);
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
          existing.media = mergeAcpMedia(existing.media, messageMedia);
          if (messageMeta !== undefined) existing.meta = messageMeta;
          if (delivery) existing.delivery = delivery;
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
            media: messageMedia,
            delivery,
            meta: messageMeta,
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
          last.media = mergeAcpMedia(last.media, messageMedia);
          if (messageMeta !== undefined) last.meta = messageMeta;
          if (delivery) last.delivery = delivery;
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
            media: messageMedia,
            delivery,
            meta: messageMeta,
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
        media: extractAcpMedia(parsed.content),
        meta: parsed.meta,
        session_id: event.session_id ?? null,
        seq: event.seq,
        event_id: event.event_id,
        ts: event.ts,
      };
      if (!call.id) continue;
      const existing = toolCallMap.get(call.id);
      if (existing) {
        existing.title = call.title;
        existing.kind = call.kind;
        existing.status = call.status;
        existing.content = mergeToolCallContent(existing.content, call.content ?? "");
        existing.raw_input = call.raw_input ?? existing.raw_input;
        existing.media = mergeAcpMedia(existing.media, call.media);
        existing.meta = call.meta ?? existing.meta;
        existing.seq = call.seq;
        existing.event_id = call.event_id;
        existing.ts = call.ts;
      } else {
        toolCallMap.set(call.id, call);
        toolCalls.push(call);
      }
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
      if (parsed.meta !== undefined) call.meta = parsed.meta;
      call.media = mergeAcpMedia(call.media, extractAcpMedia(parsed.content));
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
      const parsedMeta =
        parsed.meta && typeof parsed.meta === "object"
          ? (parsed.meta as Record<string, unknown>)
          : null;
      const terminalOutput =
        parsedMeta?.terminal_output && typeof parsedMeta.terminal_output === "object"
          ? (parsedMeta.terminal_output as Record<string, unknown>)
          : null;
      if (terminalOutput?.data) {
        call.terminal_output =
          (call.terminal_output ?? "") +
          String(terminalOutput.data);
      }
      const terminalActivity = parseAcpTerminalActivity(parsedMeta?.terminal_activity);
      if (terminalActivity) {
        const prev = call.terminal_activities ?? [];
        const last = prev[prev.length - 1];
        if (
          !last ||
          last.kind !== terminalActivity.kind ||
          last.command !== terminalActivity.command
        ) {
          call.terminal_activities = [...prev, terminalActivity];
        }
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
        session_id: parsed.session_id
          ? String(parsed.session_id)
          : event.session_id ?? undefined,
      };
      if (inThinking) {
        inThinking = false;
        thinkingStartTs = null;
      }
    }
  }

  const limitedRawEvents =
    rawEvents.length > 200 ? rawEvents.slice(rawEvents.length - 200) : rawEvents;
  const resolvedCurrentMode =
    currentMode ??
    configOptions.find((option) => option.id === "mode")?.currentValueId ??
    null;
  closeStaleLiveToolCalls(toolCalls, messages, runStatus);
  return {
    hasAcp:
      toolCalls.length > 0 ||
      messages.length > 0 ||
      limitedRawEvents.length > 0 ||
      (plan?.entries?.length ?? 0) > 0 ||
      commands.length > 0 ||
      configOptions.length > 0,
    toolCalls,
    messages,
    rawEvents: limitedRawEvents,
    configOptions,
    plan,
    commands,
    currentMode: resolvedCurrentMode,
    runStatus,
    thinkingStartTs,
  };
}

const LIVE_TOOL_CALL_STATUSES = new Set(["pending", "in_progress", "running"]);
const TERMINAL_RUN_STATUSES = new Set([
  "completed",
  "failed",
  "cancelled",
  "canceled",
  "interrupted",
  "stopped",
]);

function closeStaleLiveToolCalls(
  toolCalls: AcpToolCall[],
  messages: AcpMessage[],
  runStatus: AcpRunStatus | null
): void {
  const normalizedRunStatus = normalizeAcpStatus(runStatus?.status);
  const terminalRunStatus = TERMINAL_RUN_STATUSES.has(normalizedRunStatus)
    ? normalizedRunStatus
    : null;
  const latestMessageOrderBySession = buildLatestMessageOrderBySession(messages);
  for (const call of toolCalls) {
    if (!LIVE_TOOL_CALL_STATUSES.has(normalizeAcpStatus(call.status))) {
      continue;
    }
    if (terminalRunStatus && isRunStatusForToolCall(runStatus, call)) {
      call.status = terminalRunStatus;
      continue;
    }
    if (readAgentHubMetaKind(call.meta) === "codex_subagent") {
      continue;
    }
    const latestMessageOrder = latestMessageOrderBySession.get(
      call.session_id ?? null
    );
    if (
      latestMessageOrder &&
      compareEventOrder(latestMessageOrder, {
        event_id: call.event_id ?? null,
        ts: call.ts,
      }) > 0
    ) {
      call.status = "completed";
    }
  }
}

function buildLatestMessageOrderBySession(
  messages: AcpMessage[]
): Map<string | null, { event_id: number | null; ts?: number }> {
  const latest = new Map<string | null, { event_id: number | null; ts?: number }>();
  for (const message of messages) {
    const sessionId = message.session_id ?? null;
    const order = { event_id: message.event_id ?? null, ts: message.ts };
    const existing = latest.get(sessionId);
    if (!existing || compareEventOrder(order, existing) > 0) {
      latest.set(sessionId, order);
    }
  }
  return latest;
}

function isRunStatusForToolCall(
  runStatus: AcpRunStatus | null,
  call: AcpToolCall
): boolean {
  if (!runStatus?.session_id) return true;
  return runStatus.session_id === (call.session_id ?? null);
}

function normalizeAcpStatus(status?: string | null): string {
  if (!status) return "";
  return status.trim().toLowerCase().replace(/[\s-]+/g, "_");
}

function parseAcpTerminalActivity(value: unknown): AcpTerminalActivity | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const record = value as Record<string, unknown>;
  const kind = typeof record.kind === "string" ? record.kind : "";
  if (kind !== "waiting" && kind !== "waited" && kind !== "interacted") {
    return null;
  }
  return {
    kind,
    command: typeof record.command === "string" ? record.command : undefined,
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

function parseAcpConfigOptions(value: unknown): AcpConfigOption[] | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const record = value as Record<string, unknown>;
  if (!Object.prototype.hasOwnProperty.call(record, "config_options")) {
    return null;
  }
  const rawOptions = record.config_options;
  if (!Array.isArray(rawOptions)) {
    return null;
  }
  return rawOptions
    .map((option) => parseAcpConfigOption(option))
    .filter((option): option is AcpConfigOption => option !== null);
}

function parseAcpConfigOption(value: unknown): AcpConfigOption | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const record = value as Record<string, unknown>;
  const id = typeof record.id === "string" ? record.id.trim() : "";
  if (!id) {
    return null;
  }
  const label =
    (typeof record.label === "string" && record.label.trim()) ||
    (typeof record.name === "string" && record.name.trim()) ||
    id;
  const selectOptions = Array.isArray(record.select_options)
    ? record.select_options
        .map((option) => parseAcpConfigSelectOption(option))
        .filter((option): option is AcpConfigSelectOption => option !== null)
    : [];
  const currentValueId = parseAcpConfigValueId(record.current_value);
  if (
    currentValueId &&
    !selectOptions.some((option) => option.valueId === currentValueId)
  ) {
    selectOptions.unshift({ valueId: currentValueId, label: currentValueId });
  }
  return {
    id,
    label,
    category: typeof record.category === "string" ? record.category : null,
    currentValueId,
    selectOptions,
  };
}

function parseAcpConfigSelectOption(value: unknown): AcpConfigSelectOption | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const record = value as Record<string, unknown>;
  const valueId = parseAcpConfigValueId(record.value_id ?? record.value);
  if (!valueId) {
    return null;
  }
  const label =
    (typeof record.label === "string" && record.label.trim()) ||
    (typeof record.name === "string" && record.name.trim()) ||
    valueId;
  return { valueId, label };
}

function parseAcpConfigValueId(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const record = value as Record<string, unknown>;
  if (typeof record.value_id === "string") {
    const trimmed = record.value_id.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }
  if (record.value_id && typeof record.value_id === "object") {
    const nested = record.value_id as Record<string, unknown>;
    if (typeof nested.value === "string") {
      const trimmed = nested.value.trim();
      if (trimmed.length > 0) {
        return trimmed;
      }
    }
  }
  if (record.type === "value_id" && typeof record.value === "string") {
    const trimmed = record.value.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }
  if (typeof record.value === "string") {
    const trimmed = record.value.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }
  return null;
}

function parseMessageDelivery(meta: unknown): string | undefined {
  if (!meta || typeof meta !== "object") return undefined;
  const delivery = (meta as Record<string, unknown>).delivery;
  return typeof delivery === "string" && delivery.trim() ? delivery.trim() : undefined;
}

export function readAgentHubMetaKind(meta: unknown): string | null {
  if (!meta || typeof meta !== "object") return null;
  const agenthub = (meta as Record<string, unknown>).agenthub;
  if (!agenthub || typeof agenthub !== "object") return null;
  const kind = (agenthub as Record<string, unknown>).kind;
  return typeof kind === "string" ? kind : null;
}

function isSameAcpMedia(left: AcpMedia, right: AcpMedia): boolean {
  return (
    left.mime_type === right.mime_type &&
    left.uri === right.uri &&
    left.data === right.data
  );
}

function mergeAcpMedia(
  previous: AcpMedia[] | undefined,
  next: AcpMedia[] | undefined
): AcpMedia[] | undefined {
  if (!previous?.length) return next?.length ? next : undefined;
  if (!next?.length) return previous;
  const merged = [...previous];
  for (const media of next) {
    if (merged.some((candidate) => isSameAcpMedia(candidate, media))) continue;
    merged.push(media);
  }
  return merged;
}

export function extractAcpMedia(content: unknown): AcpMedia[] | undefined {
  const media: AcpMedia[] = [];
  const visit = (value: unknown) => {
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    if (!value || typeof value !== "object") return;
    const obj = value as Record<string, unknown>;
    if (obj.type === "image") {
      const data = typeof obj.data === "string" ? obj.data : undefined;
      const uri = typeof obj.uri === "string" ? obj.uri : undefined;
      const mimeTypeValue = obj.mimeType ?? obj.mime_type;
      const mimeType =
        typeof mimeTypeValue === "string" ? mimeTypeValue.toLowerCase() : "image/png";
      if (!mimeType.startsWith("image/") || (!data && !uri)) return;
      const nameValue = obj.name ?? obj.file_name;
      const candidate: AcpMedia = {
        type: "image",
        data,
        uri,
        mime_type: mimeType,
        name: typeof nameValue === "string" ? nameValue : undefined,
      };
      if (!media.some((item) => isSameAcpMedia(item, candidate))) {
        media.push(candidate);
      }
      return;
    }
    if (obj.content !== undefined) visit(obj.content);
    if (obj.attachments !== undefined) visit(obj.attachments);
  };
  visit(content);
  return media.length > 0 ? media : undefined;
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
    if (obj.type === "image") return "";
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
  // Prefer the longer fragment to avoid dropping partial tool output.
  // On ties, prefer the newer update.
  return prev.length > next.length ? prev : next;
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
