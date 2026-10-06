import type { AcpView } from "./acp";

export type NativeRecoveryTarget = {
  runtime_id: string;
  session_id: string;
  recovery_id: string;
};

export type NativeRecoveryView = {
  local_session_id: string;
  runtime_id: string;
  session_id: string;
  recovery: {
    waiting_turn_id: string | null;
    blocked: {
      recovery_id: string;
      turn_id: string | null;
      reason: "process_lost" | "execution_interrupted" | "pending_cancelled" | "cleanup_incomplete";
    } | null;
    decisions: Array<{ state: "accepted" | "completed" | "uncertain" }>;
  };
};

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown> : null;
}

function validId(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9._:/-]{1,128}$/.test(value);
}

export function canReviewNativeRecovery(role?: string): boolean {
  return role === "root" || role === "admin" || role === "operator";
}

export function hasNativeRuntime(view: AcpView): boolean {
  return view.rawEvents.some(event =>
    record(record(record(event.payload)?.meta)?.provider_runtime)?.provider === "rara");
}

// Actions use a fresh owned query, never a token inferred from conversation history.
export function validateRecoveryView(value: unknown, localSessionId: string): asserts value is NativeRecoveryView {
  const view = record(value);
  const recovery = record(view?.recovery);
  const block = record(recovery?.blocked);
  if (!view || view.local_session_id !== localSessionId || !validId(view.runtime_id)
    || !validId(view.session_id) || !recovery
    || !(recovery.waiting_turn_id === null || validId(recovery.waiting_turn_id))
    || !(recovery.blocked === null || (block && typeof block.recovery_id === "string"
      && /^[A-Za-z0-9-]{1,128}$/.test(block.recovery_id)
      && (block.turn_id === null || validId(block.turn_id))
      && ["process_lost", "execution_interrupted", "pending_cancelled", "cleanup_incomplete"].includes(String(block.reason))
      && recovery.waiting_turn_id === null))
    || !Array.isArray(recovery.decisions) || recovery.decisions.length > 256
    || recovery.decisions.some(decision => !["accepted", "completed", "uncertain"].includes(String(record(decision)?.state)))) {
    throw new Error("Recovery state no longer matches this session. Refresh before continuing.");
  }
}

export function validRecoveryNote(note: string): boolean {
  return note.trim().length > 0 && new TextEncoder().encode(note).length <= 4096
    && !Array.from(note).some(character => {
      const code = character.codePointAt(0)!;
      return code <= 31 || (code >= 127 && code <= 159);
    });
}
