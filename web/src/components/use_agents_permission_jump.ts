import React from "react";
import type { AcpPermissionRecord } from "../api";

const PERMISSION_JUMP_MAX_ATTEMPTS = 24;
const PERMISSION_JUMP_RETRY_DELAY_MS = 120;

export type PendingPermissionJumpState = {
  toolCallId: string;
  sessionId: string | null;
  attempts: number;
};

export function shouldAttemptPermissionJump(
  pending: PendingPermissionJumpState | null,
  acpTab: "conversation" | "plan" | "debug",
  activeSessionId: string | null
): "idle" | "wait" | "attempt" | "clear" {
  if (!pending) return "idle";
  if (acpTab !== "conversation") return "wait";
  if (pending.sessionId && activeSessionId !== pending.sessionId) return "wait";
  if (pending.attempts >= PERMISSION_JUMP_MAX_ATTEMPTS) return "clear";
  return "attempt";
}

export function useAgentsPermissionJump(params: {
  acpTab: "conversation" | "plan" | "debug";
  activeSessionId: string | null;
  jumpToConversationToolCall: (toolCallId: string) => boolean;
  onSelectTab: (tab: "conversation" | "plan" | "debug") => void;
}) {
  const {
    acpTab,
    activeSessionId,
    jumpToConversationToolCall,
    onSelectTab,
  } = params;
  const [pendingPermissionJump, setPendingPermissionJump] =
    React.useState<PendingPermissionJumpState | null>(null);

  const onJumpToPermissionHistory = React.useCallback(
    (permission: AcpPermissionRecord) => {
      const toolCallId = permission.tool_call_id?.trim();
      if (!toolCallId) return;
      onSelectTab("conversation");
      setPendingPermissionJump({
        toolCallId,
        sessionId: permission.session_id ?? null,
        attempts: 0,
      });
    },
    [onSelectTab]
  );

  React.useEffect(() => {
    const jumpDecision = shouldAttemptPermissionJump(
      pendingPermissionJump,
      acpTab,
      activeSessionId
    );
    if (jumpDecision === "idle" || jumpDecision === "wait") return;
    if (jumpDecision === "clear") {
      setPendingPermissionJump(null);
      return;
    }
    if (!pendingPermissionJump) return;
    if (jumpToConversationToolCall(pendingPermissionJump.toolCallId)) {
      setPendingPermissionJump(null);
      return;
    }
    const timer = window.setTimeout(() => {
      setPendingPermissionJump((previous) => {
        if (!previous) return previous;
        return { ...previous, attempts: previous.attempts + 1 };
      });
    }, PERMISSION_JUMP_RETRY_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [
    pendingPermissionJump,
    acpTab,
    activeSessionId,
    jumpToConversationToolCall,
  ]);

  return { onJumpToPermissionHistory };
}
