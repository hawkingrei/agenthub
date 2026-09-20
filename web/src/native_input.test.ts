import { buildConversationMessages } from "./conversation";
import { describe, expect, it } from "vitest";
import { buildAcpView, type AcpEventLine } from "./acp";
import { hasNativeInputMetadata, readNativeInputTarget } from "./native_input";

const provider = { provider: "rara", runtime_id: "runtime", native_session_id: "native", request_id: "input" };
const event = (event_id: number, value: unknown, session_id = "local"): AcpEventLine => ({
  stream: "acp", event_id, ts: event_id, session_id, message: JSON.stringify(value),
});
const attempt = (id = "input", session = "local") => event(1, {
  type: "user_message", message_id: id, text: "same text", chunk: false,
  meta: { delivery: "pending", provider_runtime: { ...provider, request_id: id } },
}, session);
const receipt = (status = "accepted", metadata = provider, session = "local", kind = "prompt") => event(3, {
  type: "input_receipt", message_id: "input",
  meta: { provider_runtime: metadata },
  receipt: { request_id: "input", kind, target_session_id: "native", status },
}, session);

describe("native input history", () => {
  it("attaches an out-of-order receipt without creating or duplicating a message", () => {
    const view = buildAcpView([receipt(), attempt(), attempt()]);
    expect(view.messages).toHaveLength(1);
    expect(view.messages[0].delivery).toBe("accepted");
    expect(buildConversationMessages(view.messages, view.toolCalls, view.plan, "local")[0]).toMatchObject({ kind: "user_message", delivery: "accepted" });
    expect(buildAcpView([receipt()]).messages).toEqual([]);
  });

  it("retains identical text sent under distinct request identities", () => {
    const view = buildAcpView([attempt(), attempt("another"), receipt()]);
    expect(view.messages.map(message => message.delivery)).toEqual(["accepted", "pending"]);
  });

  it("rejects receipts from another ownership scope or control kind", () => {
    for (const invalid of [
      receipt("accepted", provider, "other-local"),
      receipt("accepted", { ...provider, runtime_id: "other-runtime" }),
      receipt("accepted", { ...provider, native_session_id: "other-native" }),
      receipt("accepted", { ...provider, request_id: "another" }),
      receipt("accepted", provider, "local", "shell_answer"),
      receipt("made_up"),
    ]) {
      expect(buildAcpView([attempt(), invalid]).messages[0].delivery).toBe("pending");
    }
  });

  it.each(["accepted", "queued", "rejected", "outcome_unknown", "not_sent"])("preserves %s delivery", status => {
    const older = { ...receipt("outcome_unknown"), event_id: 2 };
    expect(buildAcpView([receipt(status), attempt(), older]).messages[0].delivery).toBe(status);
  });
});

describe("native question targets", () => {
  it("retains the original runtime, session, and turn", () => {
    const target = { runtime_id: "runtime", session_id: "native", turn_id: "waiting" };
    expect(readNativeInputTarget({ native_input: target })).toEqual(target);
  });

  it("recognizes malformed native cards without allowing ordinary-text fallback", () => {
    for (const meta of [
      { provider_runtime: provider },
      { native_input: null },
      { native_input: { runtime_id: "runtime", session_id: "native", turn_id: " " } },
    ]) {
      expect(hasNativeInputMetadata(meta)).toBe(true);
      expect(readNativeInputTarget(meta)).toBeUndefined();
    }
    expect(hasNativeInputMetadata(undefined)).toBe(false);
  });
});
