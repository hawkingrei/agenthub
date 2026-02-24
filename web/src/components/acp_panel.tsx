import React from "react";
import { AcpView } from "../acp";
import { AcpDebug, AcpDebugProps } from "./acp_debug";
import { AcpConversation, AcpConversationProps } from "./acp_conversation";
import { AcpPlan, AcpPlanProps } from "./acp_plan";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  ACP_PANEL_HEAD_CLASS,
  ACP_PANEL_ROOT_CLASS,
  ACP_PANEL_TABS_CLASS,
  ACP_TAB_BADGE_CLASS,
  ACP_TAB_BUTTON_ACTIVE_CLASS,
  ACP_TAB_BUTTON_IDLE_CLASS,
} from "../ui/tailwind_classes";

type AcpPanelTab = "conversation" | "plan" | "debug";

type AcpPanelProps = {
  acpView: AcpView;
  subtitle: string | null;
  acpTab: AcpPanelTab;
  onSelectTab: (tab: AcpPanelTab) => void;
  showConversationBadge: boolean;
  showConversationJump: boolean;
  onJumpToConversationBottom: () => void;
  conversation: AcpConversationProps;
  plan: AcpPlanProps;
  debug: AcpDebugProps;
};

function AcpPanelView({
  subtitle,
  acpTab,
  onSelectTab,
  showConversationBadge,
  showConversationJump,
  onJumpToConversationBottom,
  conversation,
  plan,
  debug,
}: AcpPanelProps) {
  const tabButtonClassName = (selected: boolean, withGap = false) =>
    `acp-tab-button ${withGap ? "gap-2 " : ""}${selected ? ACP_TAB_BUTTON_ACTIVE_CLASS : ACP_TAB_BUTTON_IDLE_CLASS}`;
  return (
    <div className={ACP_PANEL_ROOT_CLASS}>
      <div className={ACP_PANEL_HEAD_CLASS}>
        <div className="acp-subtitle min-h-5 text-xs text-slate-500 sm:text-sm">
          {subtitle ?? " "}
        </div>
        <div className="acp-actions flex items-center gap-2">
          <div className={ACP_PANEL_TABS_CLASS}>
            <button
              className={tabButtonClassName(acpTab === "conversation", true)}
              onClick={() => onSelectTab("conversation")}
            >
              Conversation
              {showConversationBadge && (
                <span className={ACP_TAB_BADGE_CLASS}>
                  +{conversation.pendingCount}
                </span>
              )}
            </button>
            <button
              className={tabButtonClassName(acpTab === "plan")}
              onClick={() => onSelectTab("plan")}
            >
              Plan
            </button>
            <button
              className={tabButtonClassName(acpTab === "debug")}
              onClick={() => onSelectTab("debug")}
            >
              Debug
            </button>
          </div>
        </div>
      </div>
      {acpTab === "conversation" && <AcpConversation {...conversation} />}
      {acpTab === "plan" && <AcpPlan {...plan} />}
      {acpTab === "debug" && <AcpDebug {...debug} />}
      {acpTab === "conversation" && showConversationJump ? (
        <button
          className={ACP_JUMP_BOTTOM_BUTTON_CLASS}
          onClick={onJumpToConversationBottom}
          title="Jump to bottom"
          aria-label="Jump to bottom"
        >
          <i className="bi bi-chevron-down text-sm" aria-hidden="true" />
        </button>
      ) : null}
    </div>
  );
}

export const AcpPanel = React.memo(AcpPanelView);

export type { AcpPanelProps };
export { AcpPanelView };
