import type { MenuProps, ModalProps } from "@mantine/core";

export const FLOATING_MENU_WITHIN_PORTAL = import.meta.env.MODE !== "test";

export const NOTION_FLOATING_MENU_CLASSNAMES: NonNullable<MenuProps["classNames"]> = {
  dropdown:
    "min-w-[220px] rounded-[10px] border border-black/[0.06] bg-white p-1 shadow-[0_20px_24px_rgba(25,25,25,0.05),0_5px_8px_rgba(25,25,25,0.027),0_0_0_1px_rgba(42,28,0,0.07)]",
  label:
    "px-2.5 pt-1.5 pb-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-notion-text-muted",
  item:
    "min-h-7 rounded-md px-2.5 py-1.5 text-[13px] font-medium leading-5 text-notion-text transition data-[hovered]:bg-black/[0.05] data-[hovered]:text-notion-text data-[disabled]:opacity-45 data-[disabled]:bg-transparent",
  divider: "my-1 border-notion-border/80",
};

export const NOTION_FLOATING_MENU_PROPS: Partial<MenuProps> = {
  withinPortal: FLOATING_MENU_WITHIN_PORTAL,
  offset: 6,
  zIndex: 400,
  classNames: NOTION_FLOATING_MENU_CLASSNAMES,
};

export const NOTION_MODAL_CLASSNAMES: NonNullable<ModalProps["classNames"]> = {
  content:
    "rounded-[10px] border border-black/[0.06] bg-white shadow-[0_20px_24px_rgba(25,25,25,0.05),0_5px_8px_rgba(25,25,25,0.027),0_0_0_1px_rgba(42,28,0,0.07)]",
  header: "min-h-0 border-b border-black/[0.06] bg-transparent px-4 py-3",
  title: "text-[12px] font-semibold tracking-tight text-notion-text",
  body: "px-4 py-4",
  close: "rounded-md text-notion-text-muted transition hover:bg-notion-hover hover:text-notion-text",
};

export const TEAM_MODAL_CLASSNAMES: NonNullable<ModalProps["classNames"]> = {
  ...NOTION_MODAL_CLASSNAMES,
  content:
    "rounded-[10px] border border-black/[0.06] bg-[#f8f5ee] shadow-[0_20px_24px_rgba(25,25,25,0.05),0_5px_8px_rgba(25,25,25,0.027),0_0_0_1px_rgba(42,28,0,0.07)]",
  title:
    "text-[11px] font-bold uppercase tracking-[0.16em] text-[#5b6775]",
  body: "px-4 py-4",
};

export const NOTION_MODAL_OVERLAY_PROPS: ModalProps["overlayProps"] = {
  backgroundOpacity: 0.18,
  blur: 0,
};
