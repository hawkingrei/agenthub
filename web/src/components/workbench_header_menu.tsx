import React from "react";
import { Menu } from "@mantine/core";

const FLOATING_MENU_WITHIN_PORTAL = import.meta.env.MODE !== "test";

type WorkbenchHeaderMenuProps = {
  active: "agents" | "teams";
  username: string;
  isRoot: boolean;
  onLogout: () => void;
  onNavigate: (pathname: string) => void;
  buttonClassName: string;
  defaultOpened?: boolean;
};

export const WorkbenchHeaderMenu = React.memo(function WorkbenchHeaderMenu({
  active,
  username,
  isRoot,
  onLogout,
  onNavigate,
  buttonClassName,
  defaultOpened = false,
}: WorkbenchHeaderMenuProps) {
  return (
    <Menu
      withinPortal={FLOATING_MENU_WITHIN_PORTAL}
      position="bottom-end"
      shadow="md"
      zIndex={400}
      defaultOpened={defaultOpened}
    >
      <Menu.Target>
        <button
          type="button"
          className={buttonClassName}
          aria-label="Open workbench menu"
        >
          <i className="bi bi-grid-3x3-gap text-[13px] sm:text-[14px]" aria-hidden="true" />
          <span className="hidden sm:inline">Menu</span>
          <i className="bi bi-chevron-down text-[10px] text-black/65" aria-hidden="true" />
        </button>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>{username}</Menu.Label>
        <Menu.Item onClick={() => onNavigate("/")} disabled={active === "agents"}>
          Agents
        </Menu.Item>
        <Menu.Item onClick={() => onNavigate("/teams")} disabled={active === "teams"}>
          Teams
        </Menu.Item>
        <Menu.Divider />
        {isRoot && (
          <Menu.Item onClick={() => onNavigate("/admin")}>
            Settings
          </Menu.Item>
        )}
        {isRoot && <Menu.Divider />}
        <Menu.Item color="red" onClick={onLogout}>
          Logout
        </Menu.Item>
      </Menu.Dropdown>
    </Menu>
  );
});
