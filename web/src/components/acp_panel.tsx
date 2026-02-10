import React from "react";
import { AcpView } from "../acp";
import { AcpDebug, AcpDebugProps } from "./acp_debug";
import { AcpConversation, AcpConversationProps } from "./acp_conversation";

type AcpPanelProps = {
  acpView: AcpView;
  subtitle: string | null;
  acpTab: "conversation" | "debug";
  onSelectTab: (tab: "conversation" | "debug") => void;
  showConversationBadge: boolean;
  canControlAcp: boolean;
  onAcpCancel: () => void;
  conversation: AcpConversationProps;
  debug: AcpDebugProps;
};

export function AcpPanel({
  acpView,
  subtitle,
  acpTab,
  onSelectTab,
  showConversationBadge,
  canControlAcp,
  onAcpCancel,
  conversation,
  debug,
}: AcpPanelProps) {
  const runStatus = acpView.runStatus?.status ?? null;
  const hasInProgressToolCall = acpView.toolCalls.some(
    (call) => call.status === "in_progress"
  );
  const isActiveRun = runStatus === "running" || hasInProgressToolCall;
  const canInterrupt = canControlAcp && isActiveRun;
  return (
    <div className="acp">
      <div className="acp-head minimal">
        <div className="acp-subtitle">
          {subtitle ?? " "}
        </div>
        <div className="acp-actions">
          <button
            className="acp-interrupt-button"
            onClick={onAcpCancel}
            disabled={!canInterrupt}
            title="Interrupt current run"
          >
            Interrupt
          </button>
          <div className="acp-tabs">
            <button
              className={acpTab === "conversation" ? "tab active" : "tab"}
              onClick={() => onSelectTab("conversation")}
            >
              Conversation
              {showConversationBadge && (
                <span className="tab-badge">+{conversation.pendingCount}</span>
              )}
            </button>
            <button
              className={acpTab === "debug" ? "tab active" : "tab"}
              onClick={() => onSelectTab("debug")}
            >
              Debug
            </button>
          </div>
        </div>
      </div>
      {acpTab === "conversation" && <AcpConversation {...conversation} />}
      {acpTab === "debug" && <AcpDebug {...debug} />}
    </div>
  );
}

export type { AcpPanelProps };
