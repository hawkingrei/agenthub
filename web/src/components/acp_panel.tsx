import React from "react";
import { AcpView } from "../acp";
import { AcpDebug, AcpDebugProps } from "./acp_debug";
import { AcpConversation, AcpConversationProps } from "./acp_conversation";
import { AcpPlan, AcpPlanProps, summarizePlanEntries } from "./acp_plan";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  ACP_PANEL_HEAD_CLASS,
  ACP_PANEL_ROOT_CLASS,
  ACP_PANEL_TABS_CLASS,
  ACP_TAB_BADGE_CLASS,
  ACP_TAB_BUTTON_BASE_CLASS,
  ACP_TAB_BUTTON_ACTIVE_CLASS,
  ACP_TAB_BUTTON_IDLE_CLASS,
} from "../ui/tailwind_classes";

type AcpPanelTab = "conversation" | "plan" | "debug";

type AcpPanelProps = {
  acpView: AcpView;
  subtitle: string | null;
  mobileTitle?: string | null;
  acpTab: AcpPanelTab;
  developerMode: boolean;
  conversationBottomClearance?: number;
  onSelectTab: (tab: AcpPanelTab) => void;
  showConversationBadge: boolean;
  showConversationJump: boolean;
  showFloatingConversationJump?: boolean;
  onJumpToConversationBottom: () => void;
  conversation: AcpConversationProps;
  plan: AcpPlanProps;
  debug: AcpDebugProps;
};

export const ACP_INPUT_DOCK_CONVERSATION_CLEARANCE_PX = 104;

function AcpPanelView({
  subtitle,
  mobileTitle,
  acpTab,
  developerMode,
  conversationBottomClearance = 0,
  onSelectTab,
  showConversationBadge,
  showConversationJump,
  showFloatingConversationJump = true,
  onJumpToConversationBottom,
  conversation,
  plan,
  debug,
}: AcpPanelProps) {
  const effectiveTab = !developerMode && acpTab === "debug" ? "conversation" : acpTab;
  const planSummary = summarizePlanEntries(plan.plan?.entries ?? []);
  const planStatus =
    planSummary.active > 0
      ? {
          label: `${planSummary.active} active`,
          className:
            "bg-notion-accent/10 text-notion-accent",
        }
      : planSummary.pending > 0
        ? {
            label: `${planSummary.pending} pending`,
            className:
              "bg-amber-100 text-amber-800",
          }
        : planSummary.total > 0
          ? {
              label: "done",
              className:
                "bg-emerald-100 text-emerald-800",
            }
          : null;
  const tabButtonClassName = (selected: boolean, withGap = false) =>
    `${ACP_TAB_BUTTON_BASE_CLASS} ${withGap ? "gap-2 " : ""}${selected ? ACP_TAB_BUTTON_ACTIVE_CLASS : ACP_TAB_BUTTON_IDLE_CLASS}`;
  const tabsNode = (
    <div className={ACP_PANEL_TABS_CLASS}>
      <button
        type="button"
        className={tabButtonClassName(effectiveTab === "conversation", true)}
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
        type="button"
        className={tabButtonClassName(effectiveTab === "plan")}
        onClick={() => onSelectTab("plan")}
      >
        Plan
        {planStatus ? (
          <span className={`${ACP_TAB_BADGE_CLASS} ${planStatus.className}`}>
            {planStatus.label}
          </span>
        ) : null}
      </button>
      {developerMode && (
        <button
          type="button"
          className={tabButtonClassName(effectiveTab === "debug")}
          onClick={() => onSelectTab("debug")}
        >
          Debug
        </button>
      )}
    </div>
  );
  return (
    <div className={ACP_PANEL_ROOT_CLASS}>
      <div className={ACP_PANEL_HEAD_CLASS}>
        {mobileTitle ? (
          <div className="flex min-w-0 items-center gap-2 sm:hidden">
            <div className="min-w-0 max-w-[42%] flex-none overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-bold text-notion-text">
              {mobileTitle}
            </div>
            <div className="min-w-0 flex-1">
              {tabsNode}
            </div>
          </div>
        ) : null}
        {subtitle ? (
          <div
            className={`${mobileTitle ? "hidden sm:block " : ""}acp-subtitle text-[12px] text-notion-text-muted max-[720px]:hidden`}
          >
            {subtitle}
          </div>
        ) : null}
        <div
          className={`${mobileTitle ? "hidden sm:flex " : "flex "}items-center gap-2 max-[720px]:w-full max-[720px]:justify-start max-[720px]:flex-wrap`}
        >
          {tabsNode}
        </div>
      </div>
      {effectiveTab === "conversation" && (
        <AcpConversation
          {...conversation}
          bottomClearancePx={conversationBottomClearance}
        />
      )}
      {effectiveTab === "plan" && <AcpPlan {...plan} />}
      {developerMode && effectiveTab === "debug" && <AcpDebug {...debug} />}
      {effectiveTab === "conversation" &&
      showConversationJump &&
      showFloatingConversationJump ? (
        <button
          type="button"
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
