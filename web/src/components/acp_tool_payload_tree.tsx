import React from "react";
import {
  formatConversationPreview,
  unescapeLineBreaks,
} from "../conversation";
import {
  SegmentedMoreFooter,
  useProgressiveVisibleCount,
} from "./acp_progressive_views";

const TOOL_PAYLOAD_MAX_NESTED_DEPTH = 2;
const TOOL_PAYLOAD_HIDDEN_KEY_NORMALIZED = new Set<string>([
  "turnid",
  "processid",
  "source",
  "callid",
  "cwd",
  "success",
  "duration",
  "durationms",
  "elapsedms",
  "latencyms",
]);
const TOOL_PAYLOAD_OUTPUT_PRIORITY_NORMALIZED = [
  "aggregatedoutput",
  "formattedoutput",
  "stdout",
] as const;
const TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED = new Set<string>(
  TOOL_PAYLOAD_OUTPUT_PRIORITY_NORMALIZED
);
const TOOL_PAYLOAD_HIDE_EMPTY_STREAM_KEY_NORMALIZED = new Set<string>([
  "stdout",
  "stderr",
]);
const TOOL_PAYLOAD_PLAIN_TEXT_KEY_NORMALIZED = new Set<string>([
  "parsedcmd",
  "context",
  "content",
]);
const TOOL_PAYLOAD_INITIAL_ITEMS = 8;
const TOOL_PAYLOAD_ITEM_CHUNK = 16;

export const ACP_PAYLOAD_GRID_CLASS = "acp-payload-grid m-0 grid gap-1.5";
export const ACP_PAYLOAD_ROW_CLASS =
  "acp-payload-row grid grid-cols-[minmax(84px,128px)_minmax(0,1fr)] items-start gap-1.5 rounded-md border border-slate-200/80 bg-white/88 px-2 py-1.5 [&>dt]:break-words [&>dt]:font-mono [&>dt]:text-[11px] [&>dt]:leading-[1.4] [&>dt]:text-slate-500 [&>dd]:m-0 [&>dd]:min-w-0 max-[720px]:grid-cols-[minmax(64px,96px)_minmax(0,1fr)] max-[720px]:gap-[5px] max-[600px]:grid-cols-1 max-[600px]:gap-1";
const ACP_PAYLOAD_SCALAR_CLASS =
  "acp-payload-scalar inline-block max-w-full break-words font-mono text-[11px] leading-[1.45] text-slate-800";
const ACP_PAYLOAD_SCALAR_MUTED_CLASS =
  "acp-payload-scalar muted inline-block max-w-full break-words font-mono text-[11px] leading-[1.45] text-slate-400";
const ACP_PAYLOAD_SEGMENTED_CLASS = "acp-payload-segmented grid gap-1.5";
const ACP_PAYLOAD_LIST_CLASS = "acp-payload-list m-0 grid list-none gap-1.5 pl-0";
const ACP_PAYLOAD_LIST_ITEM_CLASS = "m-0 min-w-0";
const ACP_PAYLOAD_NESTED_CLASS = "acp-payload-nested rounded-md border border-slate-200 bg-white";
const ACP_PAYLOAD_NESTED_SUMMARY_CLASS =
  "cursor-pointer font-mono text-[10.5px] leading-[1.35] text-slate-500";
const ACP_PAYLOAD_NESTED_BODY_CLASS =
  "acp-payload-nested-body mt-1 border-t border-slate-200 px-2 py-2";

export type StructuredPayloadRenderers = {
  renderText: (text: string) => React.ReactNode;
  renderPlainText: (text: string) => React.ReactNode;
};

export function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function summarizePayloadValue(value: unknown): string {
  const normalizedValue = normalizeNumericKeyedObject(value);
  if (normalizedValue !== value) {
    return summarizePayloadValue(normalizedValue);
  }
  if (value == null) return "null";
  if (typeof value === "string") return formatConversationPreview(unescapeLineBreaks(value), 120);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return "Array(0)";
    const sample = value
      .slice(0, 3)
      .map((item) => summarizeScalarValue(item))
      .filter((item) => item.length > 0)
      .join(", ");
    return sample ? `Array(${value.length}) · ${sample}` : `Array(${value.length})`;
  }
  if (isPlainObject(value)) {
    const entries = filterPayloadEntries(value);
    if (entries.length === 0) return "Object(0)";
    const preferredKeys = [
      "q",
      "query",
      "cmd",
      "command",
      "path",
      "url",
      "goal",
      "target",
      "tool",
      "fn",
    ];
    const picked = preferredKeys
      .filter((key) => entries.some(([entryKey]) => entryKey === key))
      .slice(0, 3)
      .map((key) => {
        const found = entries.find(([entryKey]) => entryKey === key);
        const resolved = found ? found[1] : undefined;
        return `${key}=${summarizeScalarValue(resolved)}`;
      })
      .filter((pair) => !pair.endsWith("="));
    if (picked.length > 0) return picked.join(" · ");
    const generic = entries
      .slice(0, 3)
      .map(([key, item]) => `${key}=${summarizeScalarValue(item)}`)
      .filter((pair) => !pair.endsWith("="));
    if (generic.length > 0) return generic.join(" · ");
    return `Object(${entries.length})`;
  }
  return String(value);
}

export function renderStructuredPayloadValue(
  value: unknown,
  depth: number,
  renderers: StructuredPayloadRenderers
): React.ReactNode {
  const normalizedValue = normalizeNumericKeyedObject(value);
  if (normalizedValue !== value) {
    return renderStructuredPayloadValue(normalizedValue, depth, renderers);
  }
  if (value == null) {
    return <span className={ACP_PAYLOAD_SCALAR_MUTED_CLASS}>null</span>;
  }
  if (typeof value === "string") {
    return renderers.renderText(unescapeLineBreaks(value));
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return <span className={ACP_PAYLOAD_SCALAR_CLASS}>{String(value)}</span>;
  }
  if (Array.isArray(value)) {
    return <PayloadArrayView value={value} depth={depth} renderers={renderers} />;
  }
  if (isPlainObject(value)) {
    return <PayloadObjectView value={value} depth={depth} renderers={renderers} />;
  }
  return <span className={ACP_PAYLOAD_SCALAR_CLASS}>{String(value)}</span>;
}

function summarizeScalarValue(value: unknown): string {
  const normalizedValue = normalizeNumericKeyedObject(value);
  if (normalizedValue !== value) {
    return summarizeScalarValue(normalizedValue);
  }
  if (value == null) return "null";
  if (typeof value === "string") return formatConversationPreview(unescapeLineBreaks(value), 48);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return "Array(0)";
    if (value.length <= 3) {
      const inline = value
        .map((item) => summarizePayloadValue(item))
        .filter((item) => item.length > 0)
        .join(", ");
      if (inline) return inline;
    }
    return `Array(${value.length})`;
  }
  if (isPlainObject(value)) return `Object(${Object.keys(value).length})`;
  return "";
}

function normalizePayloadKey(key: string): string {
  return key.trim().toLowerCase().replace(/[\s_-]+/g, "");
}

function isHiddenPayloadKey(key: string): boolean {
  return TOOL_PAYLOAD_HIDDEN_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function isPayloadValueEffectivelyEmpty(value: unknown): boolean {
  if (value == null) return true;
  if (typeof value === "string") {
    return unescapeLineBreaks(value).trim().length === 0;
  }
  if (Array.isArray(value)) return value.length === 0;
  if (isPlainObject(value)) return Object.keys(value).length === 0;
  return false;
}

function isEmptyStdStreamPayloadField(key: string, value: unknown): boolean {
  const normalized = normalizePayloadKey(key);
  if (!TOOL_PAYLOAD_HIDE_EMPTY_STREAM_KEY_NORMALIZED.has(normalized)) return false;
  return isPayloadValueEffectivelyEmpty(value);
}

function findPreferredOutputPayloadKey(entries: Array<[string, unknown]>): string | null {
  for (const normalizedKey of TOOL_PAYLOAD_OUTPUT_PRIORITY_NORMALIZED) {
    const match = entries.find(
      ([key, value]) =>
        normalizePayloadKey(key) === normalizedKey &&
        !isPayloadValueEffectivelyEmpty(value)
    );
    if (match) return match[0];
  }
  return null;
}

function filterPayloadEntries(value: Record<string, unknown>): Array<[string, unknown]> {
  const entries = Object.entries(value);
  const preferredOutputKey = findPreferredOutputPayloadKey(entries);
  return entries.filter(([key, item]) => {
    const normalized = normalizePayloadKey(key);
    if (
      TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED.has(normalized) &&
      key !== preferredOutputKey
    ) {
      return false;
    }
    return !isHiddenPayloadKey(key) && !isEmptyStdStreamPayloadField(key, item);
  });
}

function PayloadArrayView({
  value,
  depth,
  renderers,
}: {
  value: unknown[];
  depth: number;
  renderers: StructuredPayloadRenderers;
}) {
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    value.length,
    TOOL_PAYLOAD_INITIAL_ITEMS,
    TOOL_PAYLOAD_ITEM_CHUNK
  );
  if (value.length === 0) return <span className={ACP_PAYLOAD_SCALAR_MUTED_CLASS}>[]</span>;
  const allScalar = value.every((item) => !Array.isArray(item) && !isPlainObject(item));
  const visibleItems = value.slice(0, visibleCount);

  if (allScalar) {
    return (
      <div className={ACP_PAYLOAD_SEGMENTED_CLASS}>
        <span className={ACP_PAYLOAD_SCALAR_CLASS}>
          {visibleItems.map((item) => summarizeScalarValue(item)).join(", ")}
          {hasMore ? ` … (+${remaining} more)` : ""}
        </span>
        {hasMore && (
          <SegmentedMoreFooter remaining={remaining} unitLabel="items" onShowMore={showMore} />
        )}
      </div>
    );
  }

  return (
    <div className={ACP_PAYLOAD_SEGMENTED_CLASS}>
      <ul className={ACP_PAYLOAD_LIST_CLASS}>
        {visibleItems.map((item, index) => (
          <li className={ACP_PAYLOAD_LIST_ITEM_CLASS} key={index}>
            {renderNestedPayloadValue(item, depth + 1, renderers)}
          </li>
        ))}
      </ul>
      {hasMore && (
        <SegmentedMoreFooter remaining={remaining} unitLabel="items" onShowMore={showMore} />
      )}
    </div>
  );
}

function PayloadObjectView({
  value,
  depth,
  renderers,
}: {
  value: Record<string, unknown>;
  depth: number;
  renderers: StructuredPayloadRenderers;
}) {
  const entries = filterPayloadEntries(value);
  const { visibleCount, hasMore, remaining, showMore } = useProgressiveVisibleCount(
    entries.length,
    TOOL_PAYLOAD_INITIAL_ITEMS,
    TOOL_PAYLOAD_ITEM_CHUNK
  );
  if (entries.length === 0) return <span className={ACP_PAYLOAD_SCALAR_MUTED_CLASS}>{"{}"}</span>;
  const visibleEntries = entries.slice(0, visibleCount);
  return (
    <div className={ACP_PAYLOAD_SEGMENTED_CLASS}>
      <dl className={ACP_PAYLOAD_GRID_CLASS}>
        {visibleEntries.map(([key, item]) => (
          <div className={ACP_PAYLOAD_ROW_CLASS} key={key}>
            <dt>{key}</dt>
            <dd className="font-medium text-notion-text text-sm opacity-90">
              {renderPayloadFieldValue(key, item, depth + 1, renderers)}
            </dd>
          </div>
        ))}
      </dl>
      {hasMore && (
        <SegmentedMoreFooter remaining={remaining} unitLabel="fields" onShowMore={showMore} />
      )}
    </div>
  );
}

function renderPayloadFieldValue(
  key: string,
  value: unknown,
  depth: number,
  renderers: StructuredPayloadRenderers
): React.ReactNode {
  if (typeof value === "string" && shouldPreferPlainTextForPayloadKey(key)) {
    return renderers.renderPlainText(unescapeLineBreaks(value));
  }
  if (isPrimaryOutputPayloadField(key) && typeof value === "string") {
    return renderers.renderPlainText(unescapeLineBreaks(value));
  }
  return renderNestedPayloadValue(value, depth, renderers);
}

function isPrimaryOutputPayloadField(key: string): boolean {
  return TOOL_PAYLOAD_OUTPUT_PRIORITY_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function renderNestedPayloadValue(
  value: unknown,
  depth: number,
  renderers: StructuredPayloadRenderers
): React.ReactNode {
  const isStructured = Array.isArray(value) || isPlainObject(value);
  if (isStructured && depth > TOOL_PAYLOAD_MAX_NESTED_DEPTH) {
    return <span className={ACP_PAYLOAD_SCALAR_CLASS}>{summarizePayloadValue(value)}</span>;
  }
  if (isStructured && shouldInlineStructuredPayload(value, depth)) {
    return <div className="acp-payload-inline">{renderStructuredPayloadValue(value, depth, renderers)}</div>;
  }
  if (isStructured) {
    return (
      <details className={ACP_PAYLOAD_NESTED_CLASS}>
        <summary
          className={`${ACP_PAYLOAD_NESTED_SUMMARY_CLASS} flex cursor-pointer list-none items-center gap-2 px-2 py-1.5`}
        >
          <i className="bi bi-chevron-right text-[10px]" />
          <span>{summarizePayloadValue(value)}</span>
        </summary>
        <div className={ACP_PAYLOAD_NESTED_BODY_CLASS}>
          {renderStructuredPayloadValue(value, depth, renderers)}
        </div>
      </details>
    );
  }
  return renderStructuredPayloadValue(value, depth, renderers);
}

function shouldInlineStructuredPayload(value: unknown, depth: number): boolean {
  if (depth > 2) return false;
  if (Array.isArray(value)) {
    return value.length > 0 && value.length <= 10;
  }
  if (isPlainObject(value)) {
    const size = Object.keys(value).length;
    return size > 0 && size <= 8;
  }
  return false;
}

function shouldPreferPlainTextForPayloadKey(key: string): boolean {
  return TOOL_PAYLOAD_PLAIN_TEXT_KEY_NORMALIZED.has(normalizePayloadKey(key));
}

function normalizeNumericKeyedObject(value: unknown): unknown {
  if (!isPlainObject(value)) return value;
  const keys = Object.keys(value);
  if (keys.length === 0) return value;
  if (!keys.every((key) => /^\d+$/.test(key))) return value;
  const sorted = keys.map((key) => Number(key)).sort((a, b) => a - b);
  const start = sorted[0];
  if (start !== 0 && start !== 1) return value;
  const end = sorted[sorted.length - 1];
  if (end - start + 1 !== sorted.length) return value;
  const normalized: unknown[] = [];
  for (let index = start; index <= end; index += 1) {
    normalized.push(value[String(index)]);
  }
  return normalized;
}
