import type { AcpTerminalActivity } from "../acp";
import type {
  ExploreGroupConversationItem,
  ToolCallConversationItem,
  ToolCallGroupConversationItem,
} from "../conversation";
import { formatConversationPreview, unescapeLineBreaks } from "../conversation";
import type { ToolGroupStatusTone } from "./acp_tool_call_meta";

export const ACP_TOOL_ROW_CLASS =
  "acp-row group relative mb-0.5 flex w-full flex-col items-start rounded-lg border border-transparent px-1.5 py-1.5 transition hover:border-black/10 hover:bg-white/80";
export const ACP_TOOL_CARD_CLASS =
  "self-start w-full max-w-full overflow-hidden rounded-xl border border-black/6 bg-white/94 shadow-notion-row";
export const ACP_TOOL_CARD_NESTED_CLASS = "max-w-full bg-slate-50/48 shadow-none";
export const ACP_TOOL_SUMMARY_CLASS =
  "flex cursor-pointer list-none items-start gap-2 px-2 py-1.5 [&::-webkit-details-marker]:hidden";
export const ACP_TOOL_TITLE_CLASS =
  "min-w-0 flex-1 text-[12px] font-semibold leading-5 text-slate-900";
export const ACP_TOOL_GROUP_LIST_CLASS = "flex flex-col gap-1 px-2 pb-2";

const FAILED_TOOL_STATUSES = new Set([
  "failed",
  "cancelled",
  "canceled",
  "interrupted",
  "stopped",
]);

export type ToolCallBubbleProps = {
  msg: ToolCallConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  grouped?: boolean;
  indexLabel?: string;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export type ToolCallGroupBubbleProps = {
  msg: ToolCallGroupConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export type ExploreGroupBubbleProps = {
  msg: ExploreGroupConversationItem;
  ansi: (input: string) => string;
  runStatus?: string | null;
  autoCollapse?: boolean;
  onSubmitRequestUserInput?: (input: string) => Promise<void> | void;
};

export function formatTerminalActivityLabel(activity: AcpTerminalActivity): string {
  const base =
    activity.kind === "waiting"
      ? "Waiting for background terminal"
      : activity.kind === "waited"
        ? "Waited for background terminal"
        : "Interacted with background terminal";
  if (!activity.command?.trim()) {
    return base;
  }
  return `${base} · ${activity.command.trim()}`;
}

export function deriveToolCallHint(
  title: string,
  rawInput: unknown,
  content?: string
): string {
  const normalized = title.trim().toLowerCase();
  if (!rawInput || typeof rawInput !== "object") {
    if (normalized.includes("explore") && content) {
      return formatConversationPreview(unescapeLineBreaks(content), 60);
    }
    return "";
  }
  const value = rawInput as Record<string, unknown>;
  if (normalized.includes("search")) {
    const query =
      typeof value.q === "string"
        ? value.q
        : typeof value.query === "string"
          ? value.query
          : typeof value.keyword === "string"
            ? value.keyword
            : null;
    if (query) return formatConversationPreview(query, 60);
  }
  if (normalized.includes("explore")) {
    const goal =
      typeof value.goal === "string"
        ? value.goal
        : typeof value.target === "string"
          ? value.target
          : typeof value.topic === "string"
            ? value.topic
            : null;
    if (goal) return formatConversationPreview(goal, 60);
  }
  return "";
}

export function summarizeToolGroupTitles(calls: ToolCallConversationItem[]): string {
  if (calls.length === 0) return "";
  const previews = calls
    .slice(0, 2)
    .map((call) => call.title.trim())
    .filter((title) => title.length > 0);
  if (previews.length === 0) return "";
  if (calls.length <= 2) return previews.join(" · ");
  return `${previews.join(" · ")} +${calls.length - 2} more`;
}

export function deriveToolGroupStatusSummary(
  calls: ToolCallConversationItem[],
  runStatus: string | null | undefined,
  isToolCallEffectivelyLive: (status?: string, runStatus?: string | null) => boolean
): { label: string; tone: ToolGroupStatusTone } | null {
  if (calls.length === 0) return null;
  let liveCount = 0;
  let failedCount = 0;
  for (const call of calls) {
    if (isToolCallEffectivelyLive(call.status, runStatus)) {
      liveCount += 1;
      continue;
    }
    const normalized = normalizeToolCallStatus(call.status);
    if (FAILED_TOOL_STATUSES.has(normalized)) {
      failedCount += 1;
    }
  }
  if (liveCount > 0) return { label: `${liveCount} running`, tone: "running" };
  if (failedCount > 0) return { label: `${failedCount} failed`, tone: "failure" };
  return { label: `${calls.length} completed`, tone: "success" };
}

export function normalizeToolCallStatus(status?: string): string {
  if (!status) return "";
  return status.trim().toLowerCase().replace(/[\s-]+/g, "_");
}

export function formatToolCallStatus(status?: string): string {
  if (!status) return "";
  const normalized = normalizeToolCallStatus(status);
  switch (normalized) {
    case "in_progress":
      return "In Progress";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "pending":
      return "Pending";
    case "running":
      return "Running";
    case "cancelled":
    case "canceled":
      return "Cancelled";
    case "interrupted":
      return "Interrupted";
    case "stopped":
      return "Stopped";
    default:
      return status;
  }
}

export function getToolCallStatusMark(
  status?: string
): { tone: "success" | "failure"; label: string } | null {
  if (!status) return null;
  const normalized = normalizeToolCallStatus(status);
  if (normalized === "completed") {
    return { tone: "success", label: "Completed" };
  }
  if (FAILED_TOOL_STATUSES.has(normalized)) {
    return { tone: "failure", label: "Failed" };
  }
  return null;
}
