import type { AcpEventLine, AcpMessage } from "./acp";
import { compareEventOrder } from "./seq_order";

export type NativeInputTarget = {
  runtime_id: string;
  session_id: string;
  turn_id: string;
};

export type SubmitRequestUserInput = (
  input: string,
  target?: NativeInputTarget
) => Promise<void> | void;

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function validId(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9._:/-]{1,128}$/.test(value);
}

export function readNativeInputTarget(meta: unknown): NativeInputTarget | undefined {
  const target = record(record(meta)?.native_input);
  if (!target || !validId(target.runtime_id) || !validId(target.session_id) || !validId(target.turn_id)) {
    return undefined;
  }
  return { runtime_id: target.runtime_id, session_id: target.session_id, turn_id: target.turn_id };
}

export function hasNativeInputMetadata(meta: unknown): boolean {
  const value = record(meta);
  return value !== null && ("native_input" in value || record(value.provider_runtime)?.provider === "rara");
}

function requestKey(session: string | null | undefined, id: unknown, meta: unknown): string | null {
  const provider = record(record(meta)?.provider_runtime);
  if (!session || !validId(id) || !provider || provider.provider !== "rara"
    || provider.request_id !== id || !validId(provider.runtime_id) || !validId(provider.native_session_id)) {
    return null;
  }
  return JSON.stringify([session, provider.runtime_id, provider.native_session_id, id]);
}

const RECEIPT_STATUSES = new Set(["accepted", "queued", "rejected", "outcome_unknown", "not_sent"]);

// History pages and live events may arrive in either order. Receipts only annotate
// an existing attempt with the same local and native ownership identities.
export function applyNativeInputReceipts(messages: AcpMessage[], events: AcpEventLine[]): void {
  const receipts = new Map<string, { status: string; event: AcpEventLine }>();
  for (const event of events) {
    let value: Record<string, unknown> | null;
    try { value = record(JSON.parse(event.message)); } catch { continue; }
    if (!value || value.type !== "input_receipt") continue;
    const receipt = record(value.receipt);
    const provider = record(record(value.meta)?.provider_runtime);
    const key = requestKey(event.session_id, value.message_id, value.meta);
    if (!key || !receipt || receipt.request_id !== value.message_id
      || receipt.target_session_id !== provider?.native_session_id
      || !["prompt", "follow_up", "user_answer"].includes(String(receipt.kind))
      || typeof receipt.status !== "string" || !RECEIPT_STATUSES.has(receipt.status)) continue;
    const previous = receipts.get(key);
    if (!previous || compareEventOrder(event, previous.event) > 0) {
      receipts.set(key, { status: receipt.status, event });
    }
  }
  for (const message of messages) {
    if (message.kind !== "user_message") continue;
    const key = requestKey(message.session_id, message.message_id, message.meta);
    const receipt = key ? receipts.get(key) : undefined;
    if (receipt) message.delivery = receipt.status;
  }
}

export function inputDeliveryLabel(delivery?: string): string | undefined {
  switch (delivery) {
    case "async": return "Background update";
    case "pending": return "Sending";
    case "accepted": return "Accepted";
    case "queued": return "Queued";
    case "rejected": return "Rejected";
    case "not_sent": return "Not sent";
    case "outcome_unknown": return "Delivery unknown — check the output before retrying";
    default: return undefined;
  }
}
