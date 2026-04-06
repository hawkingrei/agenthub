import React from "react";
import { formatConversationPreview } from "../conversation";
import type { ToolCallDetailItem } from "./acp_tool_call_meta";
import {
  ACP_PAYLOAD_GRID_CLASS,
  ACP_PAYLOAD_ROW_CLASS,
  renderStructuredPayloadValue,
  summarizePayloadValue,
} from "./acp_tool_payload_tree";
import {
  createPayloadTextRenderers,
  ToolTextContent,
} from "./acp_tool_text_views";

export const TOOL_PAYLOAD_PREVIEW_LIMIT = 64;

const ACP_PAYLOAD_CARD_CLASS =
  "acp-payload-card overflow-hidden rounded-[10px] border border-notion-payload-border bg-notion-payload-bg px-[7px] py-1.5 shadow-[0_1px_0_rgba(15,23,42,0.03)] max-[720px]:rounded-lg max-[720px]:px-1.5 max-[720px]:py-[5px]";

export type NormalizedToolPayload =
  | { kind: "empty" }
  | { kind: "text"; text: string }
  | { kind: "json_text"; text: string; parsed?: unknown }
  | { kind: "json"; value: unknown };

export { isPlainObject } from "./acp_tool_payload_tree";
export {
  ACP_SEGMENTED_BLOCK_CLASS,
  shouldAutoExpandToolContent,
  TOOL_TEXT_INITIAL_LINES,
  TOOL_TEXT_LINE_CHUNK,
  ToolTextContent,
} from "./acp_tool_text_views";

export function summarizeToolPayload(payload: NormalizedToolPayload, limit: number): string {
  if (payload.kind === "empty") return "";
  if (payload.kind === "text") return formatConversationPreview(payload.text, limit);
  if (payload.kind === "json_text") {
    if (payload.parsed !== undefined) {
      return formatConversationPreview(summarizePayloadValue(payload.parsed), limit);
    }
    return formatConversationPreview(payload.text, limit);
  }
  return formatConversationPreview(summarizePayloadValue(payload.value), limit);
}

export const ToolPayloadView = React.memo(function ToolPayloadView({
  payload,
}: {
  payload: NormalizedToolPayload;
}) {
  if (payload.kind === "empty") return null;
  if (payload.kind === "text") {
    return <ToolTextContent text={payload.text} />;
  }
  if (payload.kind === "json_text") {
    if (payload.parsed === undefined) {
      return <ToolTextContent text={payload.text} />;
    }
    return (
      <div className={ACP_PAYLOAD_CARD_CLASS}>
        {renderStructuredPayloadValue(payload.parsed, 0, payloadTextRenderers)}
      </div>
    );
  }
  return (
    <div className={ACP_PAYLOAD_CARD_CLASS}>
      {renderStructuredPayloadValue(payload.value, 0, payloadTextRenderers)}
    </div>
  );
});

export const ToolCallDetailsView = React.memo(function ToolCallDetailsView({
  details,
}: {
  details: ToolCallDetailItem[];
}) {
  return (
    <div className={ACP_PAYLOAD_CARD_CLASS}>
      <dl className={ACP_PAYLOAD_GRID_CLASS}>
        {details.map((detail) => (
          <div className={ACP_PAYLOAD_ROW_CLASS} key={detail.key}>
            <dt>{detail.key}</dt>
            <dd className="font-medium text-sm text-notion-text opacity-90">
              <code>{detail.value}</code>
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
});

export function isJsonLikeText(value: string): boolean {
  const trimmed = value.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

export function parseJsonLikeString(value: string, countParse = false): unknown | undefined {
  const trimmed = value.trim();
  if (!isJsonLikeText(trimmed)) return undefined;
  if (countParse) {
    incrementPayloadParseCount();
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    if (countParse) {
      incrementPayloadParseFailureCount();
    }
    return undefined;
  }
}

let payloadParseCount = 0;
let payloadParseFailureCount = 0;

export function getToolPayloadParseStats(): {
  payloadParses: number;
  payloadParseFailures: number;
} {
  return {
    payloadParses: payloadParseCount,
    payloadParseFailures: payloadParseFailureCount,
  };
}

export function resetToolPayloadParseStats(): void {
  payloadParseCount = 0;
  payloadParseFailureCount = 0;
}

function incrementPayloadParseCount(): void {
  payloadParseCount += 1;
}

function incrementPayloadParseFailureCount(): void {
  payloadParseFailureCount += 1;
}

const payloadTextRenderers = createPayloadTextRenderers();
