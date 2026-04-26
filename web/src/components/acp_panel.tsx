import { UnstyledButton } from "@mantine/core";
import React from "react";
import type { AcpView } from "../acp";
import type { AcpDebugProps } from "./acp_debug";
import {
  ACP_INPUT_DOCK_CONVERSATION_CLEARANCE_PX,
  ACP_INPUT_DOCK_CONVERSATION_MARGIN_PX,
  resolveAcpInputDockConversationClearance,
} from "./acp_input_dock_clearance";
import { AcpConversation, AcpConversationProps } from "./acp_conversation";
import type { AcpPlanProps } from "./acp_plan";
import { AcpDebugSlot } from "./acp_panel_debug_slot";
import { AcpPanelTabs } from "./acp_panel_tabs";
import type { AcpPanelTab } from "./acp_panel_types";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  ACP_PANEL_HEAD_CLASS,
  ACP_PANEL_ROOT_CLASS,
} from "../ui/tailwind_classes";

const LazyAcpPlan = React.lazy(async () => {
  const mod = await import("./acp_plan");
  return { default: mod.AcpPlan };
});

type AcpPanelProps = {
  acpView: AcpView;
  subtitle: string | null;
  mobileTitle?: string | null;
  headerContext?: React.ReactNode;
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

function AcpConversationLoadingSkeleton() {
  return (
    <div
      className="flex min-h-0 flex-1 flex-col overflow-hidden px-2.5 py-1.5"
      data-acp-conversation-loading-skeleton="true"
      aria-busy="true"
    >
      <div
        role="status"
        aria-live="polite"
        className="mb-1.5 px-0.5 text-[11px] font-medium uppercase tracking-[0.08em] text-notion-text-muted/70"
      >
        Loading activity...
      </div>
      <div className="flex flex-col gap-2" aria-hidden="true">
        {Array.from({ length: 6 }, (_, index) => (
          <div
            key={`acp-loading-${index}`}
            className={`flex ${index % 3 === 2 ? "justify-end" : "justify-start"}`}
          >
            <div className="max-w-[min(100%,42rem)] min-w-[12rem] rounded-[12px] border border-notion-border/60 bg-white px-2 py-1.5 shadow-none">
              <div className="space-y-1">
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

function AcpPlanLoadingSkeleton() {
  return (
    <section className="acp-plan-view flex min-h-0 flex-1 flex-col gap-3 p-3 sm:p-4">
      <div className="rounded-xl border border-ui-border bg-ui-surface p-3 shadow-sm sm:p-4">
        <div className="space-y-3" aria-hidden="true">
          <div className="h-4 w-28 animate-pulse rounded bg-notion-hover" />
          <div className="space-y-2">
            <div className="h-3 w-32 animate-pulse rounded bg-notion-hover/90" />
            <div className="h-2 animate-pulse rounded-full bg-notion-hover/80" />
          </div>
          <div className="space-y-2">
            {Array.from({ length: 3 }, (_, index) => (
              <div
                key={`acp-plan-loading-${index}`}
                className="h-9 animate-pulse rounded-md border border-ui-border bg-ui-surface-soft"
              />
            ))}
          </div>
        </div>
      </div>
    </section>
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
  headerContext,
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
  const tabsNode = (
    <AcpPanelTabs
      acpTab={acpTab}
      developerMode={developerMode}
      showConversationBadge={showConversationBadge}
      pendingCount={conversation.pendingCount}
      planEntries={plan.plan?.entries ?? []}
      onSelectTab={onSelectTab}
    />
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
          className={`${mobileTitle ? "hidden sm:flex " : "flex "}items-center justify-between gap-2 max-[720px]:w-full max-[720px]:justify-start max-[720px]:flex-wrap`}
        >
          {headerContext ? (
            <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2 max-[720px]:w-full">
              {headerContext}
            </div>
          ) : (
            <div className="min-w-0 flex-1" />
          )}
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
      {effectiveTab === "plan" && (
        <React.Suspense fallback={<AcpPlanLoadingSkeleton />}>
          <LazyAcpPlan {...plan} />
        </React.Suspense>
      )}
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
