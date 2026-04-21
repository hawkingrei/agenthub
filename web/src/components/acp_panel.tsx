import { UnstyledButton } from "@mantine/core";
import React from "react";
import type { AcpView } from "../acp";
import type { AcpDebugProps } from "./acp_debug";
import { loadAcpDebugModule } from "./acp_debug_loader";
import {
  ACP_INPUT_DOCK_CONVERSATION_CLEARANCE_PX,
  ACP_INPUT_DOCK_CONVERSATION_MARGIN_PX,
  resolveAcpInputDockConversationClearance,
} from "./acp_input_dock_clearance";
import { AcpConversation, AcpConversationProps } from "./acp_conversation";
import { AcpPlan, AcpPlanProps, summarizePlanEntries } from "./acp_plan";
import { cx } from "../ui/primitives";
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
  conversationLoading?: boolean;
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

function AcpDebugSlot(props: AcpDebugProps) {
  const [DebugView, setDebugView] = React.useState<React.ComponentType<AcpDebugProps> | null>(null);
  const [loadFailed, setLoadFailed] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    setLoadFailed(false);
    void loadAcpDebugModule()
      .then((module) => {
        if (!cancelled) {
          setDebugView(() => module.AcpDebug);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLoadFailed(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (loadFailed) {
    return (
      <div className="px-3 py-2 text-sm text-notion-text-muted">
        Unable to load debug view.
      </div>
    );
  }

  if (DebugView == null) {
    return (
      <div className="px-3 py-2 text-sm text-notion-text-muted">
        Loading debug...
      </div>
    );
  }

  return <DebugView {...props} />;
}

function AcpConversationLoadingSkeleton() {
  return (
    <div
      className="flex min-h-0 flex-1 flex-col overflow-hidden px-3 py-2"
      data-acp-conversation-loading-skeleton="true"
      aria-hidden="true"
    >
      <div className="flex flex-col gap-2.5">
        {Array.from({ length: 6 }, (_, index) => (
          <div
            key={`acp-loading-${index}`}
            className={`flex ${index % 3 === 2 ? "justify-end" : "justify-start"}`}
          >
            <div className="max-w-[min(100%,42rem)] min-w-[12rem] rounded-[16px] border border-notion-border/60 bg-white px-3 py-2 shadow-notion-soft">
              <div className="space-y-1.5">
                <div className="h-3 animate-pulse rounded bg-notion-hover" />
                <div className="h-3 w-11/12 animate-pulse rounded bg-notion-hover/90" />
                <div
                  className={`h-3 animate-pulse rounded bg-notion-hover/80 ${
                    index % 2 === 0 ? "w-8/12" : "w-6/12"
                  }`}
                />
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export {
  ACP_INPUT_DOCK_CONVERSATION_CLEARANCE_PX,
  ACP_INPUT_DOCK_CONVERSATION_MARGIN_PX,
  resolveAcpInputDockConversationClearance,
};

function AcpPanelView({
  subtitle,
  mobileTitle,
  acpTab,
  developerMode,
  conversationLoading = false,
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
    cx(
      "acp-tab-button",
      ACP_TAB_BUTTON_BASE_CLASS,
      withGap && "gap-2",
      selected ? ACP_TAB_BUTTON_ACTIVE_CLASS : ACP_TAB_BUTTON_IDLE_CLASS
    );
  const tabsNode = (
    <div className={ACP_PANEL_TABS_CLASS}>
      <UnstyledButton
        type="button"
        className={tabButtonClassName(effectiveTab === "conversation", true)}
        onClick={() => onSelectTab("conversation")}
      >
        Activity
        {showConversationBadge && (
          <span className={ACP_TAB_BADGE_CLASS}>
            +{conversation.pendingCount}
          </span>
        )}
      </UnstyledButton>
      <UnstyledButton
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
      </UnstyledButton>
      {developerMode && (
        <UnstyledButton
          type="button"
          className={tabButtonClassName(effectiveTab === "debug")}
          onClick={() => onSelectTab("debug")}
        >
          Inspect
        </UnstyledButton>
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
      {effectiveTab === "conversation" && conversationLoading ? (
        <AcpConversationLoadingSkeleton />
      ) : null}
      {effectiveTab === "conversation" && !conversationLoading && (
        <AcpConversation
          {...conversation}
          bottomClearancePx={conversationBottomClearance}
        />
      )}
      {effectiveTab === "plan" && <AcpPlan {...plan} />}
      {developerMode && effectiveTab === "debug" && <AcpDebugSlot {...debug} />}
      {effectiveTab === "conversation" &&
      showConversationJump &&
      showFloatingConversationJump ? (
        <UnstyledButton
          type="button"
          className={ACP_JUMP_BOTTOM_BUTTON_CLASS}
          onClick={onJumpToConversationBottom}
          title="Jump to bottom"
          aria-label="Jump to bottom"
        >
          <i className="bi bi-chevron-down text-sm" aria-hidden="true" />
        </UnstyledButton>
      ) : null}
    </div>
  );
}

export const AcpPanel = React.memo(AcpPanelView);

export type { AcpPanelProps };
export { AcpPanelView };
