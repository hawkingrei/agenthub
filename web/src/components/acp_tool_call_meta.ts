import {
  ACP_TOOL_STATUS_CLASS,
  ACP_TOOL_STATUS_GROUP_DEFAULT_CLASS,
  ACP_TOOL_STATUS_GROUP_FAILURE_CLASS,
  ACP_TOOL_STATUS_GROUP_RUNNING_CLASS,
  ACP_TOOL_STATUS_GROUP_SUCCESS_CLASS,
} from "../ui/tailwind_classes";

export type ToolGroupStatusTone = "running" | "failure" | "success";

export type ToolCallDetailItem = {
  key: string;
  value: string;
};

const EDIT_OUTPUT_KEY = "unified_diff";
const DURATION_KEY_NORMALIZED = new Set<string>([
  "duration",
  "durationms",
  "elapsedms",
  "latencyms",
]);

export function resolveToolGroupStatusClassName(tone: ToolGroupStatusTone): string {
  let toneClass = ACP_TOOL_STATUS_GROUP_DEFAULT_CLASS;
  if (tone === "running") {
    toneClass = ACP_TOOL_STATUS_GROUP_RUNNING_CLASS;
  } else if (tone === "failure") {
    toneClass = ACP_TOOL_STATUS_GROUP_FAILURE_CLASS;
  } else if (tone === "success") {
    toneClass = ACP_TOOL_STATUS_GROUP_SUCCESS_CLASS;
  }
  return `${ACP_TOOL_STATUS_CLASS} ${toneClass}`;
}

export function formatToolCallDurationLabel(rawOutput: unknown): string | null {
  const topLevel = toTopLevelPayloadObject(rawOutput);
  if (!topLevel) return null;
  const durationMs =
    durationToMilliseconds(topLevel.duration) ??
    asFiniteNumber(topLevel.duration_ms) ??
    asFiniteNumber(topLevel.elapsed_ms) ??
    asFiniteNumber(topLevel.latency_ms);
  if (durationMs == null || durationMs < 0) return null;
  return formatDurationMilliseconds(durationMs);
}

export function selectToolCallOutputForDisplay(
  title: string,
  rawOutput: unknown
): unknown {
  if (!isEditToolTitle(title)) return rawOutput;
  const topLevel = toTopLevelPayloadObject(rawOutput);
  if (!topLevel) return rawOutput;
  if (!hasOwn(topLevel, EDIT_OUTPUT_KEY)) return null;
  return { [EDIT_OUTPUT_KEY]: topLevel[EDIT_OUTPUT_KEY] };
}

export function extractToolCallDetails(
  callId: string,
  rawOutput: unknown,
  title?: string
): ToolCallDetailItem[] {
  const details: ToolCallDetailItem[] = [{ key: "call_id", value: callId }];
  const topLevel = toTopLevelPayloadObject(rawOutput);
  if (!topLevel) return details;
  if (isEditToolTitle(title)) {
    for (const [key, value] of Object.entries(topLevel)) {
      const normalized = normalizePayloadKey(key);
      if (
        normalized === "callid" ||
        normalized === normalizePayloadKey(EDIT_OUTPUT_KEY) ||
        DURATION_KEY_NORMALIZED.has(normalized)
      ) {
        continue;
      }
      const detailValue = formatDetailValue(value);
      if (detailValue == null) continue;
      details.push({ key, value: detailValue });
    }
    return details;
  }

  appendScalarDetail(details, "cwd", topLevel.cwd);
  appendScalarDetail(details, "success", topLevel.success);
  return details;
}

function toTopLevelPayloadObject(
  value: unknown
): Record<string, unknown> | null {
  if (isPlainObject(value)) return value;
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!isJsonLikeText(trimmed)) return null;
  const parsed = parseJsonLikeString(trimmed);
  if (!isPlainObject(parsed)) return null;
  return parsed;
}

function durationToMilliseconds(value: unknown): number | null {
  if (value == null) return null;
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : null;
  }
  if (typeof value === "string") {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (!isPlainObject(value)) return null;

  const secs =
    asFiniteNumber(value.secs) ??
    asFiniteNumber(value.seconds) ??
    asFiniteNumber(value.sec) ??
    asFiniteNumber(value.s) ??
    0;
  const nanos =
    asFiniteNumber(value.nanos) ??
    asFiniteNumber(value.nanoseconds) ??
    asFiniteNumber(value.ns) ??
    0;
  const millis =
    asFiniteNumber(value.millis) ??
    asFiniteNumber(value.milliseconds) ??
    asFiniteNumber(value.ms) ??
    0;
  return secs * 1000 + nanos / 1_000_000 + millis;
}

function asFiniteNumber(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : null;
  }
  if (typeof value === "string") {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function appendScalarDetail(
  details: ToolCallDetailItem[],
  key: string,
  value: unknown
): void {
  const formatted = formatScalarDetailValue(value);
  if (formatted == null) return;
  details.push({ key, value: formatted });
}

function formatScalarDetailValue(value: unknown): string | null {
  if (typeof value === "boolean") return String(value);
  if (typeof value === "number") {
    return Number.isFinite(value) ? String(value) : null;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
  }
  return null;
}

function formatDetailValue(value: unknown): string | null {
  const scalar = formatScalarDetailValue(value);
  if (scalar != null) return scalar;
  if (value == null) return "null";
  try {
    const encoded = JSON.stringify(value);
    return encoded == null ? null : encoded;
  } catch {
    return String(value);
  }
}

function formatDurationMilliseconds(durationMs: number): string {
  if (durationMs < 1000) return `${Math.round(durationMs)}ms`;
  const seconds = durationMs / 1000;
  if (seconds < 10) return `${formatDecimal(seconds, 2)}s`;
  if (seconds < 60) return `${formatDecimal(seconds, 1)}s`;
  const minutes = Math.floor(seconds / 60);
  const remainSeconds = seconds - minutes * 60;
  if (remainSeconds < 10) {
    return `${minutes}m ${formatDecimal(remainSeconds, 1)}s`;
  }
  return `${minutes}m ${Math.round(remainSeconds)}s`;
}

function formatDecimal(value: number, digits: number): string {
  const fixed = value.toFixed(digits);
  return fixed
    .replace(/\.0+$/, "")
    .replace(/(\.\d*?)0+$/, "$1");
}

function isJsonLikeText(value: string): boolean {
  const trimmed = value.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

function parseJsonLikeString(value: string): unknown | undefined {
  const trimmed = value.trim();
  if (!isJsonLikeText(trimmed)) return undefined;
  try {
    return JSON.parse(trimmed);
  } catch {
    return undefined;
  }
}

function isEditToolTitle(title?: string): boolean {
  if (!title) return false;
  const normalized = title
    .trim()
    .toLowerCase()
    .replace(/[_-]+/g, " ");
  return normalized === "edit" || normalized.startsWith("edit ");
}

function hasOwn(value: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function normalizePayloadKey(key: string): string {
  return key.toLowerCase().replace(/[_-]+/g, "");
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value == null) return false;
  if (Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
