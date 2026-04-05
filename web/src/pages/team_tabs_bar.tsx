import { UnstyledButton } from "@mantine/core";
import { TEAM_TAB_BAR_CLASS, TEAM_TAB_BUTTON_ACTIVE_CLASS, TEAM_TAB_BUTTON_IDLE_CLASS } from "../ui/tailwind_classes";
import { TEAM_TAB_ITEMS, type TeamTab } from "./team/state";

type TeamTabsBarProps = {
  tab: TeamTab;
  onTabChange: (next: TeamTab) => void;
  items?: ReadonlyArray<{ value: TeamTab; label: string }>;
};

export function TeamTabsBar(props: TeamTabsBarProps) {
  const { tab, onTabChange, items = TEAM_TAB_ITEMS } = props;
  return (
    <div className={`mt-2 ${TEAM_TAB_BAR_CLASS}`} data-team-surface="workflow-tabs">
      {items.map((item) => (
        <UnstyledButton
          key={item.value}
          className={tab === item.value ? TEAM_TAB_BUTTON_ACTIVE_CLASS : TEAM_TAB_BUTTON_IDLE_CLASS}
          onClick={() => onTabChange(item.value)}
        >
          {item.label}
        </UnstyledButton>
      ))}
    </div>
  );
}
