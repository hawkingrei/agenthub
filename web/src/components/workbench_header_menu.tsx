import React from "react";
import { Menu, UnstyledButton } from "@mantine/core";
import { NOTION_FLOATING_MENU_PROPS } from "../ui/floating_surfaces";

type WorkbenchHeaderMenuProps = {
  active: "workspace" | "teams";
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
      position="bottom-end"
      defaultOpened={defaultOpened}
      {...NOTION_FLOATING_MENU_PROPS}
    >
      <Menu.Target>
        <UnstyledButton
          type="button"
          className={buttonClassName}
          aria-label="Open workbench menu"
        >
          <i className="bi bi-grid-3x3-gap text-[13px] sm:text-[14px]" aria-hidden="true" />
          <span className="hidden sm:inline">Workspace</span>
          <i className="bi bi-chevron-down text-[10px] text-black/65" aria-hidden="true" />
        </UnstyledButton>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>{username}</Menu.Label>
        <Menu.Label>Workspace</Menu.Label>
        <Menu.Item onClick={() => onNavigate("/workspace")} disabled={active === "workspace"}>
          Workspace
        </Menu.Item>
        <Menu.Item onClick={() => onNavigate("/workspace/teams")} disabled={active === "teams"}>
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
