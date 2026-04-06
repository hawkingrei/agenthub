import { unescapeLineBreaks } from "../conversation";

export {
  getToolContentCacheStats,
  parseAnsiSegmentsCached,
  resetToolContentCaches,
  TerminalOutputView,
  type ToolContentCacheStats,
} from "./acp_terminal_output";

export {
  TOOL_PAYLOAD_PREVIEW_LIMIT,
  ToolCallDetailsView,
  ToolPayloadView,
  ToolTextContent,
  isJsonLikeText,
  isPlainObject,
  parseJsonLikeString,
  summarizeToolPayload,
  shouldAutoExpandToolContent,
  type NormalizedToolPayload,
} from "./acp_tool_payload_content";

import {
  isJsonLikeText,
  isPlainObject,
  parseJsonLikeString,
  type NormalizedToolPayload,
} from "./acp_tool_payload_content";

export function normalizeToolPayload(value: unknown): NormalizedToolPayload {
  if (value == null) return { kind: "empty" };
  if (typeof value === "string") {
    const text = unescapeLineBreaks(value).trim();
    if (!text) return { kind: "empty" };
    if (isJsonLikeText(text)) {
      return {
        kind: "json_text",
        text,
        parsed: parseJsonLikeString(text, true),
      };
    }
    return { kind: "text", text };
  }
  if (isPlainObject(value) || Array.isArray(value)) {
    return { kind: "json", value };
  }
  return {
    kind: "text",
    text: String(value),
  };
}

export function hasToolPayload(payload: NormalizedToolPayload): boolean {
  return payload.kind !== "empty";
}
