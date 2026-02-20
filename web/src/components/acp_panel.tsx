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
  conversation: AcpConversationProps;
  debug: AcpDebugProps;
};

function AcpPanelView({
  subtitle,
  acpTab,
  onSelectTab,
  showConversationBadge,
  conversation,
  debug,
}: AcpPanelProps) {
  return (
    <div className="acp flex min-h-0 flex-1 flex-col rounded-2xl border border-slate-200 bg-white/90 shadow-sm">
      <div className="acp-head minimal flex flex-wrap items-start justify-between gap-3 border-b border-slate-200 px-3 py-2 sm:px-4">
        <div className="acp-subtitle min-h-5 text-xs text-slate-500 sm:text-sm">
          {subtitle ?? " "}
        </div>
        <div className="acp-actions flex items-center gap-2">
          <div className="acp-tabs flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 p-1">
            <button
              className={`tab inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm ${
                acpTab === "conversation"
                  ? "active bg-slate-900 text-white shadow-sm"
                  : "text-slate-600 hover:bg-white hover:text-slate-900"
              }`}
              onClick={() => onSelectTab("conversation")}
            >
              Conversation
              {showConversationBadge && (
                <span className="tab-badge rounded-full border border-current/30 px-1.5 py-0.5 text-[10px] font-semibold leading-none sm:text-xs">
                  +{conversation.pendingCount}
                </span>
              )}
            </button>
            <button
              className={`tab inline-flex items-center rounded-md px-3 py-1.5 text-xs font-medium transition sm:text-sm ${
                acpTab === "debug"
                  ? "active bg-slate-900 text-white shadow-sm"
                  : "text-slate-600 hover:bg-white hover:text-slate-900"
              }`}
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

export const AcpPanel = React.memo(AcpPanelView);

export type { AcpPanelProps };
export { AcpPanelView };
