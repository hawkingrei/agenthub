import { UnstyledButton } from "@mantine/core";
import React from "react";
import type { AcpPlanEntry } from "../acp";
import { cx } from "../ui/primitives";
import {
  ACP_PANEL_TABS_CLASS,
  ACP_TAB_BADGE_CLASS,
  ACP_TAB_BUTTON_ACTIVE_CLASS,
  ACP_TAB_BUTTON_BASE_CLASS,
  ACP_TAB_BUTTON_IDLE_CLASS,
} from "../ui/tailwind_classes";
import { summarizePlanEntries } from "./acp_plan_summary";
import type { AcpPanelTab } from "./acp_panel_types";

type AcpPanelTabsProps = {
  acpTab: AcpPanelTab;
  developerMode: boolean;
  showConversationBadge: boolean;
  pendingCount: number;
  planEntries: AcpPlanEntry[];
  onSelectTab: (tab: AcpPanelTab) => void;
};

export function AcpPanelTabs({
  acpTab,
  developerMode,
  showConversationBadge,
  pendingCount,
  planEntries,
  onSelectTab,
}: AcpPanelTabsProps) {
  const effectiveTab = !developerMode && acpTab === "debug" ? "conversation" : acpTab;
  const planSummary = summarizePlanEntries(planEntries);
  const planStatus =
    planSummary.active > 0
      ? {
          label: `${planSummary.active} active`,
          className: "bg-notion-accent/10 text-notion-accent",
        }
      : planSummary.pending > 0
        ? {
            label: `${planSummary.pending} pending`,
            className: "bg-amber-100 text-amber-800",
          }
        : planSummary.total > 0
          ? {
              label: "done",
              className: "bg-emerald-100 text-emerald-800",
            }
          : null;

  const tabButtonClassName = (selected: boolean, withGap = false) =>
    cx(
      "acp-tab-button",
      ACP_TAB_BUTTON_BASE_CLASS,
      withGap && "gap-2",
      selected ? ACP_TAB_BUTTON_ACTIVE_CLASS : ACP_TAB_BUTTON_IDLE_CLASS
    );

  return (
    <div className={ACP_PANEL_TABS_CLASS}>
      <UnstyledButton
        type="button"
        className={tabButtonClassName(effectiveTab === "conversation", true)}
        onClick={() => onSelectTab("conversation")}
      >
        Activity
        {showConversationBadge && (
          <span className={ACP_TAB_BADGE_CLASS}>+{pendingCount}</span>
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
}
