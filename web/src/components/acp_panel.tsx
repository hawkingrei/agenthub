import React from "react";
import { AcpView } from "../acp";
import { AcpDebug, AcpDebugProps } from "./acp_debug";
import { AcpConversation, AcpConversationProps } from "./acp_conversation";

type AcpPanelProps = {
  acpView: AcpView;
  activeSessionId: string | null;
  showAcpRuntime: boolean;
  thinkingStartTs: number | null;
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
  activeSessionId,
  showAcpRuntime,
  thinkingStartTs,
  acpTab,
  onSelectTab,
  showConversationBadge,
  canControlAcp,
  onAcpCancel,
  conversation,
  debug,
}: AcpPanelProps) {
  const canInterrupt =
    canControlAcp && acpView.runStatus?.status === "running";
  return (
    <div className="acp">
      <div className="acp-head">
        <div className="acp-title">
          ACP
          {activeSessionId && (
            <span className="acp-session">{activeSessionId.slice(0, 8)}</span>
          )}
          {showAcpRuntime && acpView.runStatus?.status && (
            <span className={`acp-run ${acpView.runStatus.status}`}>
              {acpView.runStatus.status}
            </span>
          )}
          {showAcpRuntime && thinkingStartTs && (
            <span className="acp-thinking">
              thinking {Math.max(0, Math.floor(Date.now() / 1000 - thinkingStartTs))}
              s
            </span>
          )}
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
